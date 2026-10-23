use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Context, ElementId, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::capture::ActiveRecording;
use crate::front::front_bundle_id;
use crate::gateway::{self, POLISH_MODEL, TRANSCRIBE_MODEL};
use crate::paste::{self, InsertResult};
use crate::secrets::{self, KeySource};
use crate::store::{HistoryItem, Phrase, Store};

#[derive(Clone, Debug)]
pub enum Phase {
    Idle,
    Listening,
    Working(String),
    Message(String),
}

pub struct Session {
    pub phase: Phase,
    pub level: f32,
    pub wave: [f32; crate::capture::WAVE_BARS],
    pub progress: f32,
    pub show_dock: bool,
    pub key_status: String,
    pub phrases: Vec<Phrase>,
    pub history: Vec<HistoryItem>,
    key: String,
    store: Arc<Mutex<Store>>,
    recording: Option<ActiveRecording>,
    generation: u64,
    note_id: u64,
    work_started: Option<Instant>,
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
            progress: 0.0,
            show_dock,
            key_status: key_status(&key, &source),
            phrases,
            history,
            key,
            store: Arc::new(Mutex::new(store)),
            recording: None,
            generation: 0,
            note_id: 0,
            work_started: None,
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
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let level = crate::capture::level();
                let wave = crate::capture::wave();
                let alive = this
                    .update(cx, |session, cx| {
                        let mut changed = false;
                        if matches!(session.phase, Phase::Listening)
                            && ((session.level - level).abs() > 0.02 || session.wave != wave)
                        {
                            session.level = level;
                            session.wave = wave;
                            changed = true;
                        }
                        if matches!(session.phase, Phase::Working(_)) {
                            if let Some(started) = session.work_started {
                                let elapsed = started.elapsed().as_secs_f32();
                                let next = (1.0 - (-elapsed / 5.0).exp()) * 0.92;
                                if (session.progress - next).abs() > 0.008 {
                                    session.progress = next;
                                    changed = true;
                                }
                            }
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
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
            self.key_status = "输入框是空的。先粘贴 Key，再保存。".into();
            cx.notify();
            return;
        }
        match secrets::save_key(value) {
            Ok(_) => {
                self.key = value.to_string();
                self.key_status = "已保存到 .env.local".into();
            }
            Err(err) => self.key_status = format!("没能写入 .env.local：{err}"),
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

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish(cx);
        } else if !matches!(self.phase, Phase::Working(_)) {
            self.begin(cx);
        }
    }

    fn begin(&mut self, cx: &mut Context<Self>) {
        if self.key.is_empty() {
            self.note("先保存 AI Gateway Key，再开始口述", cx);
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.level = 0.0;
        self.phase = Phase::Listening;
        self.sync_voice();
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
            Err(err) => self.note(err, cx),
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        if recording.elapsed() < Duration::from_millis(300) {
            drop(recording.finish());
            self.note("录音太短了", cx);
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
        self.progress = 0.06;
        self.work_started = Some(Instant::now());
        self.phase = Phase::Working("正在处理".into());
        self.sync_voice();
        cx.notify();
        let store = Arc::clone(&self.store);
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move { run_pipeline(&key, &wav, &phrases, &bundle_for_task) })
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
        let (message, saved) = match outcome {
            Outcome::Failed(message) => (message, None),
            Outcome::Ready {
                transcript,
                paste_text,
                archive_polished,
                note,
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
                (
                    format!("{}。{note}", insert_label(inserted)),
                    Some((transcript, archive_polished)),
                )
            }
        };
        if let Some((transcript, polished)) = saved {
            if let Ok(store) = store.lock() {
                let _ = store.add_history(bundle_id, &transcript, &polished);
                self.history = store.history().unwrap_or_default();
            }
        }
        self.progress = 1.0;
        self.work_started = None;
        self.note(message, cx);
    }

    fn note(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.phase = Phase::Message(text.into());
        self.note_id = self.note_id.wrapping_add(1);
        let note_id = self.note_id;
        self.sync_voice();
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2200))
                .await;
            let _ = this.update(cx, |session, cx| {
                if session.note_id == note_id && matches!(session.phase, Phase::Message(_)) {
                    session.phase = Phase::Idle;
                    session.level = 0.0;
                    session.sync_voice();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn sync_voice(&self) {
        crate::overlay::set_voice_visible(!matches!(self.phase, Phase::Idle));
    }
}

enum Outcome {
    Failed(String),
    Ready {
        transcript: String,
        paste_text: String,
        archive_polished: String,
        note: String,
    },
}

fn run_pipeline(key: &str, wav: &[u8], phrases: &[String], bundle_id: &str) -> Outcome {
    let transcript = match gateway::transcribe(key, wav, phrases) {
        Ok(text) => text,
        Err(err) => return Outcome::Failed(err),
    };
    if transcript.trim().is_empty() {
        return Outcome::Failed("没有识别到内容".into());
    }
    match gateway::polish(key, &transcript, bundle_id, phrases) {
        Ok(text) => Outcome::Ready {
            note: model_note(),
            paste_text: text.clone(),
            archive_polished: text,
            transcript,
        },
        Err(err) => Outcome::Ready {
            note: format!("{err}。已保留转写原文"),
            paste_text: transcript.clone(),
            archive_polished: String::new(),
            transcript,
        },
    }
}

fn insert_label(result: InsertResult) -> &'static str {
    match result {
        InsertResult::Pasted => "已贴入",
        InsertResult::Copied => "已复制，请手动粘贴",
    }
}

fn model_note() -> String {
    format!("{TRANSCRIBE_MODEL} → {POLISH_MODEL}")
}

fn key_status(key: &str, source: &KeySource) -> String {
    if key.is_empty() {
        return "还没有 Key。粘贴到下面，然后保存。".into();
    }
    match source {
        KeySource::File(_) => "已从 .env.local 读取".into(),
        KeySource::Missing => "还没有 Key".into(),
    }
}

pub struct SettingsView {
    session: Entity<Session>,
    key_input: Entity<InputState>,
    phrase_input: Entity<InputState>,
}

impl SettingsView {
    pub fn new(
        session: Entity<Session>,
        key_input: Entity<InputState>,
        phrase_input: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&key_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value();
                view.session
                    .update(cx, |session, cx| session.save_key(value.as_ref(), cx));
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
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        let phase = match &session.phase {
            Phase::Idle => "按右 Option 开始，再按一次结束。".to_string(),
            Phase::Listening => "正在听。再按右 Option，或点结束口述。".to_string(),
            Phase::Working(text) | Phase::Message(text) => text.clone(),
        };
        let phrases = session.phrases.clone();
        let history = session.history.clone();
        let key_status = session.key_status.clone();
        let has_key = session.has_key();
        let show_dock = session.show_dock;
        let listening = matches!(session.phase, Phase::Listening);
        let working = matches!(session.phase, Phase::Working(_));
        let muted = cx.theme().muted_foreground;
        let session_for_dock = self.session.clone();
        let session_for_toggle = self.session.clone();
        let session_for_save = self.session.clone();
        let session_for_reload = self.session.clone();
        let key_input = self.key_input.clone();
        let phrase_for_add = self.phrase_input.clone();
        let session_for_add = self.session.clone();
        let key_path = secrets::env_path().display().to_string();
        let toggle_label = if listening { "结束口述" } else { "开始口述" };
        let save_primary = !has_key;
        let toggle_primary = has_key && !working;

        div()
            .id("settings")
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_6()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("OpenTypeless"),
                    )
                    .child(phase)
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("转写 {TRANSCRIBE_MODEL}")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("订正 {POLISH_MODEL}")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("菜单栏上的 OT 会一直保留。"),
                    )
                    .child(
                        button(
                            "dock-toggle",
                            if show_dock {
                                "从 Dock 中隐藏"
                            } else {
                                "在 Dock 中显示"
                            },
                            false,
                            move |_, cx| {
                                session_for_dock.update(cx, |session, cx| {
                                    session.set_dock(!session.show_dock, cx);
                                });
                            },
                        )
                        .self_start(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("AI Gateway Key"))
                    .child(div().text_sm().text_color(muted).child(key_status))
                    .child(Input::new(&self.key_input).w_full())
                    .child(div().text_sm().text_color(muted).child(key_path))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .when(!working, |row| {
                                row.child(button(
                                    "toggle",
                                    toggle_label,
                                    toggle_primary,
                                    move |_, cx| {
                                        session_for_toggle
                                            .update(cx, |session, cx| session.toggle(cx));
                                    },
                                ))
                            })
                            .child(button("save-key", "保存 Key", save_primary, move |_, cx| {
                                let value = key_input.read(cx).value();
                                session_for_save
                                    .update(cx, |session, cx| session.save_key(value.as_ref(), cx));
                            }))
                            .child(button("reload-key", "重新读取 Key", false, move |_, cx| {
                                session_for_reload
                                    .update(cx, |session, cx| session.reload_key(cx));
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("词库"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("这些词会原样用于转写和订正。"),
                    )
                    .child(Input::new(&self.phrase_input).w_full())
                    .child(
                        button("add-phrase", "加入词库", false, move |window, cx| {
                            let value = phrase_for_add.read(cx).value();
                            session_for_add
                                .update(cx, |session, cx| session.add_phrase(value.as_ref(), cx));
                            phrase_for_add.update(cx, |input, cx| input.set_value("", window, cx));
                        })
                        .self_start(),
                    )
                    .child(if phrases.is_empty() {
                        "还没有词条。在上面输入一个词，然后加入词库。"
                    } else {
                        "已保存的词条"
                    })
                    .children(phrases.into_iter().map(|phrase| {
                        let id = phrase.id;
                        let label = phrase.phrase.clone();
                        let session = self.session.clone();
                        let accessible = format!("删除 {label}");
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(div().flex_1().child(label))
                            .child(
                                button(
                                    SharedString::from(format!("phrase-{id}")),
                                    "删除词条",
                                    false,
                                    move |_, cx| {
                                        session.update(cx, |session, cx| {
                                            session.delete_phrase(id, cx);
                                        });
                                    },
                                )
                                .accessibility_label(accessible),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title("最近的口述"))
                    .child(if history.is_empty() {
                        "还没有记录。说完一句后，整理结果会出现在这里。"
                    } else {
                        ""
                    })
                    .children(history.into_iter().map(|item| {
                        let text = if item.polished.is_empty() {
                            item.transcript
                        } else {
                            item.polished
                        };
                        div().text_sm().child(text)
                    })),
            )
    }
}

fn section_title(title: &'static str) -> gpui_kit::Div {
    div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Button {
    let button = Button::new(id).label(label);
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
        let session = self.session.read(cx);
        let wave = session.wave;
        let progress = session.progress.clamp(0.0, 1.0);
        let body = match &session.phase {
            Phase::Listening => wave_view(wave),
            Phase::Working(_) => progress_view(progress),
            Phase::Message(text) => {
                div().text_sm().child(text.chars().take(28).collect::<String>())
            }
            Phase::Idle => div(),
        };
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap_3()
            .size_full()
            .px_3()
            .rounded_xl()
            .bg(rgb(0x111111))
            .text_color(rgb(0xffffff))
            .text_sm()
            .child(body)
    }
}

fn wave_view(wave: [f32; crate::capture::WAVE_BARS]) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(3.0))
        .h(px(36.0))
        .child("正在听")
        .children(wave.into_iter().enumerate().map(|(index, level)| {
            let height = px(4.0 + level.clamp(0.0, 1.0) * 28.0);
            div()
                .id(("wave", index))
                .w(px(3.0))
                .h(height)
                .rounded_full()
                .bg(rgb(0xffffff))
        }))
}

fn progress_view(progress: f32) -> gpui_kit::Div {
    let width = px(8.0 + progress * 210.0);
    let percent = (progress * 100.0).round() as u32;
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .justify_between()
                .w(px(230.0))
                .child("正在处理")
                .child(format!("{percent}%")),
        )
        .child(
            div()
                .w(px(230.0))
                .h(px(4.0))
                .rounded_full()
                .bg(rgb(0x3a3a3a))
                .child(div().w(width).h(px(4.0)).rounded_full().bg(rgb(0xffffff))),
        )
}
