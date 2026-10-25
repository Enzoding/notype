use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{IconName, Sizable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Entity, FontWeight, Hsla, IntoElement, ParentElement,
    Styled, div, px, rems,
};

use crate::session::Session;
use crate::ui;
use crate::views::components::primitives::{button, ellipsize, icon_button};
use crate::views::SettingsView;

pub fn setting_row(title: &str, subtitle: &str, border_bottom: bool, cx: &App) -> gpui_kit::Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .px(px(16.0))
        .py(px(14.0))
        .when(border_bottom, |row| {
            row.border_b_1().border_color(ui::hairline(cx))
        })
        .child(
            div()
                .w(px(220.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(rems(1.45))
                        .text_color(ui::fg2(cx))
                        .child(subtitle.to_string()),
                ),
        )
}

/// Wrapping cluster for a row of choices. Takes the space left beside the label.
pub fn choice_row() -> gpui_kit::Div {
    div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .flex_wrap()
        .items_center()
        .justify_start()
        .gap(px(8.0))
}

pub fn inline_wave_view(wave: [f32; crate::capture::WAVE_BARS], color: Hsla) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(18.0))
        .children(wave.into_iter().map(move |bar| {
            let height = (bar * 16.0).clamp(3.0, 16.0);
            div()
                .w(px(2.0))
                .h(px(height))
                .rounded(px(1.0))
                .bg(color)
        }))
}

pub fn key_row(
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
                .flex_shrink_0()
                .text_size(px(14.0))
                .font_weight(FontWeight::MEDIUM)
                .child("API Key"),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(Input::new(key_input).large().w_full()),
        )
        .child(button("save-key", "保存 Key", true, move |_window, cx| {
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
                .flex_shrink_0()
                .text_size(px(14.0))
                .font_weight(FontWeight::MEDIUM)
                .child("API Key"),
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
            if testing { "正在测试" } else { "测试连接" },
            false,
            move |_, cx| {
                test_session.update(cx, |session, cx| session.test_gateway(cx));
            },
        ))
        .child(button("edit-key", "更换 Key", false, move |_, cx| {
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

pub fn path_row(
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
                        if ok { ui::ok(cx) } else { ui::error(cx) },
                        if ok {
                            ui::ok_soft(cx)
                        } else {
                            ui::error(cx).opacity(0.14)
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
