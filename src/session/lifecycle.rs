use super::types::{Notice, Phase, Stage};
use super::Session;
use crate::audio::{self, SoundCue};
use crate::capture::ActiveRecording;
use crate::front::front_bundle_id;
use crate::hotkey::{self, HotkeyEvent};
use crate::paste::{self, InsertResult};
use crate::polish::{self, PolishOptions};
use crate::tasks::{self, CaptureResult, Task, TaskConfig};
use gpui_kit::Context;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

impl Session {
    pub fn has_key(&self) -> bool {
        !self.provider_key(self.connections.kind).is_empty()
    }
    pub fn elapsed(&self) -> Duration {
        self.recording
            .as_ref()
            .map(|r| r.elapsed())
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
            HotkeyEvent::Cancel => self.cancel_active(cx),
        }
    }
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish(cx);
        } else if !matches!(self.phase, Phase::Working(_)) {
            self.begin(false, cx);
        }
    }
    pub fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        self.interrupt_recording("已取消，录音草稿保留七天，可在设置中恢复或删除", cx);
    }
    pub(super) fn interrupt_recording(&mut self, message: &str, cx: &mut Context<Self>) {
        if let Some(recording) = self.recording.take() {
            if let Some(capture) = recording.finish() {
                let message = message.to_string();
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let id = capture.task.id.clone();
                            match capture.finish() {
                                Ok(mut task) => {
                                    task.phase = "已中断，可恢复".into();
                                    task.error = message;
                                    task.save()
                                }
                                Err(error) => {
                                    let mut task = Task::load(&id)?;
                                    task.phase = "录音保存异常".into();
                                    task.error = error;
                                    task.save()
                                }
                            }
                        })
                        .await;
                    let _ = this.update(cx, |s, cx| {
                        if let Err(err) = result {
                            s.recovery_status = err;
                        }
                        s.refresh_recovery(cx);
                    });
                })
                .detach();
            }
        }
        self.generation = self.generation.wrapping_add(1);
        self.studio_dictating = false;
        self.active_task = None;
        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];
        self.note(Notice::Warn(message.into()), cx);
    }
    pub fn cancel_active(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.cancel_recording(cx);
            return;
        }
        if matches!(self.phase, Phase::Working(_)) {
            if let Some(cancelled) = self.cancelled.take() {
                cancelled.store(true, Ordering::Release);
            }
            self.generation = self.generation.wrapping_add(1);
            self.stages = None;
            self.working_progress = 0.0;
            self.active_task = None;
            self.note(Notice::Warn("已取消，已保存内容可在设置中恢复".into()), cx);
            self.refresh_recovery(cx);
        }
    }
    pub(super) fn begin(&mut self, for_studio: bool, cx: &mut Context<Self>) {
        if self.provider_key(self.connections.kind).is_empty() {
            self.note(Notice::Error("请先配置转写渠道的密钥".into()), cx);
            return;
        }
        if let Some(stream) = self.mic_test_stream.take() {
            drop(stream.finish());
            self.mic_testing = false;
        }
        self.studio_dictating = for_studio;
        self.active_bundle = if for_studio {
            "notype.studio".into()
        } else {
            front_bundle_id()
        };
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let resolved = if for_studio {
            Some(self.studio_mode)
        } else {
            polish::resolve_mode(&self.polish_strategy, &self.active_bundle)
        };
        let config = TaskConfig {
            transcribe: self.profile(self.connections.kind),
            polish: self.profile(self.connections.kind),
            options: resolved.map(|mode| PolishOptions {
                mode,
                custom_prompt: self.custom_prompt.clone(),
                space_cjk: self.space_cjk,
            }),
            bundle: self.active_bundle.clone(),
            mode: resolved
                .map(|m| m.as_str().into())
                .unwrap_or_else(|| "raw".into()),
            save_audio: self.save_audio,
        };
        let preferred = if self.mic_device.is_empty() {
            None
        } else {
            Some(self.mic_device.as_str())
        };
        match ActiveRecording::start_saved(preferred, config) {
            Ok(recording) => {
                self.active_task = recording.task_id();
                self.recording = Some(recording);
                self.level = 0.0;
                self.wave = [0.0; crate::capture::WAVE_BARS];
                self.exiting = false;
                self.appearance = self.appearance.wrapping_add(1);
                self.phase = Phase::Listening;
                self.sync_voice(cx);
                if self.sound_feedback {
                    audio::play_cue(SoundCue::Start);
                }
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_secs(tasks::MAX_RECORDING_SECS))
                        .await;
                    let _ = this.update(cx, |s, cx| {
                        if s.generation == generation && s.recording.is_some() {
                            s.finish(cx);
                        }
                    });
                })
                .detach();
            }
            Err(err) => {
                self.studio_dictating = false;
                self.note(Notice::Error(err), cx);
            }
        }
        cx.notify();
    }
    pub(super) fn finish(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        let for_studio = self.studio_dictating;
        self.studio_dictating = false;
        let short = recording.elapsed() < Duration::from_millis(300);
        let Some(capture) = recording.finish() else {
            return;
        };
        if short {
            self.spawn_saved_draft(capture, cx);
            self.note(Notice::Warn("录音时间太短，已保留草稿".into()), cx);
            return;
        }
        if self.sound_feedback {
            audio::play_cue(SoundCue::Stop);
        }
        self.launch_task(Ok(capture), None, for_studio, cx);
    }
    fn spawn_saved_draft(&mut self, capture: CaptureResult, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut task = capture.finish()?;
                    task.phase = "录音过短".into();
                    task.save()
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if let Err(e) = result {
                    s.recovery_status = e;
                }
                s.refresh_recovery(cx);
            });
        })
        .detach();
    }
    pub(super) fn launch_task(
        &mut self,
        capture: Result<CaptureResult, String>,
        recovered: Option<String>,
        for_studio: bool,
        cx: &mut Context<Self>,
    ) {
        let generation = self.generation;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancelled = Some(cancelled.clone());
        let all_keys = self.connections.provider_keys.clone();
        let retry_transcribe = self.profile(self.connections.kind);
        let retry_polish = self.profile(self.connections.kind);
        let store = self.store.clone();
        let (sender, receiver) = mpsc::channel();
        self.stages = Some(receiver);
        self.phase = Phase::Working(Stage::Transcribe);
        self.working_progress = 0.08;
        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];
        self.sync_voice(cx);
        cx.notify();
        let is_recovery = recovered.is_some();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let (phrases, corrections) = {
                        let db = store.lock().map_err(|_| "历史数据库忙碌")?;
                        (db.phrase_list(), db.corrections())
                    };
                    let mut task = if let Some(id) = recovered {
                        Task::load(&id)?
                    } else {
                        let capture = capture?;
                        let id = capture.task.id.clone();
                        match capture.finish() {
                            Ok(task) => task,
                            Err(error) => {
                                let mut task = Task::load(&id)?;
                                task.phase = "录音保存异常".into();
                                task.error = error.clone();
                                task.save()?;
                                return Err(error);
                            }
                        }
                    };
                    if is_recovery {
                        if task.transcript.is_empty() {
                            task.config.transcribe = retry_transcribe;
                        }
                        if task.polished.is_empty() {
                            task.config.polish = retry_polish;
                        }
                        task.save()?;
                    }
                    let keys = crate::tasks::pipeline::Credentials {
                        transcribe: all_keys[if task.config.transcribe.kind
                            == crate::providers::ProviderKind::Vercel
                        {
                            0
                        } else {
                            1
                        }]
                        .clone(),
                        polish: all_keys[if task.config.polish.kind
                            == crate::providers::ProviderKind::Vercel
                        {
                            0
                        } else {
                            1
                        }]
                        .clone(),
                    };
                    crate::tasks::pipeline::process(
                        &mut task,
                        &keys,
                        &phrases,
                        &corrections,
                        &cancelled,
                        |polish| {
                            let _ = sender.send(if polish {
                                Stage::Polish
                            } else {
                                Stage::Transcribe
                            });
                        },
                    )?;
                    if cancelled.load(Ordering::Acquire) {
                        return Err("已取消".into());
                    }
                    let wav = if task.config.save_audio {
                        Some(task.wav()?)
                    } else {
                        None
                    };
                    let store = store.lock().map_err(|_| "历史数据库忙碌")?;
                    let id = store
                        .save_task_history(&task)
                        .map_err(|_| "历史保存失败，任务仍可恢复")?;
                    task.history_id = Some(id);
                    let has_audio = if let Some(wav) = wav {
                        store
                            .save_audio_file(id, &wav)
                            .map_err(|_| "录音归档失败，恢复缓存仍保留")?;
                        true
                    } else {
                        false
                    };
                    if cancelled.load(Ordering::Acquire) {
                        task.phase = "已取消".into();
                        task.save()?;
                        return Err("已取消".into());
                    }
                    let _ = sender.send(Stage::Insert);
                    let history = store.history().map_err(|_| "无法刷新历史")?;
                    let stats = store.stats();
                    drop(store);
                    if task.error.is_empty() {
                        task.phase = "已完成".into();
                    }
                    task.save()?;
                    if task.error.is_empty() {
                        task.remove()?;
                    }
                    Ok((task, id, has_audio, history, stats))
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation || !matches!(s.phase, Phase::Working(_)) {
                    s.refresh_recovery(cx);
                    return;
                }
                s.cancelled = None;
                s.active_task = None;
                s.stages = None;
                s.working_progress = 1.0;
                match result {
                    Err(err) => s.note(Notice::Error(err), cx),
                    Ok((task, id, has_audio, history, stats)) => {
                        s.history = history;
                        s.stats = stats;
                        if has_audio {
                            s.audio_ids.insert(id);
                        }
                        let text = if task.polished.is_empty() {
                            task.corrected.clone()
                        } else {
                            task.polished.clone()
                        };
                        if is_recovery || for_studio {
                            s.studio_raw = task.transcript;
                            s.studio_output = text;
                            s.note(
                                Notice::Done(
                                    "内容已保存到历史，可复制；恢复任务不会自动粘贴".into(),
                                ),
                                cx,
                            );
                        } else {
                            let (inserted, restore) = paste::insert_text(&text);
                            if let Some(restore) = restore {
                                cx.spawn(async move |_this, cx| {
                                    cx.background_executor()
                                        .timer(Duration::from_millis(280))
                                        .await;
                                    cx.update(|_| restore.apply());
                                })
                                .detach();
                            }
                            if matches!(inserted, InsertResult::Pasted) {
                                s.watch_paste_correction(id, text, cx);
                            }
                            let notice = if !task.error.is_empty() {
                                Notice::Warn("润色失败，已使用转写原文；可在设置中重试润色".into())
                            } else if matches!(inserted, InsertResult::Pasted) {
                                Notice::Done("已贴入并保存".into())
                            } else {
                                Notice::Copied
                            };
                            s.note(notice, cx);
                        }
                        s.maybe_mine(cx);
                    }
                }
                s.refresh_recovery(cx);
            });
        })
        .detach();
    }
    pub(super) fn note(&mut self, notice: Notice, cx: &mut Context<Self>) {
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
        hotkey::set_cancel_armed(matches!(self.phase, Phase::Listening | Phase::Working(_)));
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

    pub fn watch_level(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut permission_poll = std::time::Instant::now();
            loop {
                let mut interval = Duration::from_millis(90);
                let alive = this
                    .update(cx, |session, cx| {
                        let mut changed = false;
                        if permission_poll.elapsed() >= Duration::from_secs(2) {
                            permission_poll = std::time::Instant::now();
                            let trusted = crate::hotkey::is_accessibility_trusted();
                            let microphone = crate::permissions::microphone_status();
                            if session.microphone_access != microphone {
                                session.microphone_access = microphone;
                                changed = true;
                            }
                            if session.accessibility_trusted != trusted {
                                session.accessibility_trusted = trusted;
                                changed = true;
                            }
                        }
                        if let Some(error) = session.recording.as_ref().and_then(|r| r.error()) {
                            session.interrupt_recording(&error, cx);
                            return;
                        }

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
                        if let Phase::Working(stage) = session.phase {
                            let target = match stage {
                                Stage::Transcribe => 0.48,
                                Stage::Polish => 0.88,
                                Stage::Insert => 0.98,
                            };
                            if session.working_progress < target {
                                let diff = target - session.working_progress;
                                let step = (diff * 0.08).max(0.003);
                                session.working_progress =
                                    (session.working_progress + step).min(target);
                                changed = true;
                            } else if session.working_progress < 0.96 {
                                session.working_progress =
                                    (session.working_progress + 0.0008).min(0.96);
                                changed = true;
                            }
                        }
                        if changed {
                            cx.notify();
                        }
                        if matches!(session.phase, Phase::Listening | Phase::Working(_)) {
                            crate::overlay::track_voice_display();
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
}
