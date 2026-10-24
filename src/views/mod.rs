pub mod components;
pub mod dictionary;
pub mod history;
pub mod overview;
pub mod settings;

use std::time::Duration;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::Icon;
use gpui_kit::{
    Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, rems,
};

use crate::session::{Phase, Session};
use crate::ui;
use components::{nav_item, sidebar_status_card, status_pill};

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
            NavTab::Settings => "快捷键、麦克风、润色和连接",
        }
    }
}

pub struct SettingsView {
    pub(crate) session: Entity<Session>,
    pub active_tab: NavTab,
    pub(crate) key_input: Entity<InputState>,
    pub(crate) phrase_input: Entity<InputState>,
    pub(crate) studio_input: Entity<InputState>,
    pub(crate) history_search_input: Entity<InputState>,
    pub(crate) custom_prompt_input: Entity<InputState>,
    pub(crate) key_editing: bool,
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

        let (
            phase,
            elapsed,
            phrases_len,
            history_len,
            has_key,
            hotkey_code,
            trigger_mode,
            studio_dictating,
        ) = {
            let s = self.session.read(cx);
            (
                s.phase.clone(),
                s.elapsed(),
                s.phrases.len(),
                s.history.len(),
                s.has_key(),
                s.hotkey_code,
                s.trigger_mode,
                s.studio_dictating,
            )
        };
        let shown_phase = if studio_dictating {
            phase.clone()
        } else {
            match &phase {
                Phase::Listening | Phase::Working(_) => Phase::Idle,
                other => other.clone(),
            }
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
                                                            .text_color(ui::on_accent(cx)),
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
                                    .gap(px(4.0))
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
                            &shown_phase,
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
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(active_tab.label()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .text_color(ui::fg2(cx))
                                            .child(active_tab.subtitle()),
                                    ),
                            )
                            .child(status_pill(&shown_phase, elapsed, cx)),
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
