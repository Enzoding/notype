use std::collections::HashSet;

use gpui_kit::component::Sizable;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::component::input::Input;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::Size;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rems,
};

use crate::mining::{AutoLearnMode, PRESET_PHRASES};
use crate::paste;
use crate::session::format_relative_time;
use crate::ui;
use crate::views::SettingsView;
use crate::views::components::{button, candidate_row, empty_state, icon_button, phrase_row};

impl SettingsView {
    pub fn render_dictionary(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let phrase_input = self.phrase_input.clone();
        let copied_token = self.copied_token.clone();
        let (phrases, candidates, corrections, mining, mining_status, auto_mode, last_mined_at) = {
            let s = self.session.read(cx);
            (
                s.phrases.clone(),
                s.candidates.clone(),
                s.corrections.clone(),
                s.mining,
                s.mining_status.clone(),
                s.auto_learn_mode,
                s.last_mined_at,
            )
        };

        let app = &**cx;
        let auto_on = auto_mode != AutoLearnMode::Off;
        let status_text = if last_mined_at > 0 {
            format!("上次学习：{}", format_relative_time(last_mined_at))
        } else {
            "还没有学习过".to_string()
        };

        let existing_phrases: HashSet<String> = phrases
            .iter()
            .map(|p| p.phrase.to_lowercase())
            .collect();

        let total_phrases = phrases.len();
        let display_limit = self.dict_display_limit;
        let to_display = total_phrases.min(display_limit);
        let has_more = total_phrases > to_display;
        let displayed_phrases: Vec<_> = phrases.iter().take(to_display).cloned().collect();

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            // 1. Automatic learning card
            .child(
                ui::card(cx)
                    .p(px(16.0))
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(rems(0.84375))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("自动学习词条"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.71875))
                                            .text_color(ui::fg2(cx))
                                            .child("从历史成稿里提取产品名、人名与术语。高置信直接加入，其余进「待确认」；只分析文字，不上传录音。"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_shrink_0()
                                    .gap(px(10.0))
                                    .when(mining, |row| {
                                        row.child(Spinner::new().with_size(Size::Small))
                                    })
                                    .child({
                                        let s = session_handle.clone();
                                        Switch::new("auto-learn-switch")
                                            .checked(auto_on)
                                            .accessibility_label("自动学习词条")
                                            .on_change(move |checked, _, cx| {
                                                let mode = if *checked {
                                                    AutoLearnMode::Auto
                                                } else {
                                                    AutoLearnMode::Off
                                                };
                                                s.update(cx, |session, cx| {
                                                    session.set_auto_learn_mode(mode, cx);
                                                });
                                            })
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .flex_wrap()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .children(
                                        [AutoLearnMode::Auto, AutoLearnMode::Suggest, AutoLearnMode::Off]
                                            .into_iter()
                                            .map(|mode| {
                                                let active = auto_mode == mode;
                                                let s = session_handle.clone();
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "learn-mode-{}",
                                                        mode.as_str()
                                                    )))
                                                    .flex()
                                                    .items_center()
                                                    .h(px(28.0))
                                                    .px(px(10.0))
                                                    .rounded(px(9999.0))
                                                    .bg(if active {
                                                        ui::accent(cx)
                                                    } else {
                                                        ui::canvas_bg(cx)
                                                    })
                                                    .border_1()
                                                    .border_color(if active {
                                                        ui::accent(cx)
                                                    } else {
                                                        ui::hairline(cx)
                                                    })
                                                    .cursor_pointer()
                                                    .hover(|st| {
                                                        if active {
                                                            st
                                                        } else {
                                                            st.bg(ui::card_hover(cx))
                                                        }
                                                    })
                                                    .text_size(rems(0.6875))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(if active {
                                                        ui::on_accent(cx)
                                                    } else {
                                                        ui::fg(cx)
                                                    })
                                                    .on_click(move |_, _, cx| {
                                                        s.update(cx, |session, cx| {
                                                            session.set_auto_learn_mode(mode, cx);
                                                        });
                                                    })
                                                    .child(mode.label())
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.6875))
                                            .text_color(ui::fg3(cx))
                                            .child(status_text),
                                    ),
                            )
                            .child({
                                let s = session_handle.clone();
                                button(
                                    "learn-now-btn",
                                    if mining { "正在学习…" } else { "立即学习" },
                                    false,
                                    move |_, cx| {
                                        s.update(cx, |session, cx| session.learn_now(cx));
                                    },
                                )
                            }),
                    )
                    .when(!mining_status.is_empty(), |card| {
                        card.child(
                            div()
                                .text_size(rems(0.6875))
                                .text_color(ui::fg3(cx))
                                .child(mining_status),
                        )
                    })
                    .when(candidates.is_empty(), |card| {
                        card.child(
                            div()
                                .text_size(rems(0.6875))
                                .text_color(ui::fg3(cx))
                                .child(format!(
                                    "粘贴后 8 秒内如果你改了文字，会从改动里学习术语；当前 {} 条口述等待分析。",
                                    self.session.read(cx).history.len()
                                )),
                        )
                    }),
            )
            // 2. Pending candidates
            .when(!candidates.is_empty(), |page| {
                let count = candidates.len();
                let s_accept = session_handle.clone();
                let s_ignore = session_handle.clone();
                page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            ui::section_label("待确认词条", Some(format!("共 {count} 项")))
                                .text_color(ui::fg2(cx))
                                .justify_between(),
                        )
                        .child(
                            ui::card(cx)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.0))
                                        .px(px(14.0))
                                        .py(px(8.0))
                                        .border_b_1()
                                        .border_color(ui::hairline(cx))
                                        .child(button(
                                            "accept-all-candidates",
                                            "全部接受",
                                            false,
                                            move |_, cx| {
                                                s_accept.update(cx, |session, cx| {
                                                    session.accept_all_candidates(cx);
                                                });
                                            },
                                        ))
                                        .child(button(
                                            "ignore-all-candidates",
                                            "全部忽略",
                                            false,
                                            move |_, cx| {
                                                s_ignore.update(cx, |session, cx| {
                                                    session.ignore_all_candidates(cx);
                                                });
                                            },
                                        )),
                                )
                                .children({
                                    let last = candidates.len();
                                    let session = session_handle.clone();
                                    let hairline = ui::hairline(cx);
                                    let hover = ui::card_hover(cx);
                                    let dim = ui::fg2(cx);
                                    candidates.into_iter().enumerate().map(
                                        move |(index, candidate)| {
                                            candidate_row(
                                                candidate,
                                                index + 1 == last,
                                                session.clone(),
                                                hairline,
                                                hover,
                                                dim,
                                                app,
                                            )
                                        },
                                    )
                                }),
                        ),
                )
            })
            // 3. Learned spelling corrections
            .when(!corrections.is_empty(), |page| {
                let count = corrections.len();
                let s = session_handle.clone();
                page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            ui::section_label("拼写纠正", Some(format!("共 {count} 条")))
                                .text_color(ui::fg2(app))
                                .justify_between(),
                        )
                        .child(
                            ui::card(app)
                                .child(
                                    div()
                                        .px(px(14.0))
                                        .py(px(10.0))
                                        .border_b_1()
                                        .border_color(ui::hairline(app))
                                        .text_size(rems(0.6875))
                                        .text_color(ui::fg3(app))
                                        .child("下次转写后自动替换为「正确写法」，来自你在历史或粘贴后的修正。"),
                                )
                                .children({
                                    let last = corrections.len();
                                    let session = s;
                                    corrections.into_iter().enumerate().map(move |(index, rule)| {
                                        let wrong = rule.wrong;
                                        let right = rule.right;
                                        let s = session.clone();
                                        let del_wrong = wrong.clone();
                                        let del_right = right.clone();
                                        div()
                                            .id(SharedString::from(format!("correction-{index}")))
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap(px(12.0))
                                            .px(px(14.0))
                                            .py(px(9.0))
                                            .when(index + 1 != last, |row| {
                                                row.border_b_1().border_color(ui::hairline(app))
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.75))
                                                            .text_color(ui::fg3(app))
                                                            .child(wrong),
                                                    )
                                                    .child(
                                                        Icon::new(IconName::ArrowRight)
                                                            .size(px(12.0))
                                                            .text_color(ui::fg3(app)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.8125))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .child(right),
                                                    ),
                                            )
                                            .child(icon_button(
                                                format!("delete-correction-{index}"),
                                                IconName::Close,
                                                "删除拼写纠正",
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.delete_correction(
                                                            &del_wrong,
                                                            &del_right,
                                                            cx,
                                                        );
                                                    });
                                                },
                                                app,
                                            ))
                                    })
                                }),
                        ),
                )
            })
            // 4. Add Phrase & Batch Toolbar Card
            .child(
                ui::card(cx)
                    .p(px(16.0))
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(rems(0.84375))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("添加专属词条"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.71875))
                                            .text_color(ui::fg2(cx))
                                            .child("支持一次输入多个词条（以逗号或换行分隔），转写声学模型与润色大模型会同时锁定拼写。"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_shrink_0()
                                    .gap(px(12.0))
                                    .child({
                                        let s = session_handle.clone();
                                        let v = view.clone();
                                        button(
                                            "import-phrases-clipboard",
                                            "从剪贴板导入",
                                            false,
                                            move |_, cx| {
                                                if let Some(clip) = paste::read_from_clipboard() {
                                                    s.update(cx, |session, cx| {
                                                        session.add_phrase(&clip, cx);
                                                    });
                                                    v.update(cx, |view, cx| {
                                                        view.flash_copied("dict-imported", cx);
                                                    });
                                                }
                                            },
                                        )
                                    })
                                    .when(!phrases.is_empty(), |row| {
                                        let all_text = phrases
                                            .iter()
                                            .map(|p| p.phrase.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        let v = view.clone();
                                        let is_copied =
                                            copied_token.as_deref() == Some("dict-exported");
                                        row.child(button(
                                            "export-phrases-clipboard",
                                            if is_copied { "已复制全部词条" } else { "导出全部" },
                                            false,
                                            move |_, cx| {
                                                if paste::copy_to_clipboard(&all_text) {
                                                    v.update(cx, |view, cx| {
                                                        view.flash_copied("dict-exported", cx);
                                                    });
                                                }
                                            },
                                        ))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(Input::new(&phrase_input).large().w_full()),
                            )
                            .child({
                                let input = phrase_input.clone();
                                let target = session_handle.clone();
                                button("add-phrase-btn", "添加词条", true, move |window, cx| {
                                    let value = input.read(cx).value().to_string();
                                    target.update(cx, |session, cx| session.add_phrase(&value, cx));
                                    input.update(cx, |inp, cx| inp.set_value("", window, cx));
                                })
                            }),
                    )
                    // Preset quick-add chips
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_size(rems(0.6875))
                                    .text_color(ui::fg3(cx))
                                    .child("常用推荐："),
                            )
                            .children(PRESET_PHRASES.iter().enumerate().map(|(idx, word)| {
                                let word_str = *word;
                                let exists = existing_phrases.contains(&word_str.to_lowercase());
                                let s = session_handle.clone();
                                div()
                                    .id(SharedString::from(format!("preset-phrase-{idx}")))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(8.0))
                                    .py(px(3.0))
                                    .rounded(px(6.0))
                                    .bg(if exists {
                                        ui::accent_soft(cx)
                                    } else {
                                        ui::well_bg(cx)
                                    })
                                    .text_size(rems(0.6875))
                                    .text_color(if exists {
                                        ui::accent(cx)
                                    } else {
                                        ui::fg2(cx)
                                    })
                                    .when(!exists, |chip| {
                                        chip.cursor_pointer()
                                            .hover(|st| st.bg(ui::card_hover(cx)))
                                            .on_click(move |_, _, cx| {
                                                s.update(cx, |session, cx| {
                                                    session.add_phrase(word_str, cx);
                                                });
                                            })
                                    })
                                    .child(if exists {
                                        format!("✓ {word_str}")
                                    } else {
                                        format!("+ {word_str}")
                                    })
                            })),
                    ),
            )
            // 5. Active Glossary List
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        ui::section_label(
                            "已启用词条",
                            Some(format!("共 {} 项 · 前 {} 项参与识别", total_phrases, crate::mining::ACTIVE_PHRASE_LIMIT)),
                        )
                        .text_color(ui::fg2(cx))
                        .justify_between(),
                    )
                    .child(
                        ui::card(cx)
                            .when(displayed_phrases.is_empty(), |card| {
                                card.child(empty_state(
                                    IconName::BookOpen,
                                    "词库还是空的",
                                    "点击上方「立即学习」，从已有口述里自动提取术语；也可以手动添加产品名、人名或技术术语。",
                                    cx,
                                ))
                            })
                            .children({
                                let count = displayed_phrases.len();
                                let session = session_handle.clone();
                                let hairline = ui::hairline(cx);
                                let hover = ui::card_hover(cx);
                                let dim = ui::fg2(cx);
                                displayed_phrases.into_iter().enumerate().map(move |(index, phrase)| {
                                    phrase_row(
                                        phrase.id,
                                        phrase.phrase,
                                        phrase.source,
                                        phrase.hits,
                                        !has_more && index + 1 == count,
                                        session.clone(),
                                        hairline,
                                        hover,
                                        dim,
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
                                        .px(px(14.0))
                                        .py(px(10.0))
                                        .bg(ui::well_bg(cx))
                                        .child(
                                            div()
                                                .text_size(rems(0.75))
                                                .text_color(ui::fg3(cx))
                                                .child(format!(
                                                    "已显示前 {to_display} 项，共 {total_phrases} 项词条"
                                                )),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(button(
                                                    "load-more-phrases",
                                                    "加载更多 (+50)",
                                                    false,
                                                    move |_, cx| {
                                                        v_more.update(cx, |view, cx| {
                                                            view.dict_display_limit = view
                                                                .dict_display_limit
                                                                .saturating_add(50);
                                                            cx.notify();
                                                        });
                                                    },
                                                ))
                                                .child(button(
                                                    "show-all-phrases",
                                                    "显示全部",
                                                    false,
                                                    move |_, cx| {
                                                        v_all.update(cx, |view, cx| {
                                                            view.dict_display_limit = usize::MAX;
                                                            cx.notify();
                                                        });
                                                    },
                                                )),
                                        ),
                                )
                            }),
                    ),
            )
    }
}
