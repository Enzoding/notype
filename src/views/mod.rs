pub mod components;
pub mod dictionary;
pub mod history;
pub mod overview;
mod providers;
pub mod settings;
mod setup;

use std::time::Duration;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rems, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window,
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
            NavTab::Overview => crate::i18n::t("nav.overview"),
            NavTab::History => crate::i18n::t("nav.history"),
            NavTab::Dictionary => crate::i18n::t("nav.dictionary"),
            NavTab::Settings => crate::i18n::t("nav.settings"),
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            NavTab::Overview => crate::i18n::t("nav.overview_sub"),
            NavTab::History => crate::i18n::t("nav.history_sub"),
            NavTab::Dictionary => crate::i18n::t("nav.dictionary_sub"),
            NavTab::Settings => crate::i18n::t("nav.settings_sub"),
        }
    }
}

pub const DEFAULT_HISTORY_LIMIT: usize = 25;
pub const DEFAULT_HISTORY_PAGE_SIZE: usize = 20;
pub const DEFAULT_DICT_LIMIT: usize = 50;

pub struct SettingsView {
    pub(crate) session: Entity<Session>,
    pub active_tab: NavTab,
    pub(crate) settings_section: settings::SettingsSection,
    pub(crate) models_expanded: bool,
    pub(crate) key_input: Entity<InputState>,
    pub(crate) transcribe_model_input: Entity<InputState>,
    pub(crate) polish_model_input: Entity<InputState>,
    pub(crate) recovery_limit: usize,
    pub(crate) phrase_input: Entity<InputState>,
    #[allow(dead_code)]
    pub(crate) studio_input: Entity<InputState>,
    pub(crate) history_search_input: Entity<InputState>,
    pub(crate) custom_prompt_input: Entity<InputState>,
    pub(crate) correction_input: Entity<TextareaState>,
    #[allow(dead_code)]
    pub(crate) studio_fix_input: Entity<TextareaState>,
    pub(crate) key_editing: bool,
    #[allow(dead_code)]
    pub history_mode_filter: String,
    pub expanded_raw_ids: Vec<i64>,
    pub editing_history_id: Option<i64>,
    #[allow(dead_code)]
    pub studio_fix_open: bool,
    pub copied_token: Option<String>,
    pub confirm_clear_history: bool,
    #[allow(dead_code)]
    pub history_display_limit: usize,
    pub history_page: usize,
    pub history_page_size: usize,
    pub dict_display_limit: usize,
}

impl SettingsView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: Entity<Session>,
        key_input: Entity<InputState>,
        transcribe_model_input: Entity<InputState>,
        polish_model_input: Entity<InputState>,
        phrase_input: Entity<InputState>,
        studio_input: Entity<InputState>,
        history_search_input: Entity<InputState>,
        custom_prompt_input: Entity<InputState>,
        correction_input: Entity<TextareaState>,
        studio_fix_input: Entity<TextareaState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let key_editing = session.read(cx).connections.key.is_empty();
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&key_input, |view, input, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let value = input.read(cx).value();
                view.session
                    .update(cx, |session, cx| session.save_key(value.as_ref(), cx));
                if !view.session.read(cx).connections.key.is_empty() {
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
        cx.subscribe(&history_search_input, |view, _, _: &InputEvent, cx| {
            view.history_page = 1;
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
            settings_section: Default::default(),
            models_expanded: false,
            key_input,
            transcribe_model_input,
            polish_model_input,
            recovery_limit: 25,
            phrase_input,
            studio_input,
            history_search_input,
            custom_prompt_input,
            correction_input,
            studio_fix_input,
            key_editing,
            history_mode_filter: "all".into(),
            expanded_raw_ids: Vec::new(),
            editing_history_id: None,
            studio_fix_open: false,
            copied_token: None,
            confirm_clear_history: false,
            history_display_limit: DEFAULT_HISTORY_LIMIT,
            history_page: 1,
            history_page_size: DEFAULT_HISTORY_PAGE_SIZE,
            dict_display_limit: DEFAULT_DICT_LIMIT,
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

    pub fn begin_history_correction(
        &mut self,
        id: i64,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing_history_id = Some(id);
        self.correction_input.update(cx, |input, cx| {
            input.set_value(text, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn begin_studio_fix(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.studio_fix_open = true;
        self.studio_fix_input.update(cx, |input, cx| {
            input.set_value(text, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
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
                    .min_h(px(0.0))
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
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .child(
                                                                div()
                                                                    .text_size(rems(0.875))
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .child("Notype"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px(px(4.0))
                                                                    .py(px(0.5))
                                                                    .rounded(px(4.0))
                                                                    .bg(ui::well_bg(cx))
                                                                    .text_size(px(10.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(ui::fg3(cx))
                                                                    .child(format!(
                                                                        "v{}",
                                                                        env!("CARGO_PKG_VERSION")
                                                                    )),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.65625))
                                                            .text_color(ui::fg3(cx))
                                                            .child(crate::i18n::t(
                                                                "shell.app_tagline",
                                                            )),
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
                    .when(active_tab == NavTab::Settings, |col| {
                        col.child(self.settings_navigation(cx))
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "scroll-{active_tab:?}-{:?}",
                                self.settings_section
                            )))
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .px(px(28.0))
                            .py(px(22.0))
                            .child(page_content),
                    ),
            )
    }
}
