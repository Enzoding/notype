use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rems, App, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled,
};

use crate::paste;
use crate::polish::{self, PolishMode};
use crate::session::{format_duration_ms, format_relative_time, Session};
use crate::store::HistoryItem;
use crate::ui;
use crate::views::components::primitives::{button, icon_button, small_action_pill};
use crate::views::SettingsView;

#[allow(clippy::too_many_arguments)]
pub fn history_card_row(
    item: HistoryItem,
    last: bool,
    has_audio: bool,
    is_playing: bool,
    is_expanded: bool,
    is_repolishing: bool,
    is_editing: bool,
    copied_token: Option<String>,
    correction_input: Entity<TextareaState>,
    session: Entity<Session>,
    view: Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let id = item.id;
    let app_name = polish::friendly_app_name(&item.bundle_id);
    let mode_label = PolishMode::from_str(&item.mode)
        .unwrap_or_else(|| polish::mode_for_bundle(&item.bundle_id))
        .short_label();
    let duration_str = format_duration_ms(item.duration_ms);
    let rel_time = format_relative_time(item.created_at);
    let display_text = item.display_text().to_string();
    let raw_text = item.transcript.clone();
    let has_distinct_raw = item.has_distinct_raw();
    let chars_count = item.char_count();

    let copy_pol_tok = format!("pol-{id}");
    let copy_raw_tok = format!("raw-{id}");
    let is_pol_copied = copied_token.as_deref() == Some(copy_pol_tok.as_str());
    let is_raw_copied = copied_token.as_deref() == Some(copy_raw_tok.as_str());

    div()
        .id(SharedString::from(format!("hist-row-{id}")))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(14.0))
        .when(!last, |row| row.border_b_1().border_color(ui::hairline(cx)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(ui::pill(app_name, ui::fg(cx), ui::well_bg(cx)))
                        .child(ui::pill(mode_label, ui::accent(cx), ui::accent_soft(cx)))
                        .when(!duration_str.is_empty(), |row| {
                            row.child(
                                div()
                                    .text_size(rems(0.6875))
                                    .text_color(ui::fg3(cx))
                                    .child(format!("录音 {duration_str}")),
                            )
                        })
                        .child(
                            div()
                                .text_size(rems(0.6875))
                                .text_color(ui::fg3(cx))
                                .child(format!("{chars_count} 字 · {rel_time}")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .when(has_audio, |bar| {
                            let s = session.clone();
                            bar.child(small_action_pill(
                                format!("play-audio-{id}"),
                                if is_playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                },
                                if is_playing { "停止" } else { "回放" },
                                move |_, cx| {
                                    s.update(cx, |session, cx| {
                                        session.toggle_play_history(id, cx);
                                    });
                                },
                                cx,
                            ))
                        })
                        .when(has_distinct_raw, |bar| {
                            let v = view.clone();
                            bar.child(small_action_pill(
                                format!("toggle-raw-{id}"),
                                IconName::FileText,
                                if is_expanded {
                                    "收起原文"
                                } else {
                                    "对比原文"
                                },
                                move |_, cx| {
                                    v.update(cx, |view, cx| {
                                        view.toggle_raw_expand(id, cx);
                                    });
                                },
                                cx,
                            ))
                        })
                        .child({
                            let text_for_copy = display_text.clone();
                            let v = view.clone();
                            let tok = copy_pol_tok;
                            small_action_pill(
                                format!("copy-pol-{id}"),
                                if is_pol_copied {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                },
                                if is_pol_copied { "已复制" } else { "复制" },
                                move |_, cx| {
                                    if paste::copy_to_clipboard(&text_for_copy) {
                                        v.update(cx, |view, cx| {
                                            view.flash_copied(tok.clone(), cx);
                                        });
                                    }
                                },
                                cx,
                            )
                        })
                        .child({
                            let v = view.clone();
                            let text_for_fix = display_text.clone();
                            small_action_pill(
                                format!("fix-{id}"),
                                IconName::Replace,
                                if is_editing { "收起修正" } else { "修正" },
                                move |window, cx| {
                                    if is_editing {
                                        v.update(cx, |view, cx| {
                                            view.editing_history_id = None;
                                            cx.notify();
                                        });
                                    } else {
                                        v.update(cx, |view, cx| {
                                            view.begin_history_correction(
                                                id,
                                                &text_for_fix,
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                },
                                cx,
                            )
                        })
                        .child({
                            let s = session.clone();
                            let item_clone = item.clone();
                            small_action_pill(
                                format!("repolish-{id}"),
                                IconName::RefreshCw,
                                if is_repolishing {
                                    "润色中…"
                                } else {
                                    "重润色"
                                },
                                move |_, cx| {
                                    s.update(cx, |session, cx| {
                                        session.repolish_history_item(item_clone.clone(), cx);
                                    });
                                },
                                cx,
                            )
                        })
                        .child({
                            let s = session.clone();
                            icon_button(
                                format!("del-hist-{id}"),
                                IconName::Close,
                                "删除记录",
                                move |_, cx| {
                                    s.update(cx, |session, cx| {
                                        session.delete_history(id, cx);
                                    });
                                },
                                cx,
                            )
                        }),
                ),
        )
        .child(
            div()
                .text_size(rems(0.84375))
                .line_height(rems(1.38))
                .text_color(ui::fg(cx))
                .child(display_text),
        )
        .when(is_editing, |col| {
            let save_session = session.clone();
            let cancel_view = view.clone();
            let save_view = view.clone();
            col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(ui::well_bg(cx))
                    .border_1()
                    .border_color(ui::hairline(cx))
                    .child(
                        div()
                            .text_size(rems(0.6875))
                            .text_color(ui::fg3(cx))
                            .child("修正后会保存到历史，并只学习你改动里的术语。"),
                    )
                    .child(
                        div()
                            .w_full()
                            .child(Textarea::new(&correction_input).h(px(110.0)).w_full()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.0))
                            .child(button(
                                "cancel-history-fix",
                                "取消",
                                false,
                                move |_, cx| {
                                    cancel_view.update(cx, |view, cx| {
                                        view.editing_history_id = None;
                                        cx.notify();
                                    });
                                },
                            ))
                            .child(button(
                                "save-history-fix",
                                "保存修正并学习",
                                true,
                                move |_, cx| {
                                    let corrected = correction_input.read(cx).value().to_string();
                                    if corrected.trim().is_empty() {
                                        return;
                                    }
                                    save_session.update(cx, |session, cx| {
                                        session.apply_history_correction(id, &corrected, cx);
                                    });
                                    save_view.update(cx, |view, cx| {
                                        view.editing_history_id = None;
                                        cx.notify();
                                    });
                                },
                            )),
                    ),
            )
        })
        .when(is_expanded && has_distinct_raw, |col| {
            let v = view;
            let raw_copy = raw_text.clone();
            let tok = copy_raw_tok;
            col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(ui::well_bg(cx))
                    .border_1()
                    .border_color(ui::hairline(cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(rems(0.6875))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ui::fg3(cx))
                                    .child("语音转写原文（未润色）"),
                            )
                            .child(small_action_pill(
                                format!("copy-raw-btn-{id}"),
                                if is_raw_copied {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                },
                                if is_raw_copied {
                                    "已复制原文"
                                } else {
                                    "复制原文"
                                },
                                move |_, cx| {
                                    if paste::copy_to_clipboard(&raw_copy) {
                                        v.update(cx, |view, cx| {
                                            view.flash_copied(tok.clone(), cx);
                                        });
                                    }
                                },
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .text_size(rems(0.78125))
                            .line_height(rems(1.25))
                            .text_color(ui::fg2(cx))
                            .child(raw_text),
                    ),
            )
        })
}
