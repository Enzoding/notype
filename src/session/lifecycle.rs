use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::Context;

use crate::audio::{self, SoundCue};
use crate::capture::ActiveRecording;
use crate::front::front_bundle_id;
use crate::gateway;
use crate::hotkey::{self, HotkeyEvent};
use crate::mining;
use crate::paste::{self, InsertResult};
use crate::polish::{self, PolishOptions};
use crate::store::Store;

use super::types::{Notice, Phase, Stage};
use super::Session;

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
    corrections: &[mining::CorrectionRule],
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
    let corrected = mining::apply_corrections(&transcript, corrections);
    let (paste_text, archive_polished, polish_failed) = match polish_opts {
        Some(opts) => {
            stage(Stage::Polish);
            match gateway::polish_with_options(key, &corrected, phrases, opts, polish_model) {
                Ok(text) => (text.clone(), text, false),
                Err(_) => (corrected.clone(), String::new(), true),
            }
        }
        None => (corrected.clone(), String::new(), false),
    };
    stage(Stage::Insert);
    Outcome::Ready {
        transcript,
        paste_text,
        archive_polished,
        polish_failed,
    }
}

impl Session {
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
        let Some(recording) = self.recording.take() else {
            return;
        };
        drop(recording.finish());
        self.studio_dictating = false;
        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];
        self.note(Notice::Warn("已取消".into()), cx);
    }

    /// Drops an in-progress recording or transcription without pasting.
    pub fn cancel_active(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.cancel_recording(cx);
            return;
        }
        if matches!(self.phase, Phase::Working(_)) {
            self.generation = self.generation.wrapping_add(1);
            self.stages = None;
            self.working_progress = 0.0;
            self.level = 0.0;
            self.wave = [0.0; crate::capture::WAVE_BARS];
            self.note(Notice::Warn("已取消".into()), cx);
        }
    }

    pub(super) fn begin(&mut self, for_studio: bool, cx: &mut Context<Self>) {
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

    pub(super) fn finish(&mut self, cx: &mut Context<Self>) {
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
        let corrections = self
            .store
            .lock()
            .map(|store| store.corrections())
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
        let generation = self.generation;

        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];

        let (sender, receiver) = mpsc::channel::<Stage>();
        self.stages = Some(receiver);
        self.exiting = false;
        self.appearance = self.appearance.wrapping_add(1);
        self.phase = Phase::Working(Stage::Transcribe);
        self.working_progress = 0.08;
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
                        &corrections,
                        polish_opts.as_ref(),
                        &transcribe_model,
                        &polish_model,
                        &stage,
                    )
                })
                .await;
            let _ = this.update(cx, |session, cx| {
                if session.generation != generation || !matches!(session.phase, Phase::Working(_)) {
                    return;
                }
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
        self.working_progress = 1.0;
        let mut inserted_text = String::new();
        let mut pasted = false;
        let mut inserted_row = None;
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
                    inserted_text = paste_text.clone();
                    let (inserted, restore) = paste::insert_text(&paste_text);
                    pasted = matches!(inserted, InsertResult::Pasted);
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
                    inserted_row = Some(row_id);
                    if self.save_audio && !wav.is_empty() {
                        if store.save_audio_file(row_id, wav).is_ok() {
                            self.audio_ids.insert(row_id);
                        }
                    }
                }
                self.history = store.history().unwrap_or_default();
                self.stats = store.stats();
            }
        }
        if !for_studio && pasted {
            if let Some(row_id) = inserted_row {
                self.watch_paste_correction(row_id, inserted_text, cx);
            }
        }
        self.maybe_mine(cx);
        self.note(notice, cx);
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
        hotkey::set_cancel_armed(matches!(
            self.phase,
            Phase::Listening | Phase::Working(_)
        ));
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
