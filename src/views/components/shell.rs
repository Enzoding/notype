use std::time::Duration;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, hsla, px, rems, App, ElementId, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};

use crate::hotkey;
use crate::session::{mmss, Notice, Phase, Session};
use crate::store::TriggerMode;
use crate::ui;
use crate::views::components::primitives::button;
use crate::views::{NavTab, SettingsView};

pub fn nav_item(
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
        .h(px(36.0))
        .px(px(8.0))
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

pub fn sidebar_status_card(
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
        .p(px(12.0))
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
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().size(px(6.5)).rounded_full().bg(if is_rec {
                            ui::live(cx)
                        } else if has_key {
                            ui::ok(cx)
                        } else {
                            ui::warn(cx)
                        }))
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .font_weight(FontWeight::MEDIUM)
                                .child(if is_rec {
                                    crate::i18n::t_args(
                                        "shell.recording",
                                        &[("time", &mmss(elapsed))],
                                    )
                                } else if has_key {
                                    crate::i18n::t("shell.ready_to_dictate").into()
                                } else {
                                    crate::i18n::t("shell.no_api_key").into()
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
            if is_rec {
                crate::i18n::t("shell.finish_and_transcribe")
            } else {
                crate::i18n::t("shell.start_dictation")
            },
            is_rec,
            move |_, cx| {
                session.update(cx, |s, cx| {
                    if !s.studio_dictating
                        && matches!(s.phase, Phase::Listening | Phase::Working(_))
                    {
                        return;
                    }
                    s.toggle(cx);
                });
            },
        ))
}

pub fn status_pill(phase: &Phase, elapsed: Duration, cx: &App) -> impl IntoElement {
    let (color, label) = match phase {
        Phase::Idle => (ui::ok(cx), crate::i18n::t("shell.status_ready").to_string()),
        Phase::Listening => (
            ui::live(cx),
            crate::i18n::t_args("shell.status_listening", &[("time", &mmss(elapsed))]),
        ),
        Phase::Working(stage) => (ui::brand_teal(cx), stage.label().to_string()),
        Phase::Message(Notice::Error(_)) => (
            ui::error(cx),
            crate::i18n::t("shell.status_attention").to_string(),
        ),
        Phase::Message(Notice::Warn(_)) => (
            ui::warn(cx),
            crate::i18n::t("shell.status_attention").to_string(),
        ),
        Phase::Message(_) => (ui::ok(cx), crate::i18n::t("shell.status_done").to_string()),
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(28.0))
        .px(px(12.0))
        .rounded(px(9999.0))
        .bg(ui::card_bg(cx))
        .border_1()
        .border_color(ui::hairline(cx))
        .child(div().size(px(6.5)).rounded_full().bg(color))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::fg2(cx))
                .child(label),
        )
}

#[allow(dead_code)]
pub fn notice_look(notice: &Notice, cx: &App) -> (IconName, Hsla) {
    match notice {
        Notice::Done(_) => (IconName::Check, ui::ok(cx)),
        Notice::Copied => (IconName::Copy, ui::fg2(cx)),
        Notice::Warn(_) => (IconName::TriangleAlert, ui::warn(cx)),
        Notice::Error(_) => (IconName::CircleAlert, ui::error(cx)),
    }
}
