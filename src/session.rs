use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable, Size};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Animation, AnimationExt, App, BoxShadow, Context, ElementId, Entity, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, hsla, linear_color_stop, linear_gradient, px,
    rems, rgb,
};

use crate::capture::ActiveRecording;
use crate::front::front_bundle_id;
use crate::gateway::{self, POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::paste::{self, InsertResult};
use crate::secrets::{self, KeySource};
use crate::store::{HistoryItem, Phrase, Store};
use crate::ui;

/// The three real steps of the pipeline. The overlay reports the step that is
/// actually running instead of guessing at a percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Transcribe,
    Polish,
    Insert,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Transcribe => "正在转写",
            Stage::Polish => "正在订正",
            Stage::Insert => "正在粘贴",
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

/// What the HUD says when a run ends, and how it should look saying it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Done(&'static str),
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
            _ => Duration::from_millis(2000),
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
    pub key_status: String,
    pub phrases: Vec<Phrase>,
    pub history: Vec<HistoryItem>,
    key: String,
    store: Arc<Mutex<Store>>,
    recording: Option<ActiveRecording>,
    stages: Option<Receiver<Stage>>,
    generation: u64,
    note_id: u64,
    /// Bumped whenever the overlay should replay its entrance or exit.
    appearance: u64,
    /// True while the HUD is fading out; the phase stays on the last notice
    /// until the fade finishes.
    exiting: bool,
    /// Guards the delayed hide so a quick re-show does not blank the window.
    hide_epoch: u64,
}

impl Session {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let store = Store::open().expect("打开本机数据库");
        let (key, source) = secrets::load_key();
        let phrases = store.phrases().unwrap_or_default();
        let history = store.history().unwrap_or_default();
        let show_dock = store.show_dock();
        Self {
            phase: Phase::Idle,
            level: 0.0,
            wave: [0.0; crate::capture::WAVE_BARS],
            show_dock,
            key_status: key_status(&key, &source),
            phrases,
            history,
            key,
            store: Arc::new(Mutex::new(store)),
            recording: None,
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

    pub fn watch_level(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let mut interval = Duration::from_millis(100);
                let alive = this
                    .update(cx, |session, cx| {
                        let mut changed = false;
                        if matches!(session.phase, Phase::Listening) {
                            let level = crate::capture::level();
                            let wave = crate::capture::wave();
                            if (session.level - level).abs() > 0.01 || session.wave != wave {
                                session.level = level;
                                session.wave = wave;
                                changed = true;
                            }
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
                        if matches!(session.phase, Phase::Listening | Phase::Working(_)) {
                            interval = Duration::from_millis(16);
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
            let _ = store.add_phrase(phrase);
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

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish(cx);
        } else if !matches!(self.phase, Phase::Working(_)) {
            self.begin(cx);
        }
    }

    fn begin(&mut self, cx: &mut Context<Self>) {
        if self.key.is_empty() {
            self.note(Notice::Error("先保存 AI Gateway Key，再开始口述".into()), cx);
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.level = 0.0;
        self.wave = [0.0; crate::capture::WAVE_BARS];
        self.exiting = false;
        self.appearance = self.appearance.wrapping_add(1);
        self.phase = Phase::Listening;
        self.sync_voice(cx);
        cx.notify();
        match ActiveRecording::start() {
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
            Err(err) => self.note(Notice::Error(err), cx),
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        if recording.elapsed() < Duration::from_millis(300) {
            drop(recording.finish());
            self.note(Notice::Warn("录音太短了".into()), cx);
            return;
        }
        let recorded = recording.finish();
        let wav = recorded.to_wav();
        let bundle_id = front_bundle_id();
        let phrases = self
            .store
            .lock()
            .map(|store| store.phrase_list())
            .unwrap_or_default();
        let key = self.key.clone();
        let bundle_for_task = bundle_id.clone();
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
                    run_pipeline(&key, &wav, &phrases, &bundle_for_task, &stage)
                })
                .await;
            let _ = this.update(cx, |session, cx| {
                session.deliver(outcome, &bundle_id, &store, cx);
            });
        })
        .detach();
    }

    fn deliver(
        &mut self,
        outcome: Outcome,
        bundle_id: &str,
        store: &Arc<Mutex<Store>>,
        cx: &mut Context<Self>,
    ) {
        self.stages = None;
        let (notice, saved) = match outcome {
            Outcome::Failed(message) => (Notice::Error(message), None),
            Outcome::Ready {
                transcript,
                paste_text,
                archive_polished,
                polish_failed,
            } => {
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
                let notice = if polish_failed {
                    match inserted {
                        InsertResult::Pasted => Notice::Warn("订正失败，已贴入转写原文".into()),
                        InsertResult::Copied => Notice::Warn("订正失败，原文已复制".into()),
                    }
                } else {
                    match inserted {
                        InsertResult::Pasted => Notice::Done("已贴入"),
                        InsertResult::Copied => Notice::Copied,
                    }
                };
                (notice, Some((transcript, archive_polished)))
            }
        };
        if let Some((transcript, polished)) = saved {
            if let Ok(store) = store.lock() {
                let _ = store.add_history(bundle_id, &transcript, &polished);
                self.history = store.history().unwrap_or_default();
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

    /// Fades the HUD out while its content is still on screen, then hides the
    /// window. Hiding the window is a hard alpha switch, so the fade has to
    /// happen before it.
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

    /// Shows the HUD immediately; hides it after the exit fade instead of
    /// snapping the window's alpha between two frames.
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
    bundle_id: &str,
    stage: &(dyn Fn(Stage) + Send + Sync),
) -> Outcome {
    stage(Stage::Transcribe);
    let transcript = match gateway::transcribe(key, wav, phrases) {
        Ok(text) => text,
        Err(err) => return Outcome::Failed(err),
    };
    if transcript.trim().is_empty() {
        return Outcome::Failed("没有识别到内容".into());
    }
    stage(Stage::Polish);
    let (paste_text, archive_polished, polish_failed) =
        match gateway::polish(key, &transcript, bundle_id, phrases) {
            Ok(text) => (text.clone(), text, false),
            Err(_) => (transcript.clone(), String::new(), true),
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
        return "还没有 Key".into();
    }
    match source {
        KeySource::File(_) => "已从 .env.local 读取".into(),
        KeySource::Missing => "还没有 Key".into(),
    }
}

fn mmss(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
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

fn app_label(bundle_id: &str) -> String {
    bundle_id
        .rsplit('.')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(bundle_id)
        .to_string()
}

pub struct SettingsView {
    session: Entity<Session>,
    key_input: Entity<InputState>,
    phrase_input: Entity<InputState>,
    key_editing: bool,
}

impl SettingsView {
    pub fn new(
        session: Entity<Session>,
        key_input: Entity<InputState>,
        phrase_input: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let key_editing = !session.read(cx).has_key();
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&key_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value();
                view.session
                    .update(cx, |session, cx| session.save_key(value.as_ref(), cx));
                view.key_editing = false;
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&phrase_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value();
                view.session
                    .update(cx, |session, cx| session.add_phrase(value.as_ref(), cx));
            }
        })
        .detach();
        Self {
            session,
            key_input,
            phrase_input,
            key_editing,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (phase, phrases, history, key_status, has_key, elapsed, show_dock) = {
            let session = self.session.read(cx);
            (
                session.phase.clone(),
                session.phrases.clone(),
                session.history.clone(),
                session.key_status.clone(),
                session.has_key(),
                session.elapsed(),
                session.show_dock,
            )
        };

        let key_editing = self.key_editing || !has_key;
        let view = cx.entity();
        let session_handle = self.session.clone();
        let key_input = self.key_input.clone();
        let phrase_input = self.phrase_input.clone();
        let key_path = secrets::env_path().display().to_string();

        div()
            .id("settings")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .text_color(ui::fg(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(22.0))
                    .px(px(20.0))
                    .pt(px(18.0))
                    .pb(px(24.0))
                    .child(header(&phase, elapsed, cx))
                    .child(hero(
                        &phase,
                        elapsed,
                        has_key,
                        session_handle.clone(),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(ui::section_label("连接", None).text_color(ui::fg3(cx)))
                            .child(
                                ui::card(cx)
                                    .child(key_row(
                                        key_editing,
                                        has_key,
                                        &key_input,
                                        &session_handle,
                                        &view,
                                        cx,
                                    ))
                                    .child(path_row(&key_path, &key_status, cx)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(14.0))
                                    .flex_wrap()
                                    .text_size(rems(0.6875))
                                    .text_color(ui::fg3(cx))
                                    .child(format!("转写 {TRANSCRIBE_MODEL}"))
                                    .child(format!("订正 {POLISH_MODEL}"))
                                    .child("菜单栏常驻"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                ui::section_label(
                                    "词库",
                                    if phrases.is_empty() {
                                        None
                                    } else {
                                        Some(format!("{}", phrases.len()))
                                    },
                                )
                                .text_color(ui::fg3(cx))
                                .justify_between(),
                            )
                            .child(
                                ui::card(cx)
                                    .child(phrase_input_row(
                                        &phrase_input,
                                        &session_handle,
                                        cx,
                                    ))
                                    .when(phrases.is_empty(), |card| {
                                        card.child(empty_state(
                                            IconName::BookOpen,
                                            "还没有词条",
                                            "把常说的专有名词加进来，转写和订正都会照着念。",
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
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(ui::section_label("最近的口述", None).text_color(ui::fg3(cx)))
                            .child(
                                ui::card(cx)
                                    .when(history.is_empty(), |card| {
                                        card.child(empty_state(
                                            IconName::FileText,
                                            "还没有记录",
                                            "说完一句后，整理结果会出现在这里。",
                                            cx,
                                        ))
                                    })
                                    .children({
                                        let hairline = ui::hairline(cx);
                                        let tertiary = ui::fg3(cx);
                                        let secondary = ui::fg2(cx);
                                        history.into_iter().map(move |item| {
                                            let text = if item.polished.is_empty() {
                                                item.transcript
                                            } else {
                                                item.polished
                                            };
                                            history_row(
                                                &item.bundle_id,
                                                &text,
                                                hairline,
                                                tertiary,
                                                secondary,
                                            )
                                        })
                                    }),
                            ),
                    )
                    .child(dock_hint(show_dock, cx)),
            )
    }
}

fn header(phase: &Phase, elapsed: Duration, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(rems(1.0625))
                .font_weight(FontWeight::SEMIBOLD)
                .child("OpenTypeless"),
        )
        .child(status_pill(phase, elapsed, cx))
}

fn status_pill(phase: &Phase, elapsed: Duration, cx: &App) -> impl IntoElement {
    let (color, label) = match phase {
        Phase::Idle => (ui::ok(cx), "就绪".to_string()),
        Phase::Listening => (ui::live(cx), format!("正在听 {}", mmss(elapsed))),
        Phase::Working(stage) => (ui::warn(cx), stage.label().to_string()),
        Phase::Message(Notice::Error(_)) | Phase::Message(Notice::Warn(_)) => {
            (ui::warn(cx), "需要处理".to_string())
        }
        Phase::Message(_) => (ui::ok(cx), "完成".to_string()),
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(22.0))
        .px(px(9.0))
        .rounded(px(11.0))
        .bg(ui::card_bg(cx))
        .child(div().size(px(6.0)).rounded_full().bg(color))
        .child(
            div()
                .text_size(rems(0.6875))
                .text_color(ui::fg2(cx))
                .child(label),
        )
}

fn hero(
    phase: &Phase,
    elapsed: Duration,
    has_key: bool,
    session: Entity<Session>,
    cx: &App,
) -> impl IntoElement {
    let card = div()
        .flex()
        .items_center()
        .gap(px(14.0))
        .p(px(14.0))
        .rounded(px(12.0))
        .bg(ui::card_bg(cx))
        .border_1()
        .border_color(ui::hairline(cx));

    match phase {
        Phase::Listening => card
            .child(div().size(px(7.0)).rounded_full().bg(ui::live(cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(rems(0.84375))
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("正在听 {}", mmss(elapsed))),
                    )
                    .child(
                        div()
                            .text_size(rems(0.71875))
                            .text_color(ui::fg2(cx))
                            .child("再按一次右 Option 结束，最长 120 秒"),
                    ),
            )
            .child(div().flex_1())
            .child(button("hero-stop", "结束", true, move |_, cx| {
                session.update(cx, |session, cx| session.toggle(cx));
            })),
        Phase::Working(stage) => card
            .child(Spinner::new().with_size(Size::Small))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(rems(0.84375))
                            .font_weight(FontWeight::MEDIUM)
                            .child(stage.label()),
                    )
                    .child(
                        div()
                            .text_size(rems(0.71875))
                            .text_color(ui::fg2(cx))
                            .child(format!("第 {} 步，共 3 步", stage.index() + 1)),
                    ),
            ),
        Phase::Message(notice) => {
            let (icon, color) = notice_look(notice, cx);
            card.child(Icon::new(icon).size(px(15.0)).text_color(color))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(rems(0.84375))
                                .font_weight(FontWeight::MEDIUM)
                                .child(ellipsize(notice.text(), 48)),
                        )
                        .child(
                            div()
                                .text_size(rems(0.71875))
                                .text_color(ui::fg2(cx))
                                .child("结果会贴到最前面的窗口"),
                        ),
                )
        }
        Phase::Idle => card
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(28.0))
                    .px(px(9.0))
                    .rounded(px(7.0))
                    .bg(ui::card_hover(cx))
                    .border_1()
                    .border_color(ui::hairline(cx))
                    .text_size(rems(0.8125))
                    .text_color(ui::fg2(cx))
                    .child("⌥ →"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(rems(0.84375))
                            .font_weight(FontWeight::MEDIUM)
                            .child("开始口述"),
                    )
                    .child(
                        div()
                            .text_size(rems(0.71875))
                            .text_color(ui::fg2(cx))
                            .child("按右 Option 开始，再按一次结束"),
                    ),
            )
            .child(div().flex_1())
            .child(button(
                "hero-start",
                if has_key { "开始" } else { "需要 Key" },
                true,
                move |_, cx| {
                    session.update(cx, |session, cx| session.toggle(cx));
                },
            )),
    }
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
                .child("Gateway Key"),
        )
        .child(div().flex_1().min_w(px(0.0)).child(Input::new(key_input).w_full()))
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
        row.child(
            div()
                .text_size(rems(0.8125))
                .child("Gateway Key"),
        )
        .child(
            div()
                .flex_1()
                .text_size(rems(0.75))
                .text_color(ui::fg2(cx))
                .child("vck_••••••••••••••••"),
        )
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

fn path_row(path: &str, status: &str, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(9.0))
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
                .text_size(rems(0.6875))
                .text_color(ui::fg3(cx))
                .child(ellipsize(status, 28)),
        )
}

fn phrase_input_row(
    phrase_input: &Entity<InputState>,
    session: &Entity<Session>,
    cx: &App,
) -> impl IntoElement {
    let input = phrase_input.clone();
    let target = session.clone();
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(10.0))
        .border_b_1()
        .border_color(ui::hairline(cx))
        .child(Input::new(phrase_input).w_full())
        .child(button("add-phrase", "加入", false, move |window, cx| {
            let value = input.read(cx).value();
            target.update(cx, |session, cx| session.add_phrase(value.as_ref(), cx));
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }))
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
        .child(
            div()
                .flex_1()
                .text_size(rems(0.8125))
                .child(label),
        )
        .child(
            div()
                .id(SharedString::from(format!("delete-{id}")))
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.0))
                .rounded(px(6.0))
                .opacity(0.3)
                .group_hover("phrase-row", |style| style.opacity(0.9))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .aria_label(accessible)
                .on_click(move |_, _, cx| {
                    session.update(cx, |session, cx| session.delete_phrase(id, cx));
                })
                .child(
                    Icon::new(IconName::Close).size(px(12.0)).text_color(dim),
                ),
        )
}

fn history_row(
    bundle_id: &str,
    text: &str,
    hairline: Hsla,
    tertiary: Hsla,
    secondary: Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .px(px(14.0))
        .py(px(11.0))
        .border_b_1()
        .border_color(hairline)
        .child(
            div()
                .text_size(rems(0.65625))
                .text_color(tertiary)
                .child(app_label(bundle_id)),
        )
        .child(
            div()
                .text_size(rems(0.78125))
                .line_height(rems(1.171875))
                .text_color(secondary)
                .child(ellipsize(text, 160)),
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
        .gap(px(4.0))
        .py(px(18.0))
        .child(Icon::new(icon).size(px(16.0)).text_color(ui::fg3(cx)))
        .child(
            div()
                .text_size(rems(0.75))
                .text_color(ui::fg2(cx))
                .child(title),
        )
        .child(
            div()
                .max_w(px(300.0))
                .text_size(rems(0.6875))
                .text_color(ui::fg3(cx))
                .child(body),
        )
}

fn dock_hint(show_dock: bool, cx: &App) -> impl IntoElement {
    div()
        .text_size(rems(0.6875))
        .text_color(ui::fg3(cx))
        .child(if show_dock {
            "菜单栏的 OT 常驻。Dock 图标可以在它的菜单里关掉。"
        } else {
            "菜单栏的 OT 常驻。Dock 图标已隐藏。"
        })
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
        let (phase, wave, appearance, exiting) = {
            let session = self.session.read(cx);
            (
                session.phase.clone(),
                session.wave,
                session.appearance,
                session.exiting,
            )
        };

        let content = match &phase {
            Phase::Idle => div(),
            Phase::Listening => listening_view(wave),
            Phase::Working(stage) => working_view(*stage),
            Phase::Message(notice) => message_view(notice, cx),
        };

        div()
            .flex()
            .flex_col()
            .justify_end()
            .items_center()
            .size_full()
            .pb(px(8.0))
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
        .h(px(52.0))
        .px(px(15.0))
        .rounded(px(16.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgb(0x1b1b1f), 0.0),
            linear_color_stop(rgb(0x111113), 1.0),
        ))
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.075))
        .shadow(vec![
            BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 1.0, 0.10)).inset(),
            BoxShadow::new(px(0.0), px(12.0), hsla(0.0, 0.0, 0.0, 0.55))
                .blur_radius(px(28.0))
                .spread_radius(px(-6.0)),
        ])
        .text_size(rems(0.8125))
        .text_color(ui::hud_fg())
}

fn listening_view(wave: [f32; crate::capture::WAVE_BARS]) -> gpui_kit::Div {
    hud_shell()
        .gap(px(11.0))
        .child(div().size(px(7.0)).rounded_full().bg(rgb(0xff453a)))
        .child(div().font_weight(FontWeight::MEDIUM).child("正在听"))
        .child(wave_view(wave))
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
            let alpha = 0.35 + 0.65 * (1.0 - distance).clamp(0.0, 1.0);
            div()
                .w(px(2.5))
                .h(px(height))
                .rounded(px(1.25))
                .bg(hsla(0.0, 0.0, 1.0, alpha))
        }))
}

fn working_view(stage: Stage) -> gpui_kit::Div {
    hud_shell()
        .flex_col()
        .items_stretch()
        .justify_center()
        .h(px(64.0))
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
                        .text_size(rems(0.71875))
                        .text_color(ui::hud_fg_dim())
                        .child(format!("第 {} / 3 步", stage.index() + 1)),
                ),
        )
        .child(segments(stage))
}

fn segments(stage: Stage) -> gpui_kit::Div {
    let active = stage.index();
    div().flex().gap(px(3.0)).children((0..3).map(move |index| {
        let color = if index < active {
            hsla(0.0, 0.0, 1.0, 0.92)
        } else if index == active {
            hsla(0.0, 0.0, 1.0, 0.5)
        } else {
            hsla(0.0, 0.0, 1.0, 0.16)
        };
        div().flex_1().h(px(2.0)).rounded(px(1.0)).bg(color)
    }))
}

fn message_view(notice: &Notice, _cx: &App) -> gpui_kit::Div {
    let (icon, color) = hud_notice_look(notice);
    let shell = hud_shell().gap(px(9.0)).child(Icon::new(icon).size(px(15.0)).text_color(color));
    match notice {
        Notice::Error(_) => shell.child(
            div()
                .max_w(px(320.0))
                .text_size(rems(0.8125))
                .child(ellipsize(notice.text(), 46)),
        ),
        _ => shell.child(div().child(notice.text().to_string())),
    }
}
