use std::process::Command;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, IconName, Sizable, Size};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Animation, AnimationExt, App, BoxShadow, Context, ElementId, Entity, FontWeight,
    Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, hsla, linear_color_stop, linear_gradient, px,
    rems, rgb,
};

use crate::audio::{self, SoundCue};
use crate::capture::ActiveRecording;
use crate::front::front_bundle_id;
use crate::gateway::{self, POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::hotkey::{self, HOTKEY_OPTIONS, HotkeyEvent};
use crate::paste::{self, InsertResult};
use crate::polish::{self, PolishMode, PolishOptions};
use crate::secrets::{self, KeySource};
use crate::store::{HistoryItem, Phrase, StatsSummary, Store, TriggerMode};
use crate::ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavTab {
    Overview,
    History,
    Dictionary,
    Settings,
}

impl NavTab {
    pub fn label(self) -> &'static str {
        match self {
            NavTab::Overview => "工作台",
            NavTab::History => "口述历史",
            NavTab::Dictionary => "个人词库",
            NavTab::Settings => "偏好设置",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            NavTab::Overview => "实时状态、数据概览与内置润色演练台",
            NavTab::History => "查阅、对比原文、回放录音或重新润色历史口述",
            NavTab::Dictionary => "管理专有名词与术语，转写与润色将严格保持拼写",
            NavTab::Settings => "全局快捷键、麦克风输入、AI Gateway 与文风偏好",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Transcribe,
    Polish,
    Insert,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Transcribe => "正在转写语音",
            Stage::Polish => "正在智能润色",
            Stage::Insert => "正在贴入窗口",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Stage::Transcribe => 0,
            Stage::Polish => 1,
            Stage::Insert => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Done(String),
    Copied,
    Warn(String),
    Error(String),
}

impl Notice {
    fn text(&self) -> &str {
        match self {
            Notice::Done(text) => text,
            Notice::Copied => "已复制，请手动粘贴",
            Notice::Warn(text) | Notice::Error(text) => text,
        }
    }

    fn duration(&self) -> Duration {
        match self {
            Notice::Error(_) | Notice::Warn(_) => Duration::from_millis(3600),
            _ => Duration::from_millis(2200),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Listening,
    Working(Stage),
    Message(Notice),
}

pub struct Session {
    pub phase: Phase,
    pub level: f32,
    pub wave: [f32; crate::capture::WAVE_BARS],
    pub show_dock: bool,
    pub hotkey_code: i64,
    pub trigger_mode: TriggerMode,
    pub polish_strategy: String,
    pub custom_prompt: String,
    pub space_cjk: bool,
    pub sound_feedback: bool,
    pub save_audio: bool,
    pub mic_device: String,
    pub available_mics: Vec<String>,
    pub transcribe_model: String,
    pub polish_model: String,
    pub key_status: String,
    pub gateway_test_status: Option<(bool, String)>,
    pub gateway_testing: bool,
    pub phrases: Vec<Phrase>,
    pub history: Vec<HistoryItem>,
    pub stats: StatsSummary,
    pub active_bundle: String,
    pub playing_id: Option<i64>,
    pub repolishing_id: Option<i64>,
    pub studio_mode: PolishMode,
    pub studio_output: String,
    pub studio_raw: String,
    pub studio_busy: bool,
    pub studio_dictating: bool,
    pub mic_testing: bool,
    key: String,
    store: Arc<Mutex<Store>>,
    recording: Option<ActiveRecording>,
    mic_test_stream: Option<ActiveRecording>,
    stages: Option<Receiver<Stage>>,
    generation: u64,
    note_id: u64,
    appearance: u64,
    exiting: bool,
    hide_epoch: u64,
}

impl Session {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let store = Store::open().expect("打开本机数据库");
        let (key, source) = secrets::load_key();
        let _ = (TRANSCRIBE_MODEL, POLISH_MODEL);
        let phrases = store.phrases().unwrap_or_default();
        let history = store.history().unwrap_or_default();
        let stats = store.stats();
        let show_dock = store.show_dock();
        let hotkey_code = store.hotkey_code();
        let trigger_mode = store.trigger_mode();
        let polish_strategy = store.polish_strategy();
        let custom_prompt = store.custom_prompt();
        let space_cjk = store.space_cjk();
        let sound_feedback = store.sound_feedback();
        let save_audio = store.save_audio();
        let mic_device = store.mic_device();
        let transcribe_model = store.transcribe_model();
        let polish_model = store.polish_model();
        let available_mics = crate::capture::list_input_devices();

        Self {
            phase: Phase::Idle,
            level: 0.0,
            wave: [0.0; crate::capture::WAVE_BARS],
            show_dock,
            hotkey_code,
            trigger_mode,
            polish_strategy,
            custom_prompt,
            space_cjk,
            sound_feedback,
            save_audio,
            mic_device,
            available_mics,
            transcribe_model,
            polish_model,
            key_status: key_status(&key, &source),
            gateway_test_status: None,
            gateway_testing: false,
            phrases,
            history,
            stats,
            active_bundle: String::new(),
            playing_id: None,
            repolishing_id: None,
            studio_mode: PolishMode::Prose,
            studio_output: String::new(),
            studio_raw: String::new(),
            studio_busy: false,
            studio_dictating: false,
            mic_testing: false,
            key,
            store: Arc::new(Mutex::new(store)),
            recording: None,
            mic_test_stream: None,
            stages: None,
            generation: 0,
            note_id: 0,
            appearance: 0,
            exiting: false,
            hide_epoch: 0,
        }
    }

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
                    Ok(ms) => (true, format!("连接正常 · 延迟 {ms} ms")),
                    Err(err) => (false, err),
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub fn run_studio_polish(&mut self, input_text: &str, cx: &mut Context<Self>) {
        let input_text = input_text.trim().to_string();
        if input_text.is_empty() || self.studio_busy {
            return;
        }
        if self.key.is_empty() {
            self.studio_output = "请先在「偏好设置」中配置 AI Gateway Key。".into();
            cx.notify();
            return;
        }
        let key = self.key.clone();
        let model = self.polish_model.clone();
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
                    gateway::polish_with_options(&key, &input_text, &phrases, &options, &model)
                })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.studio_busy = false;
                match result {
                    Ok(polished) => {
                        session.studio_output = polished.clone();
                        if let Ok(s) = store.lock() {
                            let _ = s.add_history(
                                "opentypeless.studio",
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
        if let Ok(store) = self.store.lock() {
            let _ = store.clear_history();
            self.history = Vec::new();
            self.stats = store.stats();
        }
        cx.notify();
    }

    pub fn watch_level(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let mut interval = Duration::from_millis(90);
                let alive = this
                    .update(cx, |session, cx| {
                        let mut changed = false;
                        if matches!(session.phase, Phase::Listening) || session.mic_testing {
                            let level = crate::capture::level();
                            let wave = crate::capture::wave();
                            if (session.level - level).abs() > 0.008 || session.wave != wave {
                                session.level = level;
                                session.wave = wave;
                                changed = true;
                            }
                            if matches!(session.phase, Phase::Listening) {
                                changed = true;
                            }
                        }
                        let now_playing = audio::playing_id();
                        if session.playing_id != now_playing {
                            session.playing_id = now_playing;
                            changed = true;
                        }
                        if let Some(receiver) = session.stages.take() {
                            let mut connected = true;
                            let mut latest = None;
                            loop {
                                match receiver.try_recv() {
                                    Ok(stage) => latest = Some(stage),
                                    Err(TryRecvError::Empty) => break,
                                    Err(TryRecvError::Disconnected) => {
                                        connected = false;
                                        break;
                                    }
                                }
                            }
                            if let Some(stage) = latest {
                                if matches!(session.phase, Phase::Working(_)) {
                                    session.phase = Phase::Working(stage);
                                    changed = true;
                                }
                            }
                            if connected {
                                session.stages = Some(receiver);
                            }
                        }
                        if changed {
                            cx.notify();
                        }
                        if matches!(session.phase, Phase::Listening | Phase::Working(_))
                            || session.mic_testing
                        {
                            interval = Duration::from_millis(20);
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
                cx.background_executor().timer(interval).await;
            }
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
            self.key_status = "输入框是空的".into();
            cx.notify();
            return;
        }
        match secrets::save_key(value) {
            Ok(_) => {
                self.key = value.to_string();
                self.key_status = "已保存到 .env.local".into();
            }
            Err(err) => self.key_status = format!("写入失败：{err}"),
        }
        cx.notify();
    }

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
        }
        cx.notify();
    }

    pub fn has_key(&self) -> bool {
        !self.key.is_empty()
    }

    pub fn elapsed(&self) -> Duration {
        self.recording
            .as_ref()
            .map(|recording| recording.elapsed())
            .unwrap_or_default()
    }

    pub fn handle_hotkey_event(&mut self, ev: HotkeyEvent, cx: &mut Context<Self>) {
        match ev {
            HotkeyEvent::Toggle => self.toggle(cx),
            HotkeyEvent::HoldStart => {
                if self.recording.is_none() && !matches!(self.phase, Phase::Working(_)) {
                    self.begin(false, cx);
                }
            }
            HotkeyEvent::HoldStop => {
                if self.recording.is_some() {
                    self.finish(cx);
                }
            }
        }
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish(cx);
        } else if !matches!(self.phase, Phase::Working(_)) {
            self.begin(false, cx);
        }
    }

    pub fn toggle_studio_dictation(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish(cx);
        } else if !matches!(self.phase, Phase::Working(_)) {
            self.begin(true, cx);
        }
    }

    pub fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        drop(recording.finish());
        self.studio_dictating = false;
        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];
        self.note(Notice::Warn("已取消录音".into()), cx);
    }

    fn begin(&mut self, for_studio: bool, cx: &mut Context<Self>) {
        if self.key.is_empty() {
            self.note(
                Notice::Error("请先在偏好设置中保存 AI Gateway Key".into()),
                cx,
            );
            return;
        }
        if let Some(stream) = self.mic_test_stream.take() {
            drop(stream.finish());
            self.mic_testing = false;
        }
        self.studio_dictating = for_studio;
        self.active_bundle = if for_studio {
            "opentypeless.studio".to_string()
        } else {
            front_bundle_id()
        };
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];
        self.exiting = false;
        self.appearance = self.appearance.wrapping_add(1);
        self.phase = Phase::Listening;
        self.sync_voice(cx);
        if self.sound_feedback {
            audio::play_cue(SoundCue::Start);
        }
        cx.notify();

        let preferred = if self.mic_device.is_empty() {
            None
        } else {
            Some(self.mic_device.as_str())
        };
        match ActiveRecording::start_with_device(preferred) {
            Ok(recording) => {
                self.recording = Some(recording);
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_secs(120))
                        .await;
                    let _ = this.update(cx, |session, cx| {
                        if session.generation == generation && session.recording.is_some() {
                            session.finish(cx);
                        }
                    });
                })
                .detach();
            }
            Err(err) => {
                self.studio_dictating = false;
                if self.sound_feedback {
                    audio::play_cue(SoundCue::Error);
                }
                self.note(Notice::Error(err), cx);
            }
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        let for_studio = self.studio_dictating;
        self.studio_dictating = false;
        if recording.elapsed() < Duration::from_millis(300) {
            drop(recording.finish());
            self.note(Notice::Warn("录音时间太短了".into()), cx);
            return;
        }
        if self.sound_feedback {
            audio::play_cue(SoundCue::Stop);
        }
        let recorded = recording.finish();
        let duration_ms = (recorded.duration_secs() * 1000.0).round() as i64;
        let wav = recorded.to_wav();
        let bundle_id = if for_studio {
            "opentypeless.studio".to_string()
        } else if !self.active_bundle.is_empty() {
            self.active_bundle.clone()
        } else {
            front_bundle_id()
        };
        let phrases = self
            .store
            .lock()
            .map(|store| store.phrase_list())
            .unwrap_or_default();
        let key = self.key.clone();
        let transcribe_model = self.transcribe_model.clone();
        let polish_model = self.polish_model.clone();
        let resolved_mode = if for_studio {
            Some(self.studio_mode)
        } else {
            polish::resolve_mode(&self.polish_strategy, &bundle_id)
        };
        let polish_opts = resolved_mode.map(|mode| PolishOptions {
            mode,
            custom_prompt: self.custom_prompt.clone(),
            space_cjk: self.space_cjk,
        });
        let mode_tag = resolved_mode
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "raw".to_string());
        let wav_for_task = wav.clone();

        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];

        let (sender, receiver) = mpsc::channel::<Stage>();
        self.stages = Some(receiver);
        self.exiting = false;
        self.appearance = self.appearance.wrapping_add(1);
        self.phase = Phase::Working(Stage::Transcribe);
        self.sync_voice(cx);
        cx.notify();

        let store = Arc::clone(&self.store);
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    let stage = move |stage: Stage| {
                        let _ = sender.send(stage);
                    };
                    run_pipeline(
                        &key,
                        &wav_for_task,
                        &phrases,
                        polish_opts.as_ref(),
                        &transcribe_model,
                        &polish_model,
                        &stage,
                    )
                })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.deliver(
                    outcome,
                    &bundle_id,
                    &wav,
                    duration_ms,
                    &mode_tag,
                    for_studio,
                    &store,
                    cx,
                );
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_arguments)]
    fn deliver(
        &mut self,
        outcome: Outcome,
        bundle_id: &str,
        wav: &[u8],
        duration_ms: i64,
        mode_tag: &str,
        for_studio: bool,
        store: &Arc<Mutex<Store>>,
        cx: &mut Context<Self>,
    ) {
        self.stages = None;
        let (notice, saved) = match outcome {
            Outcome::Failed(message) => {
                if self.sound_feedback {
                    audio::play_cue(SoundCue::Error);
                }
                (Notice::Error(message), None)
            }
            Outcome::Ready {
                transcript,
                paste_text,
                archive_polished,
                polish_failed,
            } => {
                if self.sound_feedback {
                    audio::play_cue(SoundCue::Done);
                }
                let notice = if for_studio {
                    self.studio_raw = transcript.clone();
                    self.studio_output = paste_text.clone();
                    Notice::Done(format!("已转写到演练台 · {} 字", paste_text.chars().count()))
                } else {
                    let (inserted, restore) = paste::insert_text(&paste_text);
                    if let Some(restore) = restore {
                        cx.spawn(async move |_this, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(280))
                                .await;
                            let _ = cx.update(|_cx| restore.apply());
                        })
                        .detach();
                    }
                    if polish_failed {
                        match inserted {
                            InsertResult::Pasted => Notice::Warn("润色失败，已贴入转写原文".into()),
                            InsertResult::Copied => Notice::Warn("润色失败，原文已复制".into()),
                        }
                    } else {
                        match inserted {
                            InsertResult::Pasted => {
                                Notice::Done(format!("已贴入 · {} 字", paste_text.chars().count()))
                            }
                            InsertResult::Copied => Notice::Copied,
                        }
                    }
                };
                (notice, Some((transcript, archive_polished)))
            }
        };
        if let Some((transcript, polished)) = saved {
            if let Ok(store) = store.lock() {
                if let Ok(row_id) =
                    store.add_history(bundle_id, &transcript, &polished, duration_ms, mode_tag)
                {
                    if self.save_audio && !wav.is_empty() {
                        let _ = store.save_audio_file(row_id, wav);
                    }
                }
                self.history = store.history().unwrap_or_default();
                self.stats = store.stats();
            }
        }
        self.note(notice, cx);
    }

    fn note(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let duration = notice.duration();
        self.note_id = self.note_id.wrapping_add(1);
        let note_id = self.note_id;
        self.exiting = false;
        self.appearance = self.appearance.wrapping_add(1);
        self.phase = Phase::Message(notice);
        self.sync_voice(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            let _ = this.update(cx, |session, cx| {
                if session.note_id == note_id && matches!(session.phase, Phase::Message(_)) {
                    session.begin_exit(cx);
                }
            });
        })
        .detach();
    }

    fn begin_exit(&mut self, cx: &mut Context<Self>) {
        self.exiting = true;
        self.appearance = self.appearance.wrapping_add(1);
        self.hide_epoch = self.hide_epoch.wrapping_add(1);
        let epoch = self.hide_epoch;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(170))
                .await;
            let _ = this.update(cx, |session, _cx| {
                if session.hide_epoch == epoch {
                    session.exiting = false;
                    session.phase = Phase::Idle;
                    session.level = 0.0;
                    crate::overlay::set_voice_visible(false);
                }
            });
        })
        .detach();
    }

    fn sync_voice(&mut self, cx: &mut Context<Self>) {
        self.hide_epoch = self.hide_epoch.wrapping_add(1);
        if matches!(self.phase, Phase::Idle) {
            let epoch = self.hide_epoch;
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let _ = this.update(cx, |session, _cx| {
                    if session.hide_epoch == epoch && matches!(session.phase, Phase::Idle) {
                        crate::overlay::set_voice_visible(false);
                    }
                });
            })
            .detach();
        } else {
            crate::overlay::set_voice_visible(true);
        }
    }
}

enum Outcome {
    Failed(String),
    Ready {
        transcript: String,
        paste_text: String,
        archive_polished: String,
        polish_failed: bool,
    },
}

fn run_pipeline(
    key: &str,
    wav: &[u8],
    phrases: &[String],
    polish_opts: Option<&PolishOptions>,
    transcribe_model: &str,
    polish_model: &str,
    stage: &(dyn Fn(Stage) + Send + Sync),
) -> Outcome {
    stage(Stage::Transcribe);
    let transcript = match gateway::transcribe_with_model(key, wav, phrases, transcribe_model) {
        Ok(text) => text,
        Err(err) => return Outcome::Failed(err),
    };
    if transcript.trim().is_empty() {
        return Outcome::Failed("没有识别到语音内容".into());
    }
    let (paste_text, archive_polished, polish_failed) = match polish_opts {
        Some(opts) => {
            stage(Stage::Polish);
            match gateway::polish_with_options(key, &transcript, phrases, opts, polish_model) {
                Ok(text) => (text.clone(), text, false),
                Err(_) => (transcript.clone(), String::new(), true),
            }
        }
        None => (transcript.clone(), String::new(), false),
    };
    stage(Stage::Insert);
    Outcome::Ready {
        transcript,
        paste_text,
        archive_polished,
        polish_failed,
    }
}

fn key_status(key: &str, source: &KeySource) -> String {
    if key.is_empty() {
        return "尚未配置 Key".into();
    }
    match source {
        KeySource::File(_) => "已从 .env.local 加载".into(),
        KeySource::Missing => "尚未配置 Key".into(),
    }
}

fn mmss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn format_duration_ms(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms as f32 / 1000.0;
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let m = (secs as u64) / 60;
        let s = (secs as u64) % 60;
        format!("{m}m {s:02}s")
    }
}

fn format_relative_time(ts: i64) -> String {
    if ts <= 0 {
        return "早期记录".into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(ts);
    let diff = (now - ts).max(0);
    if diff < 45 {
        "刚刚".into()
    } else if diff < 3600 {
        format!("{} 分钟前", diff / 60)
    } else if diff < 86_400 {
        format!("{} 小时前", diff / 3600)
    } else {
        format!("{} 天前", diff / 86_400)
    }
}

fn ellipsize(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

pub struct SettingsView {
    session: Entity<Session>,
    pub active_tab: NavTab,
    key_input: Entity<InputState>,
    phrase_input: Entity<InputState>,
    studio_input: Entity<InputState>,
    history_search_input: Entity<InputState>,
    custom_prompt_input: Entity<InputState>,
    key_editing: bool,
    pub history_mode_filter: String,
    pub expanded_raw_ids: Vec<i64>,
    pub copied_token: Option<String>,
    pub confirm_clear_history: bool,
}

impl SettingsView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: Entity<Session>,
        key_input: Entity<InputState>,
        phrase_input: Entity<InputState>,
        studio_input: Entity<InputState>,
        history_search_input: Entity<InputState>,
        custom_prompt_input: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let key_editing = !session.read(cx).has_key();
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&key_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value();
                view.session
                    .update(cx, |session, cx| session.save_key(value.as_ref(), cx));
                if view.session.read(cx).has_key() {
                    view.key_editing = false;
                }
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&phrase_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value().to_string();
                if !value.trim().is_empty() {
                    view.session
                        .update(cx, |session, cx| session.add_phrase(&value, cx));
                }
            }
        })
        .detach();
        cx.subscribe(&history_search_input, |_, _, _: &InputEvent, cx| {
            cx.notify();
        })
        .detach();
        cx.subscribe(&custom_prompt_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value().to_string();
                view.session
                    .update(cx, |session, cx| session.set_custom_prompt(&value, cx));
            }
        })
        .detach();

        Self {
            session,
            active_tab: NavTab::Overview,
            key_input,
            phrase_input,
            studio_input,
            history_search_input,
            custom_prompt_input,
            key_editing,
            history_mode_filter: "all".into(),
            expanded_raw_ids: Vec::new(),
            copied_token: None,
            confirm_clear_history: false,
        }
    }

    pub fn flash_copied(&mut self, token: impl Into<String>, cx: &mut Context<Self>) {
        let token = token.into();
        self.copied_token = Some(token.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1600))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.copied_token.as_deref() == Some(token.as_str()) {
                    view.copied_token = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn toggle_raw_expand(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(pos) = self.expanded_raw_ids.iter().position(|x| *x == id) {
            self.expanded_raw_ids.remove(pos);
        } else {
            self.expanded_raw_ids.push(id);
        }
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let view = cx.entity();
        let session_handle = self.session.clone();

        let (phase, elapsed, phrases_len, history_len, has_key, hotkey_code, trigger_mode) = {
            let s = self.session.read(cx);
            (
                s.phase.clone(),
                s.elapsed(),
                s.phrases.len(),
                s.history.len(),
                s.has_key(),
                s.hotkey_code,
                s.trigger_mode,
            )
        };

        let page_content = match active_tab {
            NavTab::Overview => self.render_overview(cx).into_any_element(),
            NavTab::History => self.render_history(cx).into_any_element(),
            NavTab::Dictionary => self.render_dictionary(cx).into_any_element(),
            NavTab::Settings => self.render_settings(cx).into_any_element(),
        };

        div()
            .id("workspace-root")
            .flex()
            .size_full()
            .bg(ui::canvas_bg(cx))
            .text_color(ui::fg(cx))
            .child(
                // Left Structural Sidebar (macOS System Settings / Voice Memos style)
                div()
                    .w(px(216.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .bg(ui::sidebar_bg(cx))
                    .border_r_1()
                    .border_color(ui::hairline(cx))
                    .pt(px(38.0))
                    .pb(px(14.0))
                    .px(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .child(
                                // App Brand Header
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px(px(8.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(9.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .size(px(28.0))
                                                    .rounded(px(8.0))
                                                    .bg(ui::accent(cx))
                                                    .child(
                                                        Icon::new(LucideIcon::AudioWaveform)
                                                            .size(px(15.0))
                                                            .text_color(rgb(0xffffff)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.875))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child("OpenTypeless"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.65625))
                                                            .text_color(ui::fg3(cx))
                                                            .child("AI 语音口述与润色"),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                // Navigation Items
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.0))
                                    .child(nav_item(
                                        "nav-overview",
                                        LucideIcon::LayoutDashboard,
                                        NavTab::Overview,
                                        active_tab == NavTab::Overview,
                                        None,
                                        view.clone(),
                                        cx,
                                    ))
                                    .child(nav_item(
                                        "nav-history",
                                        LucideIcon::Clock,
                                        NavTab::History,
                                        active_tab == NavTab::History,
                                        if history_len > 0 {
                                            Some(history_len.to_string())
                                        } else {
                                            None
                                        },
                                        view.clone(),
                                        cx,
                                    ))
                                    .child(nav_item(
                                        "nav-dict",
                                        LucideIcon::BookOpen,
                                        NavTab::Dictionary,
                                        active_tab == NavTab::Dictionary,
                                        if phrases_len > 0 {
                                            Some(phrases_len.to_string())
                                        } else {
                                            None
                                        },
                                        view.clone(),
                                        cx,
                                    ))
                                    .child(nav_item(
                                        "nav-settings",
                                        LucideIcon::SlidersHorizontal,
                                        NavTab::Settings,
                                        active_tab == NavTab::Settings,
                                        if !has_key {
                                            Some("需配置".into())
                                        } else {
                                            None
                                        },
                                        view.clone(),
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        // Sidebar Footer: Quick Dictation Status & Shortcut Card
                        sidebar_status_card(
                            &phase,
                            elapsed,
                            hotkey_code,
                            trigger_mode,
                            has_key,
                            session_handle,
                            cx,
                        ),
                    ),
            )
            .child(
                // Right Main Content Area
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_w(px(0.0))
                    .child(
                        // Sticky Top Content Header
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(28.0))
                            .pt(px(24.0))
                            .pb(px(14.0))
                            .border_b_1()
                            .border_color(ui::hairline(cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(rems(1.1875))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(active_tab.label()),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.75))
                                            .text_color(ui::fg2(cx))
                                            .child(active_tab.subtitle()),
                                    ),
                            )
                            .child(status_pill(&phase, elapsed, cx)),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("scroll-{active_tab:?}")))
                            .flex_1()
                            .overflow_y_scroll()
                            .px(px(28.0))
                            .py(px(22.0))
                            .child(page_content),
                    ),
            )
    }
}

impl SettingsView {
    fn render_overview(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let studio_input = self.studio_input.clone();
        let copied_token = self.copied_token.clone();

        let (
            phase,
            elapsed,
            wave,
            has_key,
            hotkey_code,
            trigger_mode,
            polish_strategy,
            stats,
            phrases_len,
            recent_history,
            studio_mode,
            studio_output,
            studio_raw,
            studio_busy,
            studio_dictating,
            playing_id,
        ) = {
            let s = self.session.read(cx);
            (
                s.phase.clone(),
                s.elapsed(),
                s.wave,
                s.has_key(),
                s.hotkey_code,
                s.trigger_mode,
                s.polish_strategy.clone(),
                s.stats.clone(),
                s.phrases.len(),
                s.history.iter().take(3).cloned().collect::<Vec<_>>(),
                s.studio_mode,
                s.studio_output.clone(),
                s.studio_raw.clone(),
                s.studio_busy,
                s.studio_dictating,
                s.playing_id,
            )
        };

        let expanded_ids = self.expanded_raw_ids.clone();
        let repolishing_id = self.session.read(cx).repolishing_id;
        let audio_dir = Store::data_dir().unwrap_or_default().join("audio");

        let strategy_label = match polish_strategy.as_str() {
            "prose" => "固定：流畅正文",
            "literal" => "固定：代码与终端",
            "email" => "固定：商务邮件",
            "chat" => "固定：即时通讯",
            "raw" => "仅语音转写",
            _ => "智能感知前台应用",
        };

        div()
            .flex()
            .flex_col()
            .gap(px(22.0))
            // 1. Hero Action & Live Status Card
            .child(hero_banner(
                &phase,
                elapsed,
                wave,
                has_key,
                hotkey_code,
                trigger_mode,
                strategy_label,
                session_handle.clone(),
                view.clone(),
                cx,
            ))
            // 2. 4-Column Real Metrics Strip
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(metric_card(
                        "累计口述",
                        format!("{}", stats.total_sessions),
                        if stats.total_duration_ms > 0 {
                            format!("累计时长 {}", format_duration_ms(stats.total_duration_ms))
                        } else {
                            "随时按键开始口述".into()
                        },
                        LucideIcon::Mic,
                        ui::accent(cx),
                        cx,
                    ))
                    .child(metric_card(
                        "累计产出字数",
                        format!("{}", stats.total_chars),
                        if stats.saved_minutes >= 0.1 {
                            format!("约节省 {:.1} 分钟打字时间", stats.saved_minutes)
                        } else {
                            "自动去除口癖与标点整理".into()
                        },
                        LucideIcon::Sparkles,
                        ui::ok(cx),
                        cx,
                    ))
                    .child(metric_card(
                        "平均口述语速",
                        if stats.avg_cpm > 0 {
                            format!("{}", stats.avg_cpm)
                        } else {
                            "—".into()
                        },
                        "字 / 分钟（含中英混排）".into(),
                        LucideIcon::Gauge,
                        ui::warn(cx),
                        cx,
                    ))
                    .child(metric_card(
                        "个人词库术语",
                        format!("{phrases_len}"),
                        "转写与润色双重拼写锁定".into(),
                        LucideIcon::BookOpen,
                        ui::accent(cx),
                        cx,
                    )),
            )
            // 3. Built-in Live Studio / Polish Playground (One Live Thing)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                ui::section_label(
                                    "润色与试音演练台",
                                    Some("无需切换窗口，直接测试语音转写或体验不同场景的 AI 润色效果".into()),
                                )
                                .text_color(ui::fg2(cx)),
                            ),
                    )
                    .child(
                        ui::card(cx)
                            .p(px(16.0))
                            .gap(px(12.0))
                            // Mode selector & quick sample chips
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .flex_wrap()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_size(rems(0.71875))
                                                    .text_color(ui::fg3(cx))
                                                    .child("润色模式："),
                                            )
                                            .children(
                                                [
                                                    PolishMode::Prose,
                                                    PolishMode::Literal,
                                                    PolishMode::Email,
                                                    PolishMode::Chat,
                                                ]
                                                .into_iter()
                                                .map(|m| {
                                                    let selected = studio_mode == m;
                                                    let s = session_handle.clone();
                                                    mode_chip(
                                                        format!("studio-mode-{}", m.as_str()),
                                                        m.label(),
                                                        selected,
                                                        move |_, cx| {
                                                            s.update(cx, |session, cx| {
                                                                session.studio_mode = m;
                                                                cx.notify();
                                                            });
                                                        },
                                                        cx,
                                                    )
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_size(rems(0.6875))
                                                    .text_color(ui::fg3(cx))
                                                    .child("示例草稿："),
                                            )
                                            .children(
                                                [
                                                    (
                                                        "纪要分点",
                                                        PolishMode::Prose,
                                                        "嗯那个今天下午我们主要对齐三件事吧，第一是先把OpenTypeless的侧边栏做好，然后第二件事是把历史记录的录音回放加上，不对，还要加个原文对比，最后是明天上午十点大家一起过一下测试",
                                                    ),
                                                    (
                                                        "代码终端",
                                                        PolishMode::Literal,
                                                        "呃帮我在终端跑一下 cargo test 然后看看 src 斜杠 session 点 rs 里面有没有 warning 那个 unused warning 要清掉",
                                                    ),
                                                    (
                                                        "商务邮件",
                                                        PolishMode::Email,
                                                        "喂王总你好，那个关于下周三的产品评审会，我想确认一下两个点，一个是参会名单定了吗，另一个是演示环境需要我们这边提前准备吗，麻烦今天下班前回复一下谢谢",
                                                    ),
                                                ]
                                                .into_iter()
                                                .enumerate()
                                                .map(|(idx, (title, target_mode, sample_text))| {
                                                    let input = studio_input.clone();
                                                    let s = session_handle.clone();
                                                    div()
                                                        .id(SharedString::from(format!("sample-{idx}")))
                                                        .px(px(8.0))
                                                        .py(px(3.0))
                                                        .rounded(px(6.0))
                                                        .bg(ui::well_bg(cx))
                                                        .cursor_pointer()
                                                        .hover(|st| st.bg(ui::card_hover(cx)))
                                                        .text_size(rems(0.6875))
                                                        .text_color(ui::fg2(cx))
                                                        .on_click(move |_, window, cx| {
                                                            input.update(cx, |inp, cx| {
                                                                inp.set_value(sample_text, window, cx);
                                                            });
                                                            s.update(cx, |session, cx| {
                                                                session.studio_mode = target_mode;
                                                                cx.notify();
                                                            });
                                                        })
                                                        .child(title)
                                                }),
                                            ),
                                    ),
                            )
                            // Input row + action buttons
                            .child(
                                div()
                                    .text_size(rems(0.71875))
                                    .text_color(ui::fg2(cx))
                                    .child(studio_mode.description()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .child(Input::new(&studio_input).w_full()),
                                    )
                                    .child({
                                        let s = session_handle.clone();
                                        let is_rec = matches!(phase, Phase::Listening) && studio_dictating;
                                        button(
                                            "studio-mic-btn",
                                            if is_rec { "停止试音" } else { "语音试音" },
                                            false,
                                            move |_, cx| {
                                                s.update(cx, |session, cx| {
                                                    session.toggle_studio_dictation(cx);
                                                });
                                            },
                                        )
                                    })
                                    .child({
                                        let s = session_handle.clone();
                                        let inp = studio_input.clone();
                                        button(
                                            "studio-run-btn",
                                            if studio_busy { "正在润色…" } else { "立即润色" },
                                            true,
                                            move |_, cx| {
                                                let val = inp.read(cx).value().to_string();
                                                s.update(cx, |session, cx| {
                                                    session.run_studio_polish(&val, cx);
                                                });
                                            },
                                        )
                                    }),
                            )
                            // Output comparison box (shown whenever there is output or busy)
                            .when(!studio_output.is_empty() || studio_busy, |card| {
                                let out_copy = studio_output.clone();
                                let v = view.clone();
                                let is_copied = copied_token.as_deref() == Some("studio-out");
                                card.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.0))
                                        .p(px(12.0))
                                        .rounded(px(9.0))
                                        .bg(ui::well_bg(cx))
                                        .border_1()
                                        .border_color(ui::hairline(cx))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(8.0))
                                                        .child(ui::pill(
                                                            format!("{} · 润色结果", studio_mode.label()),
                                                            ui::accent(cx),
                                                            ui::accent_soft(cx),
                                                        ))
                                                        .when(!studio_output.is_empty(), |row| {
                                                            row.child(
                                                                div()
                                                                    .text_size(rems(0.6875))
                                                                    .text_color(ui::fg3(cx))
                                                                    .child(format!(
                                                                        "{} 字",
                                                                        studio_output.chars().count()
                                                                    )),
                                                            )
                                                        }),
                                                )
                                                .when(!studio_output.is_empty() && !studio_busy, |row| {
                                                    row.child(
                                                        small_action_pill(
                                                            "copy-studio-out",
                                                            if is_copied {
                                                                IconName::Check
                                                            } else {
                                                                IconName::Copy
                                                            },
                                                            if is_copied { "已复制" } else { "复制结果" },
                                                            move |_, cx| {
                                                                if paste::copy_to_clipboard(&out_copy) {
                                                                    v.update(cx, |view, cx| {
                                                                        view.flash_copied("studio-out", cx);
                                                                    });
                                                                }
                                                            },
                                                            cx,
                                                        ),
                                                    )
                                                }),
                                        )
                                        .when(!studio_raw.is_empty() && studio_raw != studio_output, |box_div| {
                                            box_div.child(
                                                div()
                                                    .text_size(rems(0.71875))
                                                    .text_color(ui::fg3(cx))
                                                    .child(format!("原文：{}", ellipsize(&studio_raw, 140))),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_size(rems(0.84375))
                                                .line_height(rems(1.35))
                                                .text_color(ui::fg(cx))
                                                .child(if studio_busy {
                                                    "正在通过 Vercel AI Gateway 润色文本…".to_string()
                                                } else {
                                                    studio_output
                                                }),
                                        ),
                                )
                            }),
                    ),
            )
            // 4. Recent Dictation History Preview
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(ui::section_label("最近口述", None).text_color(ui::fg2(cx)))
                            .when(!recent_history.is_empty(), |row| {
                                let v = view.clone();
                                row.child(
                                    div()
                                        .id("goto-all-history")
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .cursor_pointer()
                                        .text_size(rems(0.71875))
                                        .text_color(ui::accent(cx))
                                        .on_click(move |_, _, cx| {
                                            v.update(cx, |view, cx| {
                                                view.active_tab = NavTab::History;
                                                cx.notify();
                                            });
                                        })
                                        .child("查看全部历史")
                                        .child(
                                            Icon::new(IconName::ChevronRight)
                                                .size(px(12.0))
                                                .text_color(ui::accent(cx)),
                                        ),
                                )
                            }),
                    )
                    .child(
                        ui::card(cx)
                            .when(recent_history.is_empty(), |card| {
                                card.child(empty_state(
                                    IconName::FileText,
                                    "还没有口述记录",
                                    "按快捷键说一句话，或者在上方演练台点击「立即润色」，结果会自动归档到这里。",
                                    cx,
                                ))
                            })
                            .children({
                                let count = recent_history.len();
                                let s = session_handle.clone();
                                let v = view.clone();
                                let copied = copied_token.clone();
                                let exp_ids = expanded_ids.clone();
                                let a_dir = audio_dir.clone();
                                recent_history.into_iter().enumerate().map(move |(idx, item)| {
                                    let has_audio = a_dir.join(format!("{}.wav", item.id)).is_file();
                                    let is_playing = playing_id == Some(item.id);
                                    let is_expanded = exp_ids.contains(&item.id);
                                    let is_repolishing = repolishing_id == Some(item.id);
                                    history_card_row(
                                        item,
                                        idx + 1 == count,
                                        has_audio,
                                        is_playing,
                                        is_expanded,
                                        is_repolishing,
                                        copied.clone(),
                                        s.clone(),
                                        v.clone(),
                                        cx,
                                    )
                                })
                            }),
                    ),
            )
    }

    fn render_history(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let search_input = self.history_search_input.clone();
        let query = search_input.read(cx).value().trim().to_lowercase();
        let mode_filter = self.history_mode_filter.clone();
        let confirm_clear = self.confirm_clear_history;
        let copied_token = self.copied_token.clone();

        let (all_history, playing_id, repolishing_id) = {
            let s = self.session.read(cx);
            (s.history.clone(), s.playing_id, s.repolishing_id)
        };
        let expanded_ids = self.expanded_raw_ids.clone();
        let audio_dir = Store::data_dir().unwrap_or_default().join("audio");

        let filtered: Vec<HistoryItem> = all_history
            .iter()
            .filter(|item| {
                let item_mode = if item.mode.is_empty() {
                    polish::mode_for_bundle(&item.bundle_id).as_str()
                } else {
                    item.mode.as_str()
                };
                let mode_ok = mode_filter == "all" || item_mode == mode_filter;
                if !mode_ok {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                let app_name = polish::friendly_app_name(&item.bundle_id).to_lowercase();
                item.transcript.to_lowercase().contains(&query)
                    || item.polished.to_lowercase().contains(&query)
                    || app_name.contains(&query)
            })
            .cloned()
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Search & Filter Toolbar
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(260.0))
                            .child(Input::new(&search_input).w_full()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .children(
                                [
                                    ("all", "全部"),
                                    ("prose", "正文"),
                                    ("literal", "代码"),
                                    ("email", "邮件"),
                                    ("chat", "聊天"),
                                ]
                                .into_iter()
                                .map(|(code, label)| {
                                    let active = mode_filter == code;
                                    let v = view.clone();
                                    mode_chip(
                                        format!("hist-filter-{code}"),
                                        label,
                                        active,
                                        move |_, cx| {
                                            v.update(cx, |view, cx| {
                                                view.history_mode_filter = code.to_string();
                                                cx.notify();
                                            });
                                        },
                                        cx,
                                    )
                                }),
                            )
                            .when(!all_history.is_empty(), |row| {
                                let s = session_handle.clone();
                                let v = view.clone();
                                let v_cancel = view.clone();
                                if confirm_clear {
                                    row.child(button(
                                        "confirm-clear-btn",
                                        "确认清空全部",
                                        true,
                                        move |_, cx| {
                                            s.update(cx, |session, cx| session.clear_history(cx));
                                            v.update(cx, |view, cx| {
                                                view.confirm_clear_history = false;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                    .child(button(
                                        "cancel-clear-btn",
                                        "取消",
                                        false,
                                        move |_, cx| {
                                            v_cancel.update(cx, |view, cx| {
                                                view.confirm_clear_history = false;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                } else {
                                    row.child(button(
                                        "ask-clear-btn",
                                        "清空记录",
                                        false,
                                        move |_, cx| {
                                            v.update(cx, |view, cx| {
                                                view.confirm_clear_history = true;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                }
                            }),
                    ),
            )
            // Filtered History List Card
            .child(
                ui::card(cx)
                    .when(filtered.is_empty(), |card| {
                        card.child(empty_state(
                            IconName::Search,
                            if all_history.is_empty() {
                                "还没有任何口述记录"
                            } else {
                                "没有匹配的口述记录"
                            },
                            if all_history.is_empty() {
                                "使用快捷键在任意应用中口述，所有转写原文、润色文本与录音都会保存在本机。"
                            } else {
                                "尝试切换上方的模式筛选或修改搜索关键词。"
                            },
                            cx,
                        ))
                    })
                    .children({
                        let count = filtered.len();
                        let s = session_handle.clone();
                        let v = view.clone();
                        let exp_ids = expanded_ids.clone();
                        let a_dir = audio_dir.clone();
                        filtered.into_iter().enumerate().map(move |(idx, item)| {
                            let has_audio = a_dir.join(format!("{}.wav", item.id)).is_file();
                            let is_playing = playing_id == Some(item.id);
                            let is_expanded = exp_ids.contains(&item.id);
                            let is_repolishing = repolishing_id == Some(item.id);
                            history_card_row(
                                item,
                                idx + 1 == count,
                                has_audio,
                                is_playing,
                                is_expanded,
                                is_repolishing,
                                copied_token.clone(),
                                s.clone(),
                                v.clone(),
                                cx,
                            )
                        })
                    }),
            )
    }
}

const PRESET_PHRASES: &[&str] = &[
    "OpenTypeless",
    "GPUI",
    "Rust",
    "TypeScript",
    "Next.js",
    "Vercel",
    "DeepSeek",
    "Claude",
    "Cursor",
    "Xcode",
    "macOS",
    "GitHub",
    "API Key",
    "Prompt",
];

impl SettingsView {
    fn render_dictionary(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let phrase_input = self.phrase_input.clone();
        let copied_token = self.copied_token.clone();
        let phrases = self.session.read(cx).phrases.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            // 1. Add Phrase & Batch Toolbar Card
            .child(
                ui::card(cx)
                    .p(px(16.0))
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(rems(0.84375))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("添加专属词条"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.71875))
                                            .text_color(ui::fg2(cx))
                                            .child("支持一次输入多个词条（以逗号或换行分隔），转写声学模型与润色大模型会同时锁定拼写。"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child({
                                        let s = session_handle.clone();
                                        let v = view.clone();
                                        button(
                                            "import-phrases-clipboard",
                                            "从剪贴板导入",
                                            false,
                                            move |_, cx| {
                                                if let Some(clip) = paste::read_from_clipboard() {
                                                    s.update(cx, |session, cx| {
                                                        session.add_phrase(&clip, cx);
                                                    });
                                                    v.update(cx, |view, cx| {
                                                        view.flash_copied("dict-imported", cx);
                                                    });
                                                }
                                            },
                                        )
                                    })
                                    .when(!phrases.is_empty(), |row| {
                                        let all_text = phrases
                                            .iter()
                                            .map(|p| p.phrase.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        let v = view.clone();
                                        let is_copied =
                                            copied_token.as_deref() == Some("dict-exported");
                                        row.child(button(
                                            "export-phrases-clipboard",
                                            if is_copied { "已复制全部词条" } else { "导出全部" },
                                            false,
                                            move |_, cx| {
                                                if paste::copy_to_clipboard(&all_text) {
                                                    v.update(cx, |view, cx| {
                                                        view.flash_copied("dict-exported", cx);
                                                    });
                                                }
                                            },
                                        ))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(Input::new(&phrase_input).w_full()),
                            )
                            .child({
                                let input = phrase_input.clone();
                                let target = session_handle.clone();
                                button("add-phrase-btn", "添加词条", true, move |window, cx| {
                                    let value = input.read(cx).value().to_string();
                                    target.update(cx, |session, cx| session.add_phrase(&value, cx));
                                    input.update(cx, |inp, cx| inp.set_value("", window, cx));
                                })
                            }),
                    )
                    // Preset quick-add chips
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(rems(0.6875))
                                    .text_color(ui::fg3(cx))
                                    .child("常用推荐："),
                            )
                            .children(PRESET_PHRASES.iter().enumerate().map(|(idx, word)| {
                                let word_str = *word;
                                let exists = phrases.iter().any(|p| p.phrase.eq_ignore_ascii_case(word_str));
                                let s = session_handle.clone();
                                div()
                                    .id(SharedString::from(format!("preset-phrase-{idx}")))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(8.0))
                                    .py(px(3.0))
                                    .rounded(px(6.0))
                                    .bg(if exists {
                                        ui::accent_soft(cx)
                                    } else {
                                        ui::well_bg(cx)
                                    })
                                    .text_size(rems(0.6875))
                                    .text_color(if exists {
                                        ui::accent(cx)
                                    } else {
                                        ui::fg2(cx)
                                    })
                                    .when(!exists, |chip| {
                                        chip.cursor_pointer()
                                            .hover(|st| st.bg(ui::card_hover(cx)))
                                            .on_click(move |_, _, cx| {
                                                s.update(cx, |session, cx| {
                                                    session.add_phrase(word_str, cx);
                                                });
                                            })
                                    })
                                    .child(if exists {
                                        format!("✓ {word_str}")
                                    } else {
                                        format!("+ {word_str}")
                                    })
                            })),
                    ),
            )
            // 2. Active Glossary List
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        ui::section_label(
                            "已启用词条",
                            Some(format!("共 {} 项", phrases.len())),
                        )
                        .text_color(ui::fg2(cx))
                        .justify_between(),
                    )
                    .child(
                        ui::card(cx)
                            .when(phrases.is_empty(), |card| {
                                card.child(empty_state(
                                    IconName::BookOpen,
                                    "词库还是空的",
                                    "点击上方推荐词条或手动输入产品名、人名、技术术语，防止同音字误识别。",
                                    cx,
                                ))
                            })
                            .children({
                                let count = phrases.len();
                                let session = session_handle.clone();
                                let hairline = ui::hairline(cx);
                                let hover = ui::card_hover(cx);
                                let dim = ui::fg2(cx);
                                phrases.into_iter().enumerate().map(move |(index, phrase)| {
                                    phrase_row(
                                        phrase.id,
                                        phrase.phrase,
                                        index + 1 == count,
                                        session.clone(),
                                        hairline,
                                        hover,
                                        dim,
                                    )
                                })
                            }),
                    ),
            )
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let key_input = self.key_input.clone();
        let custom_prompt_input = self.custom_prompt_input.clone();
        let key_path = secrets::env_path().display().to_string();
        let data_dir = Store::data_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~/Library/Application Support/OpenTypeless".into());

        let (
            has_key,
            key_status,
            gateway_test_status,
            gateway_testing,
            hotkey_code,
            trigger_mode,
            polish_strategy,
            custom_prompt,
            space_cjk,
            sound_feedback,
            save_audio,
            mic_device,
            available_mics,
            mic_testing,
            wave,
            show_dock,
            transcribe_model,
            polish_model,
        ) = {
            let s = self.session.read(cx);
            (
                s.has_key(),
                s.key_status.clone(),
                s.gateway_test_status.clone(),
                s.gateway_testing,
                s.hotkey_code,
                s.trigger_mode,
                s.polish_strategy.clone(),
                s.custom_prompt.clone(),
                s.space_cjk,
                s.sound_feedback,
                s.save_audio,
                s.mic_device.clone(),
                s.available_mics.clone(),
                s.mic_testing,
                s.wave,
                s.show_dock,
                s.transcribe_model.clone(),
                s.polish_model.clone(),
            )
        };

        let key_editing = self.key_editing || !has_key;
        let ax_trusted = hotkey::is_accessibility_trusted() || hotkey::is_tap_active();

        div()
            .flex()
            .flex_col()
            .gap(px(22.0))
            // 1. Hotkey & Accessibility Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("快捷键与触发方式", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Trigger Key Row
                            .child(
                                setting_row(
                                    "全局口述快捷键",
                                    "在任意应用中按下此按键即可唤起口述小条",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .children(HOTKEY_OPTIONS.iter().map(|(code, label, badge)| {
                                            let code_val = *code;
                                            let active = hotkey_code == code_val;
                                            let s = session_handle.clone();
                                            mode_chip(
                                                format!("hk-{code_val}"),
                                                format!("{label} ({badge})"),
                                                active,
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.set_hotkey_code(code_val, cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        })),
                                ),
                            )
                            // Trigger Mode Row
                            .child(
                                setting_row(
                                    "触发手势模式",
                                    "选择单击切换或按住说话（松开即自动转写贴入）",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .children(
                                            [TriggerMode::Toggle, TriggerMode::Hold]
                                                .into_iter()
                                                .map(|m| {
                                                    let active = trigger_mode == m;
                                                    let s = session_handle.clone();
                                                    mode_chip(
                                                        format!("tm-{}", m.as_str()),
                                                        m.short_label(),
                                                        active,
                                                        move |_, cx| {
                                                            s.update(cx, |session, cx| {
                                                                session.set_trigger_mode(m, cx);
                                                            });
                                                        },
                                                        cx,
                                                    )
                                                }),
                                        ),
                                ),
                            )
                            // Accessibility Permission Row
                            .child(
                                setting_row(
                                    "macOS 辅助功能权限",
                                    "用于监听全局快捷键及自动执行 ⌘V 贴入前景窗口",
                                    false,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(ui::pill(
                                            if ax_trusted {
                                                "已授权并生效"
                                            } else {
                                                "需要授权"
                                            },
                                            if ax_trusted {
                                                ui::ok(cx)
                                            } else {
                                                ui::warn(cx)
                                            },
                                            if ax_trusted {
                                                ui::ok_soft(cx)
                                            } else {
                                                ui::warn_soft(cx)
                                            },
                                        ))
                                        .child(button(
                                            "open-ax-settings",
                                            "系统设置",
                                            false,
                                            |_, _| {
                                                let _ = Command::new("open")
                                                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                                                    .spawn();
                                            },
                                        )),
                                ),
                            ),
                    ),
            )
            // 2. Microphone & Audio Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("麦克风与音频反馈", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Microphone device selector
                            .child(
                                setting_row(
                                    "音频输入设备",
                                    &format!("当前默认设备：{}", crate::capture::default_device_name()),
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .flex_wrap()
                                        .gap(px(6.0))
                                        .child({
                                            let s = session_handle.clone();
                                            mode_chip(
                                                "mic-default",
                                                "系统默认",
                                                mic_device.is_empty(),
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.set_mic_device("", cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        })
                                        .children(available_mics.into_iter().enumerate().map(
                                            |(idx, dev_name)| {
                                                let active = mic_device == dev_name;
                                                let s = session_handle.clone();
                                                let name_clone = dev_name.clone();
                                                mode_chip(
                                                    format!("mic-dev-{idx}"),
                                                    ellipsize(&dev_name, 22),
                                                    active,
                                                    move |_, cx| {
                                                        s.update(cx, |session, cx| {
                                                            session.set_mic_device(&name_clone, cx);
                                                        });
                                                    },
                                                    cx,
                                                )
                                            },
                                        ))
                                        .child({
                                            let s = session_handle.clone();
                                            icon_button(
                                                "refresh-mics",
                                                IconName::RefreshCw,
                                                "刷新麦克风列表",
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.refresh_mics(cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        }),
                                ),
                            )
                            // Live Microphone Test Row
                            .child(
                                setting_row(
                                    "实时电平测试",
                                    "开启后对着麦克风说话，检查收音波纹是否灵敏",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .when(mic_testing, |row| row.child(inline_wave_view(wave, cx)))
                                        .child({
                                            let s = session_handle.clone();
                                            button(
                                                "toggle-mic-test",
                                                if mic_testing { "停止测试" } else { "测试麦克风" },
                                                mic_testing,
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.toggle_mic_test(cx);
                                                    });
                                                },
                                            )
                                        }),
                                ),
                            )
                            // Sound cues switch
                            .child(
                                setting_row(
                                    "状态提示音效",
                                    "开始录音、结束录音与贴入完成时播放轻柔的 macOS 原生提示音",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-sound-cue")
                                        .checked(sound_feedback)
                                        .accessibility_label("状态提示音效")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_sound_feedback(val, cx);
                                            });
                                        })
                                }),
                            )
                            // Save audio files switch
                            .child(
                                setting_row(
                                    "保留本地录音文件",
                                    "将每次口述保存为 16kHz WAV 文件，支持在「口述历史」中随时回放",
                                    false,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-save-audio")
                                        .checked(save_audio)
                                        .accessibility_label("保留本地录音文件")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_save_audio(val, cx);
                                            });
                                        })
                                }),
                            ),
                    ),
            )
            // 3. Polish Strategy & Style Rules Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("润色策略与文风偏好", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Strategy selector
                            .child(
                                setting_row(
                                    "默认润色策略",
                                    "默认根据最前面的应用（如 VS Code、邮件、微信、备忘录）自动切换模式",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .flex_wrap()
                                        .gap(px(6.0))
                                        .children(
                                            [
                                                ("auto", "智能识别应用"),
                                                ("prose", "流畅正文"),
                                                ("literal", "代码终端"),
                                                ("email", "商务邮件"),
                                                ("chat", "即时通讯"),
                                                ("raw", "仅转写不润色"),
                                            ]
                                            .into_iter()
                                            .map(|(code, label)| {
                                                let active = polish_strategy == code;
                                                let s = session_handle.clone();
                                                mode_chip(
                                                    format!("pol-strat-{code}"),
                                                    label,
                                                    active,
                                                    move |_, cx| {
                                                        s.update(cx, |session, cx| {
                                                            session.set_polish_strategy(code, cx);
                                                        });
                                                    },
                                                    cx,
                                                )
                                            }),
                                        ),
                                ),
                            )
                            // CJK spacing switch
                            .child(
                                setting_row(
                                    "中英文与数字自动留白",
                                    "在中文与英文单词、数字之间自动插入半角空格（如「使用 Rust 开发」）",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-space-cjk")
                                        .checked(space_cjk)
                                        .accessibility_label("中英文与数字自动留白")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_space_cjk(val, cx);
                                            });
                                        })
                                }),
                            )
                            // Custom style instruction
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.0))
                                    .px(px(14.0))
                                    .py(px(12.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(rems(0.8125))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("自定义润色指令"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(rems(0.6875))
                                                    .text_color(ui::fg3(cx))
                                                    .child(if custom_prompt.is_empty() {
                                                        "当前使用系统默认指令".to_string()
                                                    } else {
                                                        format!("已生效：{}", ellipsize(&custom_prompt, 26))
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .child(Input::new(&custom_prompt_input).w_full()),
                                            )
                                            .child({
                                                let s = session_handle.clone();
                                                let inp = custom_prompt_input.clone();
                                                button(
                                                    "save-custom-prompt",
                                                    "应用指令",
                                                    false,
                                                    move |_, cx| {
                                                        let val = inp.read(cx).value().to_string();
                                                        s.update(cx, |session, cx| {
                                                            session.set_custom_prompt(&val, cx);
                                                        });
                                                    },
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .flex_wrap()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_size(rems(0.6875))
                                                    .text_color(ui::fg3(cx))
                                                    .child("快捷风格："),
                                            )
                                            .children(
                                                [
                                                    ("默认纯净", ""),
                                                    ("精炼简短", "尽量精炼简短，删去客套话，直奔主题。"),
                                                    ("书面专业", "使用克制、专业的书面表达，适合技术文档与正式汇报。"),
                                                    ("结构化要点", "遇到多项内容时，优先使用清晰的要点列表（- ）排版。"),
                                                ]
                                                .into_iter()
                                                .enumerate()
                                                .map(|(idx, (title, rule))| {
                                                    let active = custom_prompt == rule;
                                                    let s = session_handle.clone();
                                                    let inp = custom_prompt_input.clone();
                                                    mode_chip(
                                                        format!("prompt-preset-{idx}"),
                                                        title,
                                                        active,
                                                        move |window, cx| {
                                                            inp.update(cx, |input, cx| {
                                                                input.set_value(rule, window, cx);
                                                            });
                                                            s.update(cx, |session, cx| {
                                                                session.set_custom_prompt(rule, cx);
                                                            });
                                                        },
                                                        cx,
                                                    )
                                                }),
                                            ),
                                    ),
                            ),
                    ),
            )
            // 4. AI Gateway & Models Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("Vercel AI Gateway 连接", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(key_row(
                                key_editing,
                                has_key,
                                gateway_testing,
                                &key_input,
                                &session_handle,
                                &view,
                                cx,
                            ))
                            .child(path_row(
                                &key_path,
                                &key_status,
                                gateway_test_status,
                                cx,
                            ))
                            .child(
                                setting_row(
                                    "处理管线模型",
                                    &format!("转写：{transcribe_model}  ·  润色：{polish_model}"),
                                    false,
                                    cx,
                                )
                                .child(ui::pill(
                                    "AI Gateway v4",
                                    ui::accent(cx),
                                    ui::accent_soft(cx),
                                )),
                            ),
                    ),
            )
            // 5. System & Storage Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("系统与本机存储", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(
                                setting_row(
                                    "在 Dock 中显示应用图标",
                                    "关闭后仅保留顶部菜单栏图标与全局快捷键常驻",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-show-dock")
                                        .checked(show_dock)
                                        .accessibility_label("在 Dock 中显示应用图标")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_dock(val, cx);
                                            });
                                        })
                                }),
                            )
                            .child(
                                setting_row(
                                    "本机数据库与录音目录",
                                    &data_dir,
                                    false,
                                    cx,
                                )
                                .child(button(
                                    "open-data-dir",
                                    "在 Finder 中打开",
                                    false,
                                    move |_, _| {
                                        if let Ok(dir) = Store::data_dir() {
                                            let _ = Command::new("open").arg(dir).spawn();
                                        }
                                    },
                                )),
                            ),
                    ),
            )
    }
}

fn nav_item(
    id: impl Into<ElementId>,
    icon: LucideIcon,
    tab: NavTab,
    active: bool,
    badge: Option<String>,
    view: Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let fg = if active { ui::fg(cx) } else { ui::fg2(cx) };
    let icon_color = if active { ui::accent(cx) } else { ui::fg2(cx) };
    let bg = if active {
        ui::card_hover(cx)
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .h(px(34.0))
        .px(px(10.0))
        .rounded(px(8.0))
        .bg(bg)
        .cursor_pointer()
        .hover(|st| st.bg(ui::card_hover(cx)))
        .on_click(move |_, _, cx| {
            view.update(cx, |v, cx| {
                v.active_tab = tab;
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(9.0))
                .child(Icon::new(icon).size(px(15.0)).text_color(icon_color))
                .child(
                    div()
                        .text_size(rems(0.8125))
                        .font_weight(if active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(fg)
                        .child(tab.label()),
                ),
        )
        .when_some(badge, |row, text| {
            row.child(
                div()
                    .px(px(6.0))
                    .py(px(1.5))
                    .rounded(px(5.0))
                    .bg(ui::well_bg(cx))
                    .text_size(rems(0.65625))
                    .text_color(ui::fg3(cx))
                    .child(text),
            )
        })
}

fn sidebar_status_card(
    phase: &Phase,
    elapsed: Duration,
    hotkey_code: i64,
    trigger_mode: TriggerMode,
    has_key: bool,
    session: Entity<Session>,
    cx: &App,
) -> impl IntoElement {
    let is_rec = matches!(phase, Phase::Listening);
    let badge_str = hotkey::hotkey_badge(hotkey_code);

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(11.0))
        .rounded(px(10.0))
        .bg(ui::card_bg(cx))
        .border_1()
        .border_color(ui::hairline(cx))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .size(px(6.5))
                                .rounded_full()
                                .bg(if is_rec {
                                    ui::live(cx)
                                } else if has_key {
                                    ui::ok(cx)
                                } else {
                                    ui::warn(cx)
                                }),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .font_weight(FontWeight::MEDIUM)
                                .child(if is_rec {
                                    format!("录音中 {}", mmss(elapsed))
                                } else if has_key {
                                    "引擎就绪".into()
                                } else {
                                    "等待配置 Key".into()
                                }),
                        ),
                )
                .child(ui::kbd_cap(badge_str, cx)),
        )
        .child(
            div()
                .text_size(rems(0.6875))
                .text_color(ui::fg3(cx))
                .child(format!(
                    "{} · {}",
                    hotkey::hotkey_label(hotkey_code),
                    trigger_mode.short_label()
                )),
        )
        .child(button(
            "sidebar-quick-toggle",
            if is_rec { "完成并转写" } else { "立即开始口述" },
            is_rec,
            move |_, cx| {
                session.update(cx, |s, cx| s.toggle(cx));
            },
        ))
}

#[allow(clippy::too_many_arguments)]
fn hero_banner(
    phase: &Phase,
    elapsed: Duration,
    wave: [f32; crate::capture::WAVE_BARS],
    has_key: bool,
    hotkey_code: i64,
    trigger_mode: TriggerMode,
    strategy_label: &str,
    session: Entity<Session>,
    view: Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let card = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .p(px(18.0))
        .rounded(px(12.0))
        .bg(ui::card_bg(cx))
        .border_1()
        .border_color(ui::hairline(cx));

    match phase {
        Phase::Listening => {
            let stop_session = session.clone();
            let cancel_session = session;
            card.border_color(ui::live(cx))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(38.0))
                                .rounded_full()
                                .bg(ui::live_soft(cx))
                                .child(div().size(px(10.0)).rounded_full().bg(ui::live(cx))),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(3.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(
                                            div()
                                                .text_size(rems(0.9375))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(format!("正在聆听 · {}", mmss(elapsed))),
                                        )
                                        .child(inline_wave_view(wave, cx)),
                                )
                                .child(
                                    div()
                                        .text_size(rems(0.75))
                                        .text_color(ui::fg2(cx))
                                        .child("说完后再次按快捷键或点击右侧「完成」即可转写并贴入目标窗口"),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(button("hero-cancel", "取消", false, move |_, cx| {
                            cancel_session.update(cx, |s, cx| s.cancel_recording(cx));
                        }))
                        .child(button("hero-stop", "完成转写", true, move |_, cx| {
                            stop_session.update(cx, |s, cx| s.toggle(cx));
                        })),
                )
        }
        Phase::Working(stage) => card.child(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .child(Spinner::new().with_size(Size::Medium))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(rems(0.9375))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(stage.label()),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .text_color(ui::fg2(cx))
                                .child(format!(
                                    "处理管线第 {} / 3 步 · 完成后自动贴入最前面的窗口",
                                    stage.index() + 1
                                )),
                        ),
                ),
        ),
        Phase::Message(notice) => {
            let (icon, color) = notice_look(notice, cx);
            card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(Icon::new(icon).size(px(18.0)).text_color(color))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(rems(0.9375))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(ellipsize(notice.text(), 56)),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.75))
                                    .text_color(ui::fg2(cx))
                                    .child("内容已同步归档到口述历史"),
                            ),
                    ),
            )
            .child(button("hero-again", "再次口述", false, move |_, cx| {
                session.update(cx, |s, cx| s.toggle(cx));
            }))
        }
        Phase::Idle => card
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(38.0))
                            .px(px(12.0))
                            .rounded(px(9.0))
                            .bg(ui::well_bg(cx))
                            .border_1()
                            .border_color(ui::border_strong(cx))
                            .text_size(rems(0.875))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ui::fg(cx))
                            .child(hotkey::hotkey_badge(hotkey_code)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(rems(0.9375))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("随时随地语音口述"),
                                    )
                                    .child(ui::pill(
                                        strategy_label.to_string(),
                                        ui::accent(cx),
                                        ui::accent_soft(cx),
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.75))
                                    .text_color(ui::fg2(cx))
                                    .child(format!(
                                        "当前触发方式：{} · {}",
                                        hotkey::hotkey_label(hotkey_code),
                                        trigger_mode.label()
                                    )),
                            ),
                    ),
            )
            .child(if has_key {
                button("hero-start", "开始口述", true, move |_, cx| {
                    session.update(cx, |s, cx| s.toggle(cx));
                })
            } else {
                button("hero-config-key", "配置 API Key", true, move |_, cx| {
                    view.update(cx, |v, cx| {
                        v.active_tab = NavTab::Settings;
                        cx.notify();
                    });
                })
            }),
    }
}

fn metric_card(
    title: &'static str,
    value: String,
    subtitle: String,
    icon: LucideIcon,
    accent_color: Hsla,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .justify_between()
        .gap(px(8.0))
        .p(px(14.0))
        .rounded(px(12.0))
        .bg(ui::card_bg(cx))
        .border_1()
        .border_color(ui::hairline(cx))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(rems(0.71875))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg2(cx))
                        .child(title),
                )
                .child(Icon::new(icon).size(px(14.0)).text_color(accent_color)),
        )
        .child(
            div()
                .text_size(rems(1.375))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::fg(cx))
                .child(value),
        )
        .child(
            div()
                .text_size(rems(0.6875))
                .text_color(ui::fg3(cx))
                .child(subtitle),
        )
}

#[allow(clippy::too_many_arguments)]
fn history_card_row(
    item: HistoryItem,
    last: bool,
    has_audio: bool,
    is_playing: bool,
    is_expanded: bool,
    is_repolishing: bool,
    copied_token: Option<String>,
    session: Entity<Session>,
    view: Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let id = item.id;
    let app_name = polish::friendly_app_name(&item.bundle_id);
    let mode_label = PolishMode::from_str(&item.mode)
        .unwrap_or_else(|| polish::mode_for_bundle(&item.bundle_id))
        .short_label();
    let duration_str = format_duration_ms(item.duration_ms);
    let rel_time = format_relative_time(item.created_at);
    let display_text = item.display_text().to_string();
    let raw_text = item.transcript.clone();
    let has_distinct_raw = item.has_distinct_raw();
    let chars_count = item.char_count();

    let copy_pol_tok = format!("pol-{id}");
    let copy_raw_tok = format!("raw-{id}");
    let is_pol_copied = copied_token.as_deref() == Some(copy_pol_tok.as_str());
    let is_raw_copied = copied_token.as_deref() == Some(copy_raw_tok.as_str());

    div()
        .id(SharedString::from(format!("hist-row-{id}")))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(14.0))
        .when(!last, |row| row.border_b_1().border_color(ui::hairline(cx)))
        // Top Metadata & Action Bar
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(ui::pill(app_name, ui::fg(cx), ui::well_bg(cx)))
                        .child(ui::pill(
                            mode_label,
                            ui::accent(cx),
                            ui::accent_soft(cx),
                        ))
                        .when(!duration_str.is_empty(), |row| {
                            row.child(
                                div()
                                    .text_size(rems(0.6875))
                                    .text_color(ui::fg3(cx))
                                    .child(format!("录音 {duration_str}")),
                            )
                        })
                        .child(
                            div()
                                .text_size(rems(0.6875))
                                .text_color(ui::fg3(cx))
                                .child(format!("{chars_count} 字 · {rel_time}")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .when(has_audio, |bar| {
                            let s = session.clone();
                            bar.child(small_action_pill(
                                format!("play-audio-{id}"),
                                if is_playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                },
                                if is_playing { "停止" } else { "回放" },
                                move |_, cx| {
                                    s.update(cx, |session, cx| {
                                        session.toggle_play_history(id, cx);
                                    });
                                },
                                cx,
                            ))
                        })
                        .when(has_distinct_raw, |bar| {
                            let v = view.clone();
                            bar.child(small_action_pill(
                                format!("toggle-raw-{id}"),
                                IconName::FileText,
                                if is_expanded { "收起原文" } else { "对比原文" },
                                move |_, cx| {
                                    v.update(cx, |view, cx| {
                                        view.toggle_raw_expand(id, cx);
                                    });
                                },
                                cx,
                            ))
                        })
                        .child({
                            let text_for_copy = display_text.clone();
                            let v = view.clone();
                            let tok = copy_pol_tok;
                            small_action_pill(
                                format!("copy-pol-{id}"),
                                if is_pol_copied {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                },
                                if is_pol_copied { "已复制" } else { "复制" },
                                move |_, cx| {
                                    if paste::copy_to_clipboard(&text_for_copy) {
                                        v.update(cx, |view, cx| {
                                            view.flash_copied(tok.clone(), cx);
                                        });
                                    }
                                },
                                cx,
                            )
                        })
                        .child({
                            let s = session.clone();
                            let item_clone = item.clone();
                            small_action_pill(
                                format!("repolish-{id}"),
                                IconName::RefreshCw,
                                if is_repolishing { "润色中…" } else { "重润色" },
                                move |_, cx| {
                                    s.update(cx, |session, cx| {
                                        session.repolish_history_item(item_clone.clone(), cx);
                                    });
                                },
                                cx,
                            )
                        })
                        .child({
                            let s = session;
                            icon_button(
                                format!("del-hist-{id}"),
                                IconName::Close,
                                "删除记录",
                                move |_, cx| {
                                    s.update(cx, |session, cx| {
                                        session.delete_history(id, cx);
                                    });
                                },
                                cx,
                            )
                        }),
                ),
        )
        // Main Polished Text
        .child(
            div()
                .text_size(rems(0.84375))
                .line_height(rems(1.38))
                .text_color(ui::fg(cx))
                .child(display_text),
        )
        // Expandable Raw Transcript Comparison Box
        .when(is_expanded && has_distinct_raw, |col| {
            let v = view;
            let raw_copy = raw_text.clone();
            let tok = copy_raw_tok;
            col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(ui::well_bg(cx))
                    .border_1()
                    .border_color(ui::hairline(cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(rems(0.6875))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ui::fg3(cx))
                                    .child("语音转写原文（未润色）"),
                            )
                            .child(small_action_pill(
                                format!("copy-raw-btn-{id}"),
                                if is_raw_copied {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                },
                                if is_raw_copied { "已复制原文" } else { "复制原文" },
                                move |_, cx| {
                                    if paste::copy_to_clipboard(&raw_copy) {
                                        v.update(cx, |view, cx| {
                                            view.flash_copied(tok.clone(), cx);
                                        });
                                    }
                                },
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .text_size(rems(0.78125))
                            .line_height(rems(1.25))
                            .text_color(ui::fg2(cx))
                            .child(raw_text),
                    ),
            )
        })
}

fn setting_row(title: &str, subtitle: &str, border_bottom: bool, cx: &App) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .px(px(14.0))
        .py(px(12.0))
        .when(border_bottom, |row| {
            row.border_b_1().border_color(ui::hairline(cx))
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(rems(0.8125))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(rems(0.71875))
                        .text_color(ui::fg2(cx))
                        .child(subtitle.to_string()),
                ),
        )
}

fn mode_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(26.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .bg(if active {
            ui::accent_soft(cx)
        } else {
            ui::well_bg(cx)
        })
        .border_1()
        .border_color(if active {
            ui::accent(cx)
        } else {
            ui::hairline(cx)
        })
        .cursor_pointer()
        .hover(|st| {
            if !active {
                st.bg(ui::card_hover(cx))
            } else {
                st
            }
        })
        .text_size(rems(0.75))
        .font_weight(if active {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .text_color(if active { ui::accent(cx) } else { ui::fg2(cx) })
        .on_click(move |_, window, cx| action(window, cx))
        .child(label.into())
}

fn small_action_pill(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &'static str,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(4.0))
        .h(px(22.0))
        .px(px(7.5))
        .rounded(px(6.0))
        .bg(ui::well_bg(cx))
        .cursor_pointer()
        .hover(|st| st.bg(ui::card_hover(cx)))
        .text_size(rems(0.6875))
        .text_color(ui::fg2(cx))
        .on_click(move |_, window, cx| action(window, cx))
        .child(Icon::new(icon).size(px(11.0)).text_color(ui::fg2(cx)))
        .child(label)
}

fn inline_wave_view(wave: [f32; crate::capture::WAVE_BARS], cx: &App) -> gpui_kit::Div {
    let color = ui::live(cx);
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(20.0))
        .children((0..crate::capture::WAVE_BARS * 2).map(move |index| {
            let source = if index < crate::capture::WAVE_BARS {
                crate::capture::WAVE_BARS - 1 - index
            } else {
                index - crate::capture::WAVE_BARS
            };
            let level = wave[source].clamp(0.0, 1.0).powf(0.65);
            let height = 3.0 + level * 15.0;
            div()
                .w(px(2.5))
                .h(px(height))
                .rounded(px(1.25))
                .bg(color)
        }))
}

fn status_pill(phase: &Phase, elapsed: Duration, cx: &App) -> impl IntoElement {
    let (color, label) = match phase {
        Phase::Idle => (ui::ok(cx), "就绪".to_string()),
        Phase::Listening => (ui::live(cx), format!("正在听 {}", mmss(elapsed))),
        Phase::Working(stage) => (ui::warn(cx), stage.label().to_string()),
        Phase::Message(Notice::Error(_)) | Phase::Message(Notice::Warn(_)) => {
            (ui::warn(cx), "需要留意".to_string())
        }
        Phase::Message(_) => (ui::ok(cx), "已完成".to_string()),
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(24.0))
        .px(px(10.0))
        .rounded(px(12.0))
        .bg(ui::card_bg(cx))
        .border_1()
        .border_color(ui::hairline(cx))
        .child(div().size(px(6.5)).rounded_full().bg(color))
        .child(
            div()
                .text_size(rems(0.71875))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::fg2(cx))
                .child(label),
        )
}

fn notice_look(notice: &Notice, cx: &App) -> (IconName, Hsla) {
    match notice {
        Notice::Done(_) => (IconName::Check, ui::ok(cx)),
        Notice::Copied => (IconName::Copy, ui::fg2(cx)),
        Notice::Warn(_) => (IconName::TriangleAlert, ui::warn(cx)),
        Notice::Error(_) => (IconName::CircleAlert, ui::live(cx)),
    }
}

fn hud_notice_look(notice: &Notice) -> (IconName, Hsla) {
    match notice {
        Notice::Done(_) => (IconName::Check, ui::hud_ok()),
        Notice::Copied => (IconName::Copy, ui::hud_dim()),
        Notice::Warn(_) => (IconName::TriangleAlert, ui::hud_warn()),
        Notice::Error(_) => (IconName::CircleAlert, ui::hud_live()),
    }
}

fn key_row(
    editing: bool,
    has_key: bool,
    testing: bool,
    key_input: &Entity<InputState>,
    session: &Entity<Session>,
    view: &Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let row = div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(11.0))
        .border_b_1()
        .border_color(ui::hairline(cx));

    if editing {
        let save_input = key_input.clone();
        let save_session = session.clone();
        let save_view = view.clone();
        let cancel_view = view.clone();
        row.child(
            div()
                .text_size(rems(0.8125))
                .font_weight(FontWeight::MEDIUM)
                .child("Gateway Key"),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(Input::new(key_input).w_full()),
        )
        .child(button("save-key", "保存", true, move |_window, cx| {
            let value = save_input.read(cx).value();
            save_session.update(cx, |session, cx| session.save_key(value.as_ref(), cx));
            let saved = save_session.read(cx).has_key();
            save_view.update(cx, |view, cx| {
                if saved {
                    view.key_editing = false;
                }
                cx.notify();
            });
        }))
        .when(has_key, |row| {
            row.child(button("cancel-key", "取消", false, move |_, cx| {
                cancel_view.update(cx, |view, cx| {
                    view.key_editing = false;
                    cx.notify();
                });
            }))
        })
    } else {
        let edit_view = view.clone();
        let reload_session = session.clone();
        let test_session = session.clone();
        row.child(
            div()
                .text_size(rems(0.8125))
                .font_weight(FontWeight::MEDIUM)
                .child("Gateway Key"),
        )
        .child(
            div()
                .flex_1()
                .text_size(rems(0.75))
                .text_color(ui::fg2(cx))
                .child("vck_••••••••••••••••"),
        )
        .child(button(
            "test-gateway-key",
            if testing { "正在测试…" } else { "测试连接" },
            false,
            move |_, cx| {
                test_session.update(cx, |session, cx| session.test_gateway(cx));
            },
        ))
        .child(button("edit-key", "更换", false, move |_, cx| {
            edit_view.update(cx, |view, cx| {
                view.key_editing = true;
                cx.notify();
            });
        }))
        .child(icon_button(
            "reload-key",
            IconName::RefreshCw,
            "重新读取 Key",
            move |_, cx| {
                reload_session.update(cx, |session, cx| session.reload_key(cx));
            },
            cx,
        ))
    }
}

fn path_row(
    path: &str,
    status: &str,
    test_status: Option<(bool, String)>,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(9.0))
        .border_b_1()
        .border_color(ui::hairline(cx))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(rems(0.6875))
                .text_color(ui::fg3(cx))
                .child(path.to_string())
                .text_ellipsis_middle(),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .when_some(test_status, |row, (ok, msg)| {
                    row.child(ui::pill(
                        ellipsize(&msg, 32),
                        if ok { ui::ok(cx) } else { ui::live(cx) },
                        if ok {
                            ui::ok_soft(cx)
                        } else {
                            ui::live_soft(cx)
                        },
                    ))
                })
                .child(
                    div()
                        .text_size(rems(0.6875))
                        .text_color(ui::fg3(cx))
                        .child(ellipsize(status, 28)),
                ),
        )
}

fn phrase_row(
    id: i64,
    label: String,
    last: bool,
    session: Entity<Session>,
    hairline: Hsla,
    hover: Hsla,
    dim: Hsla,
) -> impl IntoElement {
    let accessible = format!("删除 {label}");
    div()
        .id(SharedString::from(format!("phrase-{id}")))
        .group("phrase-row")
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(9.0))
        .min_h(px(38.0))
        .when(!last, |row| row.border_b_1().border_color(hairline))
        .hover(move |style| style.bg(hover))
        .child(div().flex_1().text_size(rems(0.8125)).child(label))
        .child(
            div()
                .id(SharedString::from(format!("delete-{id}")))
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.0))
                .rounded(px(6.0))
                .opacity(0.35)
                .group_hover("phrase-row", |style| style.opacity(0.95))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .aria_label(accessible)
                .on_click(move |_, _, cx| {
                    session.update(cx, |session, cx| session.delete_phrase(id, cx));
                })
                .child(Icon::new(IconName::Close).size(px(12.0)).text_color(dim)),
        )
}

fn empty_state(
    icon: IconName,
    title: &'static str,
    body: &'static str,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(5.0))
        .py(px(24.0))
        .child(Icon::new(icon).size(px(18.0)).text_color(ui::fg3(cx)))
        .child(
            div()
                .text_size(rems(0.78125))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::fg2(cx))
                .child(title),
        )
        .child(
            div()
                .max_w(px(360.0))
                .text_size(rems(0.71875))
                .text_color(ui::fg3(cx))
                .child(body),
        )
}

fn icon_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &'static str,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|style| style.bg(ui::card_hover(cx)))
        .aria_label(label)
        .on_click(move |_, window, cx| action(window, cx))
        .child(Icon::new(icon).size(px(13.0)).text_color(ui::fg2(cx)))
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Button {
    let button = Button::new(id).label(label).with_size(Size::Small);
    let button = if primary {
        button.primary()
    } else {
        button.secondary()
    };
    button.on_click(move |_, window, cx| action(window, cx))
}

pub struct OverlayView {
    session: Entity<Session>,
}

impl OverlayView {
    pub fn new(session: Entity<Session>, cx: &mut Context<Self>) -> Self {
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        Self { session }
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (phase, wave, elapsed, active_bundle, polish_strategy, hotkey_code, appearance, exiting) = {
            let session = self.session.read(cx);
            (
                session.phase.clone(),
                session.wave,
                session.elapsed(),
                session.active_bundle.clone(),
                session.polish_strategy.clone(),
                session.hotkey_code,
                session.appearance,
                session.exiting,
            )
        };

        let app_name = polish::friendly_app_name(&active_bundle);
        let mode_label = polish::resolve_mode(&polish_strategy, &active_bundle)
            .map(|m| m.short_label())
            .unwrap_or("原文");
        let context_tag = format!("{app_name} · {mode_label}");
        let hk_badge = hotkey::hotkey_badge(hotkey_code);

        let content = match &phase {
            Phase::Idle => div(),
            Phase::Listening => listening_view(wave, elapsed, &context_tag, hk_badge),
            Phase::Working(stage) => working_view(*stage, &context_tag),
            Phase::Message(notice) => message_view(notice, cx),
        };

        div()
            .flex()
            .flex_col()
            .justify_end()
            .items_center()
            .size_full()
            .pb(px(10.0))
            .child(content.with_animation(
                ("hud", appearance),
                Animation::new(if exiting {
                    Duration::from_millis(150)
                } else {
                    Duration::from_millis(180)
                })
                .with_easing(move |t| {
                    let eased = gpui_kit::base::animation::ease_out_cubic(t);
                    if exiting {
                        1.0 - eased
                    } else {
                        eased
                    }
                }),
                |element, delta| element.opacity(delta),
            ))
    }
}

fn hud_shell() -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .h(px(54.0))
        .px(px(16.0))
        .rounded(px(18.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgb(0x1d1d22), 0.0),
            linear_color_stop(rgb(0x111114), 1.0),
        ))
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.09))
        .shadow(vec![
            BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 1.0, 0.12)).inset(),
            BoxShadow::new(px(0.0), px(14.0), hsla(0.0, 0.0, 0.0, 0.58))
                .blur_radius(px(30.0))
                .spread_radius(px(-6.0)),
        ])
        .text_size(rems(0.8125))
        .text_color(ui::hud_fg())
}

fn listening_view(
    wave: [f32; crate::capture::WAVE_BARS],
    elapsed: Duration,
    context_tag: &str,
    hk_badge: &str,
) -> gpui_kit::Div {
    hud_shell()
        .gap(px(12.0))
        .child(div().size(px(8.0)).rounded_full().bg(ui::hud_live()))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(mmss(elapsed)),
        )
        .child(wave_view(wave))
        .child(
            div()
                .px(px(7.0))
                .py(px(2.5))
                .rounded(px(6.0))
                .bg(hsla(0.0, 0.0, 1.0, 0.08))
                .text_size(rems(0.6875))
                .text_color(ui::hud_dim())
                .child(context_tag.to_string()),
        )
        .child(
            div()
                .text_size(rems(0.6875))
                .text_color(ui::hud_fg_dim())
                .child(hk_badge.to_string()),
        )
}

fn wave_view(wave: [f32; crate::capture::WAVE_BARS]) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(28.0))
        .children((0..crate::capture::WAVE_BARS * 2).map(move |index| {
            let source = if index < crate::capture::WAVE_BARS {
                crate::capture::WAVE_BARS - 1 - index
            } else {
                index - crate::capture::WAVE_BARS
            };
            let level = wave[source].clamp(0.0, 1.0).powf(0.65);
            let height = 4.0 + level * 22.0;
            let center = crate::capture::WAVE_BARS as f32 - 0.5;
            let distance = (index as f32 - center).abs() / crate::capture::WAVE_BARS as f32;
            let alpha = 0.38 + 0.62 * (1.0 - distance).clamp(0.0, 1.0);
            div()
                .w(px(2.5))
                .h(px(height))
                .rounded(px(1.25))
                .bg(hsla(0.0, 0.0, 1.0, alpha))
        }))
}

fn working_view(stage: Stage, context_tag: &str) -> gpui_kit::Div {
    hud_shell()
        .flex_col()
        .items_stretch()
        .justify_center()
        .h(px(64.0))
        .min_w(px(260.0))
        .gap(px(9.0))
        .px(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(Spinner::new().with_size(Size::Small))
                .child(div().font_weight(FontWeight::MEDIUM).child(stage.label()))
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(rems(0.6875))
                        .text_color(ui::hud_fg_dim())
                        .child(format!("{context_tag} · {}/3", stage.index() + 1)),
                ),
        )
        .child(segments(stage))
}

fn segments(stage: Stage) -> gpui_kit::Div {
    let active = stage.index();
    div().flex().gap(px(3.0)).children((0..3).map(move |index| {
        let color = if index < active {
            ui::hud_accent()
        } else if index == active {
            hsla(0.0, 0.0, 1.0, 0.72)
        } else {
            hsla(0.0, 0.0, 1.0, 0.16)
        };
        div().flex_1().h(px(2.5)).rounded(px(1.25)).bg(color)
    }))
}

fn message_view(notice: &Notice, _cx: &App) -> gpui_kit::Div {
    let (icon, color) = hud_notice_look(notice);
    let shell = hud_shell()
        .gap(px(9.0))
        .child(Icon::new(icon).size(px(15.0)).text_color(color));
    match notice {
        Notice::Error(_) => shell.child(
            div()
                .max_w(px(340.0))
                .text_size(rems(0.8125))
                .child(ellipsize(notice.text(), 48)),
        ),
        _ => shell.child(div().child(notice.text().to_string())),
    }
}
