use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Sizable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Entity, FontWeight, Hsla, IntoElement, ParentElement,
    Styled, div, px, rems,
};

use crate::session::Session;
use crate::ui;
use crate::views::components::primitives::{button, ellipsize};
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
    key_value: &str,
    key_input: &Entity<InputState>,
    session: &Entity<Session>,
    view: &Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .px(px(16.0))
        .py(px(14.0))
        .border_b_1()
        .border_color(ui::hairline(cx));

    if editing {
        let save_input = key_input.clone();
        let save_session = session.clone();
        let save_view = view.clone();
        let cancel_view = view.clone();
        row.child(
            div()
                .w(px(180.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child("API Key"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(rems(1.45))
                        .text_color(ui::fg2(cx))
                        .child("访问密钥，安全保存在本机。"),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(Input::new(key_input).large().w_full()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
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
                .when(has_key, |btns| {
                    btns.child(button("cancel-key", "取消", false, move |_, cx| {
                        cancel_view.update(cx, |view, cx| {
                            view.key_editing = false;
                            cx.notify();
                        });
                    }))
                }),
        )
    } else {
        let edit_view = view.clone();
        let edit_input = key_input.clone();
        let current_key = key_value.to_string();
        let clear_session = session.clone();
        let clear_view = view.clone();
        let masked = crate::gateway::mask_key(key_value);

        row.child(
            div()
                .w(px(180.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child("API Key"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(rems(1.45))
                        .text_color(ui::fg2(cx))
                        .child("已安全保存在本地。"),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(5.0))
                        .rounded(px(6.0))
                        .bg(ui::well_bg(cx))
                        .border_1()
                        .border_color(ui::hairline(cx))
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child(masked),
                )
                .child(ui::pill("已就绪", ui::ok(cx), ui::ok_soft(cx))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(button("edit-key", "修改 Key", false, move |window, cx| {
                    let k = current_key.clone();
                    edit_input.update(cx, |input, cx| {
                        input.set_value(&k, window, cx);
                    });
                    edit_view.update(cx, |view, cx| {
                        view.key_editing = true;
                        cx.notify();
                    });
                }))
                .child(button("clear-key", "清除", false, move |_, cx| {
                    clear_session.update(cx, |session, cx| session.clear_key(cx));
                    clear_view.update(cx, |view, cx| {
                        view.key_editing = true;
                        cx.notify();
                    });
                })),
        )
    }
}

pub fn base_url_row(
    base_url_input: &Entity<InputState>,
    session: &Entity<Session>,
    cx: &App,
) -> impl IntoElement {
    let save_input = base_url_input.clone();
    let save_session = session.clone();
    let reset_input = base_url_input.clone();
    let reset_session = session.clone();

    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .px(px(16.0))
        .py(px(14.0))
        .border_b_1()
        .border_color(ui::hairline(cx))
        .child(
            div()
                .w(px(180.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child("接口地址 (Base URL)"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(rems(1.45))
                        .text_color(ui::fg2(cx))
                        .child("默认为 Vercel AI Gateway，也可填自定义反代。"),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(Input::new(base_url_input).large().w_full()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(button("save-base-url", "保存地址", false, move |_, cx| {
                    let val = save_input.read(cx).value();
                    save_session.update(cx, |session, cx| session.set_api_base_url(val.as_ref(), cx));
                }))
                .child(button("reset-base-url", "默认", false, move |window, cx| {
                    let default_url = crate::gateway::DEFAULT_BASE_URL;
                    reset_input.update(cx, |input, cx| input.set_value(default_url, window, cx));
                    reset_session.update(cx, |session, cx| session.set_api_base_url(default_url, cx));
                })),
        )
}

pub fn connection_status_row(
    testing: bool,
    test_status: Option<(bool, String)>,
    session: &Entity<Session>,
    cx: &App,
) -> impl IntoElement {
    let test_session = session.clone();
    let proxy_detected = crate::gateway::detect_macos_system_proxy();

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .px(px(16.0))
        .py(px(14.0))
        .child(
            div()
                .w(px(180.0))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg(cx))
                        .child("网络与连接测试"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(rems(1.45))
                        .text_color(ui::fg2(cx))
                        .child("验证当前 Key 与 Base URL 连通性。"),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .flex_wrap()
                .gap(px(10.0))
                .when_some(proxy_detected, |row, proxy| {
                    row.child(ui::pill(
                        format!("已适配系统代理 {proxy}"),
                        ui::accent(cx),
                        ui::accent_soft(cx),
                    ))
                })
                .when_some(test_status, |row, (ok, msg)| {
                    row.child(ui::pill(
                        ellipsize(&msg, 40),
                        if ok { ui::ok(cx) } else { ui::error(cx) },
                        if ok {
                            ui::ok_soft(cx)
                        } else {
                            ui::error(cx).opacity(0.14)
                        },
                    ))
                }),
        )
        .child(button(
            "test-connection-btn",
            if testing { "正在测试…" } else { "测试连接" },
            testing,
            move |_, cx| {
                test_session.update(cx, |session, cx| session.test_gateway(cx));
            },
        ))
}
