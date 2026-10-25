use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::Sizable;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rems,
};

use crate::paste;
use crate::polish::PolishMode;
use crate::session::{Phase, format_duration_ms};
use crate::ui;
use crate::views::SettingsView;
use crate::views::components::{
    button, empty_state, hero_banner, hero_fill, history_card_row, metric_card, metric_fills,
    mode_chip, small_action_pill,
};

impl SettingsView {
    pub fn render_overview(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let studio_input = self.studio_input.clone();
        let studio_fix_input = self.studio_fix_input.clone();
        let studio_fix_open = self.studio_fix_open;
        let copied_token = self.copied_token.clone();

        let (
            phase,
            elapsed,
            wave,
            has_key,
            hotkey_code,
            trigger_mode,
            polish_strategy,
            stats,
            phrases_len,
            recent_history,
            studio_mode,
            studio_output,
            studio_raw,
            studio_busy,
            studio_dictating,
            playing_id,
            audio_ids,
        ) = {
            let s = self.session.read(cx);
            (
                s.phase.clone(),
                s.elapsed(),
                s.wave,
                s.has_key(),
                s.hotkey_code,
                s.trigger_mode,
                s.polish_strategy.clone(),
                s.stats.clone(),
                s.phrases.len(),
                s.history.iter().take(3).cloned().collect::<Vec<_>>(),
                s.studio_mode,
                s.studio_output.clone(),
                s.studio_raw.clone(),
                s.studio_busy,
                s.studio_dictating,
                s.playing_id,
                s.audio_ids.clone(),
            )
        };

        let expanded_ids = self.expanded_raw_ids.clone();
        let correction_input = self.correction_input.clone();
        let editing_history_id = self.editing_history_id;
        let repolishing_id = self.session.read(cx).repolishing_id;

        let strategy_label = match polish_strategy.as_str() {
            "prose" => "固定：流畅正文",
            "literal" => "固定：代码与终端",
            "email" => "固定：商务邮件",
            "chat" => "固定：即时通讯",
            "raw" => "仅语音转写",
            _ => "智能感知前台应用",
        };

        let display_phase = if studio_dictating {
            phase.clone()
        } else {
            match &phase {
                Phase::Listening | Phase::Working(_) => Phase::Idle,
                other => other.clone(),
            }
        };
        let fills = metric_fills(hero_fill(&display_phase));

        div()
            .flex()
            .flex_col()
            .gap(px(32.0))
            // 1. Hero Action & Live Status Card
            .child(hero_banner(
                &display_phase,
                elapsed,
                wave,
                has_key,
                hotkey_code,
                trigger_mode,
                strategy_label,
                session_handle.clone(),
                view.clone(),
                cx,
            ))
            // 2. 4-Column Real Metrics Strip
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.0))
                    .child(metric_card(
                        "累计口述",
                        format!("{}", stats.total_sessions),
                        if stats.total_duration_ms > 0 {
                            format!("累计时长 {}", format_duration_ms(stats.total_duration_ms))
                        } else {
                            "随时按键开始口述".into()
                        },
                        LucideIcon::Mic,
                        fills[0],
                        cx,
                    ))
                    .child(metric_card(
                        "累计产出字数",
                        format!("{}", stats.total_chars),
                        if stats.saved_minutes >= 0.1 {
                            format!("约节省 {:.1} 分钟打字时间", stats.saved_minutes)
                        } else {
                            "自动去除口癖与标点整理".into()
                        },
                        LucideIcon::Sparkles,
                        fills[1],
                        cx,
                    ))
                    .child(metric_card(
                        "平均口述语速",
                        if stats.avg_cpm > 0 {
                            format!("{}", stats.avg_cpm)
                        } else {
                            "—".into()
                        },
                        "字 / 分钟（含中英混排）".into(),
                        LucideIcon::Gauge,
                        fills[2],
                        cx,
                    ))
                    .child(metric_card(
                        "个人词库术语",
                        format!("{phrases_len}"),
                        "转写与润色双重拼写锁定".into(),
                        LucideIcon::BookOpen,
                        fills[3],
                        cx,
                    )),
            )
            // 3. Built-in Live Studio / Polish Playground (One Live Thing)
            .child(
                ui::feature_surface(ui::FeatureFill::Lavender, cx)
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(24.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ui::FeatureFill::Lavender.ink(cx))
                                    .child("润色与试音演练台"),
                            )
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(ui::body(cx))
                                    .child("无需切换窗口，直接测试语音转写，或试不同场景的润色"),
                            ),
                    )
                    .child(
                        ui::card(cx)
                            .p(px(24.0))
                            .gap(px(16.0))
                            // Mode selector & quick sample chips
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .flex_wrap()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .text_size(rems(0.71875))
                                                    .text_color(ui::fg3(cx))
                                                    .child("润色模式："),
                                            )
                                            .children(
                                                [
                                                    PolishMode::Prose,
                                                    PolishMode::Literal,
                                                    PolishMode::Email,
                                                    PolishMode::Chat,
                                                ]
                                                .into_iter()
                                                .map(|m| {
                                                    let selected = studio_mode == m;
                                                    let s = session_handle.clone();
                                                    mode_chip(
                                                        format!("studio-mode-{}", m.as_str()),
                                                        m.label(),
                                                        selected,
                                                        move |_, cx| {
                                                            s.update(cx, |session, cx| {
                                                                session.studio_mode = m;
                                                                cx.notify();
                                                            });
                                                        },
                                                        cx,
                                                    )
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
                                                    .text_size(rems(0.6875))
                                                    .text_color(ui::fg3(cx))
                                                    .child("示例草稿："),
                                            )
                                            .children(
                                                [
                                                    (
                                                        "纪要分点",
                                                        PolishMode::Prose,
                                                        "嗯那个今天下午我们主要对齐三件事吧，第一是先把OpenTypeless的侧边栏做好，然后第二件事是把历史记录的录音回放加上，不对，还要加个原文对比，最后是明天上午十点大家一起过一下测试",
                                                    ),
                                                    (
                                                        "代码终端",
                                                        PolishMode::Literal,
                                                        "呃帮我在终端跑一下 cargo test 然后看看 src 斜杠 session 点 rs 里面有没有 warning 那个 unused warning 要清掉",
                                                    ),
                                                    (
                                                        "商务邮件",
                                                        PolishMode::Email,
                                                        "喂王总你好，那个关于下周三的产品评审会，我想确认一下两个点，一个是参会名单定了吗，另一个是演示环境需要我们这边提前准备吗，麻烦今天下班前回复一下谢谢",
                                                    ),
                                                ]
                                                .into_iter()
                                                .enumerate()
                                                .map(|(idx, (title, target_mode, sample_text))| {
                                                    let input = studio_input.clone();
                                                    let s = session_handle.clone();
                                                    div()
                                                        .id(SharedString::from(format!("sample-{idx}")))
                                                        .px(px(8.0))
                                                        .py(px(3.0))
                                                        .rounded(px(6.0))
                                                        .bg(ui::well_bg(cx))
                                                        .cursor_pointer()
                                                        .hover(|st| st.bg(ui::card_hover(cx)))
                                                        .text_size(rems(0.6875))
                                                        .text_color(ui::fg2(cx))
                                                        .on_click(move |_, window, cx| {
                                                            input.update(cx, |inp, cx| {
                                                                inp.set_value(sample_text, window, cx);
                                                            });
                                                            s.update(cx, |session, cx| {
                                                                session.studio_mode = target_mode;
                                                                cx.notify();
                                                            });
                                                        })
                                                        .child(title)
                                                }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.71875))
                                    .text_color(ui::fg2(cx))
                                    .child(studio_mode.description()),
                            )
                            // Input row + action buttons
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .child(Input::new(&studio_input).large().w_full()),
                                    )
                                    .child({
                                        let s = session_handle.clone();
                                        let is_rec = matches!(phase, Phase::Listening) && studio_dictating;
                                        button(
                                            "studio-mic-btn",
                                            if is_rec { "停止试音" } else { "语音试音" },
                                            false,
                                            move |_, cx| {
                                                s.update(cx, |session, cx| {
                                                    session.toggle_studio_dictation(cx);
                                                });
                                            },
                                        )
                                    })
                                    .child({
                                        let s = session_handle.clone();
                                        let inp = studio_input.clone();
                                        button(
                                            "studio-run-btn",
                                            if studio_busy { "正在润色…" } else { "立即润色" },
                                            true,
                                            move |_, cx| {
                                                let val = inp.read(cx).value().to_string();
                                                s.update(cx, |session, cx| {
                                                    session.run_studio_polish(&val, cx);
                                                });
                                            },
                                        )
                                    }),
                            )
                            // Output comparison box
                            .when(!studio_output.is_empty() || studio_busy, |card| {
                                let out_copy = studio_output.clone();
                                let v = view.clone();
                                let is_copied = copied_token.as_deref() == Some("studio-out");
                                card.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.0))
                                        .p(px(12.0))
                                        .rounded(px(9.0))
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
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(8.0))
                                                        .child(ui::pill(
                                                            format!("{} · 润色结果", studio_mode.label()),
                                                            ui::accent(cx),
                                                            ui::accent_soft(cx),
                                                        ))
                                                        .when(!studio_output.is_empty(), |row| {
                                                            row.child(
                                                                div()
                                                                    .text_size(rems(0.6875))
                                                                    .text_color(ui::fg3(cx))
                                                                    .child(format!(
                                                                        "{} 字",
                                                                        studio_output.chars().count()
                                                                    )),
                                                            )
                                                        }),
                                                )
                                                .when(!studio_output.is_empty() && !studio_busy, |row| {
                                                    let fix_view = view.clone();
                                                    let fix_text = studio_output.clone();
                                                    row.child(
                                                        small_action_pill(
                                                            "fix-studio-out",
                                                            IconName::Replace,
                                                            if studio_fix_open { "收起修正" } else { "修正" },
                                                            move |window, cx| {
                                                                fix_view.update(cx, |view, cx| {
                                                                    if view.studio_fix_open {
                                                                        view.studio_fix_open = false;
                                                                        cx.notify();
                                                                    } else {
                                                                        view.begin_studio_fix(
                                                                            &fix_text,
                                                                            window,
                                                                            cx,
                                                                        );
                                                                    }
                                                                });
                                                            },
                                                            cx,
                                                        ),
                                                    )
                                                    .child(
                                                        small_action_pill(
                                                            "copy-studio-out",
                                                            if is_copied {
                                                                IconName::Check
                                                            } else {
                                                                IconName::Copy
                                                            },
                                                            if is_copied { "已复制" } else { "复制结果" },
                                                            move |_, cx| {
                                                                if paste::copy_to_clipboard(&out_copy) {
                                                                    v.update(cx, |view, cx| {
                                                                        view.flash_copied("studio-out", cx);
                                                                    });
                                                                }
                                                            },
                                                            cx,
                                                        ),
                                                    )
                                                }),
                                        )
                                        .when(!studio_raw.is_empty() && studio_raw != studio_output, |box_div| {
                                            box_div.child(
                                                div()
                                                    .text_size(rems(0.71875))
                                                    .text_color(ui::fg3(cx))
                                                    .child(format!("原文：{}", crate::views::components::ellipsize(&studio_raw, 140))),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_size(rems(0.84375))
                                                .line_height(rems(1.35))
                                                .text_color(ui::fg(cx))
                                                .child(if studio_busy {
                                                    "正在通过 Vercel AI Gateway 润色文本…".to_string()
                                                } else {
                                                    studio_output
                                                }),
                                        )
                                        .when(studio_fix_open && !studio_busy, |box_div| {
                                            let s = session_handle.clone();
                                            let v = view.clone();
                                            let input = studio_fix_input.clone();
                                            box_div.child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(8.0))
                                                    .p(px(10.0))
                                                    .rounded(px(8.0))
                                                    .bg(ui::card_bg(cx))
                                                    .border_1()
                                                    .border_color(ui::hairline(cx))
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.6875))
                                                            .text_color(ui::fg3(cx))
                                                            .child("修正润色结果，保存后只学习你改动里的术语。"),
                                                    )
                                                    .child(
                                                        Textarea::new(&input)
                                                            .h(px(100.0))
                                                            .w_full(),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .justify_end()
                                                            .gap(px(8.0))
                                                            .child(button(
                                                                "cancel-studio-fix",
                                                                "取消",
                                                                false,
                                                                {
                                                                    let v = v.clone();
                                                                    move |_, cx| {
                                                                        v.update(cx, |view, cx| {
                                                                            view.studio_fix_open = false;
                                                                            cx.notify();
                                                                        });
                                                                    }
                                                                },
                                                            ))
                                                            .child(button(
                                                                "save-studio-fix",
                                                                "保存修正并学习",
                                                                true,
                                                                move |_, cx| {
                                                                    let corrected =
                                                                        input.read(cx).value().to_string();
                                                                    if corrected.trim().is_empty() {
                                                                        return;
                                                                    }
                                                                    s.update(cx, |session, cx| {
                                                                        session.apply_studio_correction(
                                                                            &corrected,
                                                                            cx,
                                                                        );
                                                                    });
                                                                    v.update(cx, |view, cx| {
                                                                        view.studio_fix_open = false;
                                                                        cx.notify();
                                                                    });
                                                                },
                                                            )),
                                                    ),
                                            )
                                        }),
                                )
                            }),
                    ),
            )
            // 4. Recent Dictation History Preview
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(ui::section_label("最近口述", None).text_color(ui::fg2(cx)))
                            .when(!recent_history.is_empty(), |row| {
                                let v = view.clone();
                                row.child(
                                    div()
                                        .id("goto-all-history")
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .cursor_pointer()
                                        .text_size(rems(0.71875))
                                        .text_color(ui::accent(cx))
                                        .on_click(move |_, _, cx| {
                                            v.update(cx, |view, cx| {
                                                view.active_tab = crate::views::NavTab::History;
                                                cx.notify();
                                            });
                                        })
                                        .child("查看全部历史")
                                        .child(
                                            Icon::new(IconName::ChevronRight)
                                                .size(px(12.0))
                                                .text_color(ui::accent(cx)),
                                        ),
                                )
                            }),
                    )
                    .child(
                        ui::card(cx)
                            .when(recent_history.is_empty(), |card| {
                                card.child(empty_state(
                                    IconName::FileText,
                                    "还没有口述记录",
                                    "按快捷键说一句话，或者在上方演练台点击「立即润色」，结果会自动归档到这里。",
                                    cx,
                                ))
                            })
                            .children({
                                let count = recent_history.len();
                                let s = session_handle.clone();
                                let v = view.clone();
                                let copied = copied_token.clone();
                                let exp_ids = expanded_ids.clone();
                                let a_ids = audio_ids.clone();
                                recent_history.into_iter().enumerate().map(move |(idx, item)| {
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
                                        copied.clone(),
                                        correction_input.clone(),
                                        s.clone(),
                                        v.clone(),
                                        cx,
                                    )
                                })
                            }),
                    ),
            )
    }
}
