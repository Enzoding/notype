use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::Context;

use crate::audio::{self, SoundCue};
use crate::capture::ActiveRecording;
use crate::front::front_bundle_id;
use crate::gateway::{self, POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::hotkey::{self, HotkeyEvent};
use crate::paste::{self, InsertResult};
use crate::polish::{self, PolishMode, PolishOptions};
use crate::secrets::{self, KeySource};
use crate::store::{HistoryItem, Phrase, StatsSummary, Store, TriggerMode};

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
    pub fn text(&self) -> &str {
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
    pub appearance: u64,
    pub exiting: bool,
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

pub fn mmss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn format_duration_ms(ms: i64) -> String {
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

pub fn format_relative_time(ts: i64) -> String {
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
