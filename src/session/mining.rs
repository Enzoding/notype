use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::Context;

use crate::ax;
use crate::gateway;
use crate::mining::{self, AutoLearnMode, EditLearning, EditMode};
use crate::polish;
use crate::store::Store;

use super::time::{now_secs, truncate_text};
use super::Session;

pub(super) fn persist_learning(
    store: &Arc<Mutex<Store>>,
    learning: &EditLearning,
) -> (usize, usize) {
    let Ok(store) = store.lock() else {
        return (0, 0);
    };
    let _ = store.upsert_candidates(&learning.terms);
    let mut learned = 0usize;
    for term in &learning.terms {
        if term.auto_accept
            && store
                .accept_candidate_by_key(&term.phrase_key)
                .ok()
                .flatten()
                .is_some()
        {
            learned += 1;
        }
    }
    let mut corrections = 0usize;
    for correction in &learning.corrections {
        if correction.confidence >= 0.8
            && store
                .upsert_correction(&correction.wrong, &correction.right)
                .is_ok()
        {
            corrections += 1;
        }
    }
    for deleted in &learning.deleted {
        let _ = store.remember_ignored(deleted, "edit-delete");
    }
    (learned, corrections)
}

impl Session {
    pub fn add_phrase(&mut self, phrase: &str, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.add_phrases_batch(phrase);
            self.phrases = store.phrases().unwrap_or_default();
        }
        cx.notify();
    }

    pub fn delete_phrase(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.delete_phrase(id);
            self.phrases = store.phrases().unwrap_or_default();
            self.candidates = store.pending_candidates().unwrap_or_default();
            self.corrections = store.corrections();
        }
        cx.notify();
    }

    pub fn accept_candidate(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.accept_candidate(id);
            self.phrases = store.phrases().unwrap_or_default();
            self.candidates = store.pending_candidates().unwrap_or_default();
        }
        cx.notify();
    }

    pub fn ignore_candidate(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.ignore_candidate(id);
            self.candidates = store.pending_candidates().unwrap_or_default();
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn ignore_all_candidates(&mut self, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            for item in store.pending_candidates().unwrap_or_default() {
                let _ = store.ignore_candidate(item.id);
            }
            self.candidates = store.pending_candidates().unwrap_or_default();
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn accept_all_candidates(&mut self, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.accept_all_pending();
            self.phrases = store.phrases().unwrap_or_default();
            self.candidates = store.pending_candidates().unwrap_or_default();
        }
        cx.notify();
    }

    /// Scan history in the background. Manual scans ignore the 6-hour throttle.
    pub fn learn_now(&mut self, cx: &mut Context<Self>) {
        self.start_mining(true, cx);
    }

    /// Called after each history insert and at launch. Cheap when there is
    /// nothing new; the LLM request itself always runs on a background thread.
    pub fn maybe_mine(&mut self, cx: &mut Context<Self>) {
        if self.mining
            || self.provider_key(self.connections.kind).is_empty()
            || self.auto_learn_mode == AutoLearnMode::Off
        {
            return;
        }
        let (last_id, last_at, new_rows) = {
            let Ok(store) = self.store.lock() else {
                return;
            };
            (
                store.last_mined_history_id(),
                store.last_mined_at(),
                store.count_history_after(store.last_mined_history_id()),
            )
        };
        if new_rows == 0 {
            return;
        }
        let elapsed = now_secs().saturating_sub(last_at);
        if last_id == 0 || new_rows >= 10 || elapsed >= 6 * 3600 {
            self.start_mining(false, cx);
        }
    }

    fn start_mining(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.mining {
            return;
        }
        if self.provider_key(self.connections.kind).is_empty() {
            self.mining_status = "先在偏好设置里保存 API Key。".into();
            cx.notify();
            return;
        }
        let (rows, existing, cursor_hint) = {
            let Ok(store) = self.store.lock() else {
                return;
            };
            let cursor = if manual {
                0
            } else {
                store.last_mined_history_id()
            };
            let rows = if cursor == 0 {
                store.history_for_mining(mining::MAX_MINING_ROWS)
            } else {
                store.history_since(cursor, mining::MAX_MINING_ROWS)
            };
            (
                rows.unwrap_or_default(),
                store.phrase_keys(),
                store.max_history_id(),
            )
        };
        if rows.is_empty() {
            self.mining_status = "还没有新的口述记录。".into();
            cx.notify();
            return;
        }
        let mining_rows: Vec<mining::MiningRow> = rows
            .iter()
            .filter_map(|item| {
                let text = item.display_text().trim();
                if text.is_empty() {
                    return None;
                }
                Some(mining::MiningRow {
                    history_id: item.id,
                    app: polish::friendly_app_name(&item.bundle_id),
                    text: text.to_string(),
                })
            })
            .collect();
        if mining_rows.is_empty() {
            self.mining_status = "还没有可分析的成稿。".into();
            cx.notify();
            return;
        }
        let prompt = mining::glossary_prompt(&mining_rows);
        let cursor = rows.iter().map(|item| item.id).max().unwrap_or(cursor_hint);
        let (base_url, key, model) = self.polish_route();
        let mode = self.auto_learn_mode;
        self.mining = true;
        self.mining_status = format!("正在分析 {} 条口述…", mining_rows.len());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { gateway::extract_glossary(&base_url, &key, &model, &prompt) })
                .await;
            let outcome = match result {
                Ok(raw) => Ok(mining::parse_glossary(&raw, &mining_rows, &existing)),
                Err(err) => Err(err),
            };
            let _ = this.update(cx, |session, cx| {
                session.finish_mining(outcome, cursor, mode, cx);
            });
        })
        .detach();
    }

    fn finish_mining(
        &mut self,
        outcome: Result<Vec<mining::CandidateDraft>, String>,
        cursor: i64,
        mode: AutoLearnMode,
        cx: &mut Context<Self>,
    ) {
        self.mining = false;
        match outcome {
            Ok(drafts) => {
                let mut accepted = 0usize;
                if let Ok(store) = self.store.lock() {
                    let _ = store.upsert_candidates(&drafts);
                    if mode == AutoLearnMode::Auto {
                        for draft in &drafts {
                            if draft.auto_accept
                                && store
                                    .accept_candidate_by_key(&draft.phrase_key)
                                    .ok()
                                    .flatten()
                                    .is_some()
                            {
                                accepted += 1;
                            }
                        }
                    }
                    let _ = store.set_last_mined_history_id(cursor);
                    let _ = store.set_last_mined_at(now_secs());
                    let _ = store.set_last_mine_error("");
                    self.phrases = store.phrases().unwrap_or_default();
                    self.candidates = store.pending_candidates().unwrap_or_default();
                }
                self.last_mined_at = now_secs();
                let pending = self.candidates.len();
                self.mining_status = if drafts.is_empty() {
                    "没有发现新词条。".into()
                } else if accepted > 0 {
                    format!("已自动学习 {accepted} 个词条，{pending} 个待确认。")
                } else {
                    format!("发现 {} 个候选词条。", drafts.len())
                };
            }
            Err(err) => {
                self.mining_status = format!("学习失败：{}", truncate_text(&err, 90));
                if let Ok(store) = self.store.lock() {
                    let _ = store.set_last_mine_error(&err);
                }
            }
        }
        cx.notify();
    }

    fn refresh_learning(&mut self, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            self.candidates = store.pending_candidates().unwrap_or_default();
            self.phrases = store.phrases().unwrap_or_default();
            self.corrections = store.corrections();
        }
        cx.notify();
    }

    pub fn delete_correction(&mut self, wrong: &str, right: &str, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.delete_correction(wrong, right);
            self.corrections = store.corrections();
        }
        cx.notify();
    }

    pub(super) fn persist_and_refresh(
        &mut self,
        learning: EditLearning,
        label: &str,
        cx: &mut Context<Self>,
    ) {
        if learning.is_empty() {
            cx.notify();
            return;
        }
        let (learned, corrections) = persist_learning(&self.store, &learning);
        self.refresh_learning(cx);
        if learned > 0 || corrections > 0 {
            self.mining_status =
                format!("{label}：学习 {learned} 个词条，{corrections} 条拼写纠正。");
        }
        cx.notify();
    }

    /// After a paste, look at the focused field once more in the background.
    /// This never runs on the insertion path, so typing/pasting latency is
    /// unchanged even if the app being read is slow.
    pub(super) fn watch_paste_correction(
        &self,
        history_id: i64,
        inserted_text: String,
        cx: &mut Context<Self>,
    ) {
        if !self.learn_from_edits || inserted_text.trim().is_empty() {
            return;
        }
        let store = Arc::clone(&self.store);
        let known: Vec<String> = self.phrases.iter().map(|p| p.phrase.clone()).collect();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(800))
                .await;
            let Some(before) = cx
                .background_executor()
                .spawn(async { ax::focused_text() })
                .await
            else {
                return;
            };
            cx.background_executor().timer(Duration::from_secs(8)).await;
            let Some(after) = cx
                .background_executor()
                .spawn(async { ax::focused_text() })
                .await
            else {
                return;
            };
            if before.pid != after.pid || before.text == after.text {
                return;
            }
            let learning = mining::learn_from_edit(
                &before.text,
                &after.text,
                &inserted_text,
                history_id,
                EditMode::ReplaceOnly,
                &known,
            );
            if learning.is_empty() {
                return;
            }
            let (learned, corrections) = persist_learning(&store, &learning);
            if learned == 0 && corrections == 0 && learning.deleted.is_empty() {
                return;
            }
            let _ = this.update(cx, |session, cx| {
                session.refresh_learning(cx);
                if learned > 0 || corrections > 0 {
                    session.mining_status = format!("从粘贴后的修正中学习 {learned} 个词条");
                }
                cx.notify();
            });
        })
        .detach();
    }
}
