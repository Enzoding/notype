use gpui_kit::Context;

use crate::audio::{self, SoundCue};
use crate::capture::ActiveRecording;
use crate::gateway::{self, POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::hotkey;
use crate::mining::AutoLearnMode;
use crate::secrets::{self, KeySource};
use crate::store::TriggerMode;

use super::Session;

pub(super) fn key_status(key: &str, source: &KeySource) -> String {
    if key.is_empty() {
        return "还没有 API Key。".into();
    }
    match source {
        KeySource::File(_) => "已从 .env.local 读取。".into(),
        KeySource::Missing => "还没有 API Key。".into(),
    }
}

impl Session {
    pub fn set_dock(&mut self, show: bool, cx: &mut Context<Self>) {
        if let Ok(store) = self.store.lock() {
            let _ = store.set_show_dock(show);
        }
        self.show_dock = show;
        cx.set_activation_policy(if show {
            gpui_kit::ActivationPolicy::Regular
        } else {
            gpui_kit::ActivationPolicy::Accessory
        });
        if show {
            cx.activate(true);
        }
        cx.notify();
    }

    pub fn set_hotkey_code(&mut self, code: i64, cx: &mut Context<Self>) {
        self.hotkey_code = code;
        hotkey::set_hotkey_code(code);
        if let Ok(store) = self.store.lock() {
            let _ = store.set_hotkey_code(code);
        }
        cx.notify();
    }

    pub fn set_trigger_mode(&mut self, mode: TriggerMode, cx: &mut Context<Self>) {
        self.trigger_mode = mode;
        hotkey::set_trigger_mode(mode);
        if let Ok(store) = self.store.lock() {
            let _ = store.set_trigger_mode(mode);
        }
        cx.notify();
    }

    pub fn set_polish_strategy(&mut self, strategy: &str, cx: &mut Context<Self>) {
        self.polish_strategy = strategy.to_string();
        if let Ok(store) = self.store.lock() {
            let _ = store.set_polish_strategy(strategy);
        }
        cx.notify();
    }

    pub fn set_custom_prompt(&mut self, prompt: &str, cx: &mut Context<Self>) {
        self.custom_prompt = prompt.trim().to_string();
        if let Ok(store) = self.store.lock() {
            let _ = store.set_custom_prompt(&self.custom_prompt);
        }
        cx.notify();
    }

    pub fn set_space_cjk(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.space_cjk = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_space_cjk(enabled);
        }
        cx.notify();
    }

    pub fn set_sound_feedback(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.sound_feedback = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_sound_feedback(enabled);
        }
        if enabled {
            audio::play_cue(SoundCue::Start);
        }
        cx.notify();
    }

    pub fn set_save_audio(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.save_audio = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_save_audio(enabled);
        }
        cx.notify();
    }

    pub fn set_auto_learn_mode(&mut self, mode: AutoLearnMode, cx: &mut Context<Self>) {
        self.auto_learn_mode = mode;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_auto_learn_mode(mode.as_str());
        }
        cx.notify();
    }

    pub fn set_learn_from_edits(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.learn_from_edits = enabled;
        if let Ok(store) = self.store.lock() {
            let _ = store.set_learn_from_edits(enabled);
        }
        cx.notify();
    }

    pub fn set_mic_device(&mut self, name: &str, cx: &mut Context<Self>) {
        self.mic_device = name.to_string();
        if let Ok(store) = self.store.lock() {
            let _ = store.set_mic_device(name);
        }
        if self.mic_testing {
            self.toggle_mic_test(cx);
            self.toggle_mic_test(cx);
        }
        cx.notify();
    }

    pub fn refresh_mics(&mut self, cx: &mut Context<Self>) {
        self.available_mics = crate::capture::list_input_devices();
        cx.notify();
    }

    pub fn toggle_mic_test(&mut self, cx: &mut Context<Self>) {
        if let Some(stream) = self.mic_test_stream.take() {
            drop(stream.finish());
            self.mic_testing = false;
            self.level = 0.0;
            self.wave = [0.0; crate::capture::WAVE_BARS];
        } else if self.recording.is_none() {
            let preferred = if self.mic_device.is_empty() {
                None
            } else {
                Some(self.mic_device.as_str())
            };
            if let Ok(stream) = ActiveRecording::start_with_device(preferred) {
                self.mic_test_stream = Some(stream);
                self.mic_testing = true;
            }
        }
        cx.notify();
    }

    pub fn test_gateway(&mut self, cx: &mut Context<Self>) {
        if self.gateway_testing {
            return;
        }
        let key = self.key.clone();
        let model = self.polish_model.clone();
        self.gateway_testing = true;
        self.gateway_test_status = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { gateway::test_connection(&key, &model) })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.gateway_testing = false;
                session.gateway_test_status = Some(match res {
                    Ok(ms) => (true, format!("已连通，延迟 {ms} ms")),
                    Err(err) => (false, err),
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub fn reload_key(&mut self, cx: &mut Context<Self>) {
        let (key, source) = secrets::load_key();
        let _ = (TRANSCRIBE_MODEL, POLISH_MODEL);
        self.key = key;
        self.key_status = key_status(&self.key, &source);
        cx.notify();
    }

    pub fn save_key(&mut self, value: &str, cx: &mut Context<Self>) {
        let value = value.trim();
        if value.is_empty() {
            self.key_status = "先粘贴 API Key。".into();
            cx.notify();
            return;
        }
        match secrets::save_key(value) {
            Ok(_) => {
                self.key = value.to_string();
                self.key_status = "已保存到 .env.local。".into();
            }
            Err(err) => self.key_status = format!("无法保存。{err}"),
        }
        cx.notify();
    }
}
