use gpui_kit::component::input::Input;
use gpui_kit::component::IconName;
use gpui_kit::component::Sizable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rems, App, Context, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::polish;
use crate::store::HistoryItem;
use crate::ui;
use crate::views::components::{button, empty_state, history_card_row};
use crate::views::SettingsView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageItem {
    Number(usize),
    Ellipsis,
}

fn compute_page_items(current: usize, total: usize) -> Vec<PageItem> {
    if total <= 7 {
        return (1..=total).map(PageItem::Number).collect();
    }
    let mut items = Vec::new();
    if current <= 4 {
        for p in 1..=5 {
            items.push(PageItem::Number(p));
        }
        items.push(PageItem::Ellipsis);
        items.push(PageItem::Number(total));
    } else if current >= total - 3 {
        items.push(PageItem::Number(1));
        items.push(PageItem::Ellipsis);
        for p in (total - 4)..=total {
            items.push(PageItem::Number(p));
        }
    } else {
        items.push(PageItem::Number(1));
        items.push(PageItem::Ellipsis);
        items.push(PageItem::Number(current - 1));
        items.push(PageItem::Number(current));
        items.push(PageItem::Number(current + 1));
        items.push(PageItem::Ellipsis);
        items.push(PageItem::Number(total));
    }
    items
}

fn page_btn(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    disabled: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let mut btn = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.0))
        .min_w(px(30.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .text_size(rems(0.75))
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        });

    if disabled {
        btn = btn
            .text_color(ui::fg3(cx))
            .bg(ui::well_bg(cx).opacity(0.4))
            .cursor_default();
    } else if active {
        btn = btn
            .text_color(ui::on_accent(cx))
            .bg(ui::accent(cx))
            .cursor_default();
    } else {
        btn = btn
            .text_color(ui::fg(cx))
            .bg(ui::well_bg(cx))
            .border_1()
            .border_color(ui::hairline(cx))
            .cursor_pointer()
            .hover(|st| st.bg(ui::card_hover(cx)))
            .on_click(move |_, window, cx| action(window, cx));
    }
    btn.child(label.into())
}

impl SettingsView {
    pub fn render_history(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let search_input = self.history_search_input.clone();
        let query = search_input.read(cx).value().trim().to_lowercase();
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
        let page_size = self.history_page_size.max(1);
        let total_pages = total_count.div_ceil(page_size).max(1);
        let current_page = self.history_page.clamp(1, total_pages);
        if self.history_page != current_page {
            self.history_page = current_page;
        }

        let start_idx = (current_page - 1) * page_size;
        let displayed_items: Vec<HistoryItem> = filtered
            .into_iter()
            .skip(start_idx)
            .take(page_size)
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // Search & Action Toolbar (Clean without mode chips)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(280.0))
                            .child(Input::new(&search_input).large().w_full()),
                    )
                    .when(!all_history_empty, |row| {
                        let s = session_handle.clone();
                        let v = view.clone();
                        let v_cancel = view.clone();
                        if confirm_clear {
                            row.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(button(
                                        "confirm-clear-btn",
                                        "确认清空全部",
                                        true,
                                        move |_, cx| {
                                            s.update(cx, |session, cx| session.clear_history(cx));
                                            v.update(cx, |view, cx| {
                                                view.confirm_clear_history = false;
                                                view.history_page = 1;
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
                                    )),
                            )
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
            )
            // History List Card
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
                                "尝试修改搜索关键词。"
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
                                idx + 1 == count,
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
                    // Bottom Pagination Bar
                    .when(total_count > 0, |card| {
                        let v_prev = view.clone();
                        let v_next = view.clone();
                        let app = &**cx;
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
                                            "共 {total_count} 条记录 · 第 {current_page} / {total_pages} 页",
                                        )),
                                )
                                .when(total_pages > 1, |row| {
                                    let page_items = compute_page_items(current_page, total_pages);
                                    let v_page = view.clone();
                                    row.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(page_btn(
                                                "prev-page",
                                                "上一页",
                                                false,
                                                current_page <= 1,
                                                move |_, cx| {
                                                    v_prev.update(cx, |view, cx| {
                                                        view.history_page = view
                                                            .history_page
                                                            .saturating_sub(1);
                                                        cx.notify();
                                                    });
                                                },
                                                app,
                                            ))
                                            .children(page_items.into_iter().enumerate().map(
                                                move |(idx, item)| match item {
                                                    PageItem::Number(p) => {
                                                        let vp = v_page.clone();
                                                        let is_active = p == current_page;
                                                        page_btn(
                                                            format!("page-{p}"),
                                                            format!("{p}"),
                                                            is_active,
                                                            false,
                                                            move |_, cx| {
                                                                vp.update(cx, |view, cx| {
                                                                    view.history_page = p;
                                                                    cx.notify();
                                                                });
                                                            },
                                                            app,
                                                        )
                                                        .into_any_element()
                                                    }
                                                    PageItem::Ellipsis => div()
                                                        .id(SharedString::from(format!("page-ell-{idx}")))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .h(px(30.0))
                                                        .min_w(px(24.0))
                                                        .text_size(rems(0.75))
                                                        .text_color(ui::fg3(app))
                                                        .child("…")
                                                        .into_any_element(),
                                                },
                                            ))
                                            .child(page_btn(
                                                "next-page",
                                                "下一页",
                                                false,
                                                current_page >= total_pages,
                                                move |_, cx| {
                                                    v_next.update(cx, |view, cx| {
                                                        view.history_page = view
                                                            .history_page
                                                            .saturating_add(1);
                                                        cx.notify();
                                                    });
                                                },
                                                app,
                                            )),
                                    )
                                }),
                        )
                    }),
            )
    }
}
