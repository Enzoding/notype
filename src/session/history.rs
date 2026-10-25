use std::sync::Arc;

use gpui_kit::Context;

use crate::audio;
use crate::gateway;
use crate::mining::{self, EditMode};
use crate::polish::{self, PolishMode, PolishOptions};
use crate::store::HistoryItem;

use super::Session;

impl Session {
    pub fn repolish_history_item(&mut self, item: HistoryItem, cx: &mut Context<Self>) {
        if self.repolishing_id.is_some() || self.key.is_empty() {
            return;
        }
        let key = self.key.clone();
        let model = self.polish_model.clone();
        let phrases = self
            .store
            .lock()
            .map(|s| s.phrase_list())
            .unwrap_or_default();
        let mode = PolishMode::from_str(&item.mode)
            .unwrap_or_else(|| polish::mode_for_bundle(&item.bundle_id));
        let options = PolishOptions {
            mode,
            custom_prompt: self.custom_prompt.clone(),
            space_cjk: self.space_cjk,
        };
        let mode_str = mode.as_str().to_string();
        let id = item.id;
        let transcript = item.transcript.clone();
        self.repolishing_id = Some(id);
        cx.notify();

        let store = Arc::clone(&self.store);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    gateway::polish_with_options(&key, &transcript, &phrases, &options, &model)
                })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.repolishing_id = None;
                if let Ok(polished) = result {
                    if let Ok(s) = store.lock() {
                        let _ = s.update_history_polished(id, &polished, &mode_str);
                        session.history = s.history().unwrap_or_default();
                        session.stats = s.stats();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_play_history(&mut self, id: i64, cx: &mut Context<Self>) {
        if audio::playing_id() == Some(id) {
            audio::stop_wav();
            self.playing_id = None;
        } else {
            let path = self
                .store
                .lock()
                .map(|s| s.audio_path(id))
                .unwrap_or_default();
            if audio::play_wav(id, &path).is_ok() {
                self.playing_id = Some(id);
            }
        }
        cx.notify();
    }

    pub fn delete_history(&mut self, id: i64, cx: &mut Context<Self>) {
        if audio::playing_id() == Some(id) {
            audio::stop_wav();
            self.playing_id = None;
        }
        self.audio_ids.remove(&id);
        if let Ok(store) = self.store.lock() {
            let _ = store.delete_history(id);
            self.history = store.history().unwrap_or_default();
            self.stats = store.stats();
        }
        cx.notify();
    }

    pub fn clear_history(&mut self, cx: &mut Context<Self>) {
        audio::stop_wav();
        self.playing_id = None;
        self.audio_ids.clear();
        if let Ok(store) = self.store.lock() {
            let _ = store.clear_history();
            self.history = Vec::new();
            self.stats = store.stats();
        }
        cx.notify();
    }

    /// Persist a correction learned in the app (history / studio).
    pub fn apply_history_correction(&mut self, id: i64, corrected: &str, cx: &mut Context<Self>) {
        let Some(item) = self.store.lock().ok().and_then(|store| store.history_item(id)) else {
            return;
        };
        let old = item.display_text().to_string();
        if old.trim() == corrected.trim() {
            return;
        }
        let known: Vec<String> = self.phrases.iter().map(|p| p.phrase.clone()).collect();
        let learning =
            mining::learn_from_edit(&old, corrected, &old, id, EditMode::General, &known);
        if let Ok(store) = self.store.lock() {
            let _ = store.update_history_polished(id, corrected, &item.mode);
            self.history = store.history().unwrap_or_default();
            self.stats = store.stats();
        }
        self.persist_and_refresh(learning, "历史修正", cx);
    }
}
