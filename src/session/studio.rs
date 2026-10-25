use std::sync::Arc;

use gpui_kit::Context;

use crate::gateway;
use crate::mining::{self, EditMode};
use crate::polish::PolishOptions;

use super::types::Phase;
use super::Session;

#[allow(dead_code)]
impl Session {
    pub fn run_studio_polish(&mut self, input_text: &str, cx: &mut Context<Self>) {
        let input_text = input_text.trim().to_string();
        if input_text.is_empty() || self.studio_busy {
            return;
        }
        if self.provider_key(self.connections.kind).is_empty() {
            self.studio_output = "请先在设置中填写并保存 API Key。".into();
            cx.notify();
            return;
        }
        let (base_url, key, model) = self.polish_route();
        let phrases = self
            .store
            .lock()
            .map(|s| s.phrase_list())
            .unwrap_or_default();
        let options = PolishOptions {
            mode: self.studio_mode,
            custom_prompt: self.custom_prompt.clone(),
            space_cjk: self.space_cjk,
        };
        self.studio_busy = true;
        self.studio_raw = input_text.clone();
        cx.notify();

        let store = Arc::clone(&self.store);
        let mode_str = self.studio_mode.as_str().to_string();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    gateway::polish_with_options(
                        &base_url,
                        &key,
                        &input_text,
                        &phrases,
                        &options,
                        &model,
                    )
                })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.studio_busy = false;
                match result {
                    Ok(polished) => {
                        session.studio_output = polished.clone();
                        if let Ok(s) = store.lock() {
                            let _ = s.add_history(
                                "notype.studio",
                                &session.studio_raw,
                                &polished,
                                0,
                                &mode_str,
                            );
                            session.history = s.history().unwrap_or_default();
                            session.stats = s.stats();
                        }
                    }
                    Err(err) => {
                        session.studio_output = format!("润色失败：{err}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_studio_dictation(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish(cx);
        } else if !matches!(self.phase, Phase::Working(_)) {
            self.begin(true, cx);
        }
    }

    pub fn apply_studio_correction(&mut self, corrected: &str, cx: &mut Context<Self>) {
        let old = self.studio_output.clone();
        if old.trim() == corrected.trim() {
            return;
        }
        self.studio_output = corrected.to_string();
        let known: Vec<String> = self.phrases.iter().map(|p| p.phrase.clone()).collect();
        let learning = mining::learn_from_edit(&old, corrected, &old, 0, EditMode::General, &known);
        self.persist_and_refresh(learning, "润色修正", cx);
    }
}
