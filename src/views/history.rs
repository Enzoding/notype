use gpui_kit::component::input::Input;
use gpui_kit::component::Sizable;
use gpui_kit::component::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{Context, IntoElement, ParentElement, Styled, div, px};

use crate::polish;
use crate::store::{HistoryItem, Store};
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

        let (all_history, playing_id, repolishing_id) = {
            let s = self.session.read(cx);
            (s.history.clone(), s.playing_id, s.repolishing_id)
        };
        let expanded_ids = self.expanded_raw_ids.clone();
        let audio_dir = Store::data_dir().unwrap_or_default().join("audio");

        let filtered: Vec<HistoryItem> = all_history
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
            .collect();

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
                            .when(!all_history.is_empty(), |row| {
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
                    .when(filtered.is_empty(), |card| {
                        card.child(empty_state(
                            IconName::Search,
                            if all_history.is_empty() {
                                "还没有任何口述记录"
                            } else {
                                "没有匹配的口述记录"
                            },
                            if all_history.is_empty() {
                                "使用快捷键在任意应用中口述，所有转写原文、润色文本与录音都会保存在本机。"
                            } else {
                                "尝试切换上方的模式筛选或修改搜索关键词。"
                            },
                            cx,
                        ))
                    })
                    .children({
                        let count = filtered.len();
                        let s = session_handle.clone();
                        let v = view.clone();
                        let exp_ids = expanded_ids.clone();
                        let a_dir = audio_dir.clone();
                        filtered.into_iter().enumerate().map(move |(idx, item)| {
                            let has_audio = a_dir.join(format!("{}.wav", item.id)).is_file();
                            let is_playing = playing_id == Some(item.id);
                            let is_expanded = exp_ids.contains(&item.id);
                            let is_repolishing = repolishing_id == Some(item.id);
                            history_card_row(
                                item,
                                idx + 1 == count,
                                has_audio,
                                is_playing,
                                is_expanded,
                                is_repolishing,
                                copied_token.clone(),
                                s.clone(),
                                v.clone(),
                                cx,
                            )
                        })
                    }),
            )
    }
}
