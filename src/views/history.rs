use gpui_kit::component::input::Input;
use gpui_kit::component::Sizable;
use gpui_kit::component::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{Context, IntoElement, ParentElement, Styled, div, px, rems};

use crate::polish;
use crate::store::HistoryItem;
use crate::ui;
use crate::views::SettingsView;
use crate::views::components::{button, empty_state, history_card_row, mode_chip};

impl SettingsView {
    pub fn render_history(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let search_input = self.history_search_input.clone();
        let query = search_input.read(cx).value().trim().to_lowercase();
        let mode_filter = self.history_mode_filter.clone();
        let confirm_clear = self.confirm_clear_history;
        let copied_token = self.copied_token.clone();

        let (all_history_empty, playing_id, repolishing_id, audio_ids) = {
            let s = self.session.read(cx);
            (
                s.history.is_empty(),
                s.playing_id,
                s.repolishing_id,
                s.audio_ids.clone(),
            )
        };
        let expanded_ids = self.expanded_raw_ids.clone();
        let correction_input = self.correction_input.clone();
        let editing_history_id = self.editing_history_id;

        let filtered: Vec<HistoryItem> = {
            let s = self.session.read(cx);
            s.history
                .iter()
                .filter(|item| {
                    let item_mode = if item.mode.is_empty() {
                        polish::mode_for_bundle(&item.bundle_id).as_str()
                    } else {
                        item.mode.as_str()
                    };
                    let mode_ok = mode_filter == "all" || item_mode == mode_filter;
                    if !mode_ok {
                        return false;
                    }
                    if query.is_empty() {
                        return true;
                    }
                    let app_name = polish::friendly_app_name(&item.bundle_id).to_lowercase();
                    item.transcript.to_lowercase().contains(&query)
                        || item.polished.to_lowercase().contains(&query)
                        || app_name.contains(&query)
                })
                .cloned()
                .collect()
        };

        let total_count = filtered.len();
        let display_limit = self.history_display_limit;
        let to_display = total_count.min(display_limit);
        let has_more = total_count > to_display;
        let displayed_items: Vec<HistoryItem> = filtered.into_iter().take(to_display).collect();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Search & Filter Toolbar
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(260.0))
                            .child(Input::new(&search_input).large().w_full()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .children(
                                [
                                    ("all", "全部"),
                                    ("prose", "正文"),
                                    ("literal", "代码"),
                                    ("email", "邮件"),
                                    ("chat", "聊天"),
                                ]
                                .into_iter()
                                .map(|(code, label)| {
                                    let active = mode_filter == code;
                                    let v = view.clone();
                                    mode_chip(
                                        format!("hist-filter-{code}"),
                                        label,
                                        active,
                                        move |_, cx| {
                                            v.update(cx, |view, cx| {
                                                view.history_mode_filter = code.to_string();
                                                cx.notify();
                                            });
                                        },
                                        cx,
                                    )
                                }),
                            )
                            .when(!all_history_empty, |row| {
                                let s = session_handle.clone();
                                let v = view.clone();
                                let v_cancel = view.clone();
                                if confirm_clear {
                                    row.child(button(
                                        "confirm-clear-btn",
                                        "确认清空全部",
                                        true,
                                        move |_, cx| {
                                            s.update(cx, |session, cx| session.clear_history(cx));
                                            v.update(cx, |view, cx| {
                                                view.confirm_clear_history = false;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                    .child(button(
                                        "cancel-clear-btn",
                                        "取消",
                                        false,
                                        move |_, cx| {
                                            v_cancel.update(cx, |view, cx| {
                                                view.confirm_clear_history = false;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                } else {
                                    row.child(button(
                                        "ask-clear-btn",
                                        "清空记录",
                                        false,
                                        move |_, cx| {
                                            v.update(cx, |view, cx| {
                                                view.confirm_clear_history = true;
                                                cx.notify();
                                            });
                                        },
                                    ))
                                }
                            }),
                    ),
            )
            // Filtered History List Card
            .child(
                ui::card(cx)
                    .when(displayed_items.is_empty(), |card| {
                        card.child(empty_state(
                            IconName::Search,
                            if all_history_empty {
                                "还没有任何口述记录"
                            } else {
                                "没有匹配的口述记录"
                            },
                            if all_history_empty {
                                "使用快捷键在任意应用中口述，所有转写原文、润色文本与录音都会保存在本机。"
                            } else {
                                "尝试切换上方的模式筛选或修改搜索关键词。"
                            },
                            cx,
                        ))
                    })
                    .children({
                        let count = displayed_items.len();
                        let s = session_handle.clone();
                        let v = view.clone();
                        let exp_ids = expanded_ids.clone();
                        let a_ids = audio_ids.clone();
                        let app = &**cx;
                        displayed_items.into_iter().enumerate().map(move |(idx, item)| {
                            let has_audio = a_ids.contains(&item.id);
                            let is_playing = playing_id == Some(item.id);
                            let is_expanded = exp_ids.contains(&item.id);
                            let is_repolishing = repolishing_id == Some(item.id);
                            let is_editing = editing_history_id == Some(item.id);
                            history_card_row(
                                item,
                                !has_more && idx + 1 == count,
                                has_audio,
                                is_playing,
                                is_expanded,
                                is_repolishing,
                                is_editing,
                                copied_token.clone(),
                                correction_input.clone(),
                                s.clone(),
                                v.clone(),
                                app,
                            )
                        })
                    })
                    .when(has_more, |card| {
                        let v_more = view.clone();
                        let v_all = view.clone();
                        card.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(16.0))
                                .py(px(12.0))
                                .bg(ui::well_bg(cx))
                                .child(
                                    div()
                                        .text_size(rems(0.75))
                                        .text_color(ui::fg3(cx))
                                        .child(format!(
                                            "已显示前 {to_display} 条，共 {total_count} 条口述记录"
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(button(
                                            "load-more-history",
                                            "加载更多 (+25)",
                                            false,
                                            move |_, cx| {
                                                v_more.update(cx, |view, cx| {
                                                    view.history_display_limit = view
                                                        .history_display_limit
                                                        .saturating_add(25);
                                                    cx.notify();
                                                });
                                            },
                                        ))
                                        .child(button(
                                            "show-all-history",
                                            "显示全部",
                                            false,
                                            move |_, cx| {
                                                v_all.update(cx, |view, cx| {
                                                    view.history_display_limit = usize::MAX;
                                                    cx.notify();
                                                });
                                            },
                                        )),
                                ),
                        )
                    }),
            )
    }
}
