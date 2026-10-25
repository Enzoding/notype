use std::time::Duration;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, IconName, Sizable, Size};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, ElementId, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, hsla, px, rems,
    rgb,
};

use crate::hotkey;
use crate::paste;
use crate::polish::{self, PolishMode};
use crate::session::{Notice, Phase, Session, format_duration_ms, format_relative_time, mmss};
use crate::mining::TermKind;
use crate::store::{HistoryItem, PhraseCandidate, TriggerMode};
use crate::ui;
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
                        .child(
                            div()
                                .size(px(6.5))
                                .rounded_full()
                                .bg(if is_rec {
                                    ui::live(cx)
                                } else if has_key {
                                    ui::ok(cx)
                                } else {
                                    ui::warn(cx)
                                }),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .font_weight(FontWeight::MEDIUM)
                                .child(if is_rec {
                                    format!("录音中 {}", mmss(elapsed))
                                } else if has_key {
                                    "可以口述".into()
                                } else {
                                    "还没有 API Key".into()
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
            if is_rec { "完成并转写" } else { "立即开始口述" },
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

pub fn hero_fill(phase: &Phase) -> ui::FeatureFill {
    match phase {
        Phase::Listening => ui::FeatureFill::Pink,
        Phase::Working(_) => ui::FeatureFill::Teal,
        Phase::Message(Notice::Error(_)) | Phase::Message(Notice::Warn(_)) => ui::FeatureFill::Cream,
        Phase::Message(_) => ui::FeatureFill::Peach,
        Phase::Idle => ui::FeatureFill::Cream,
    }
}

/// Four metric fills that do not repeat the hero, and leave lavender for the studio card.
pub fn metric_fills(hero: ui::FeatureFill) -> [ui::FeatureFill; 4] {
    match hero {
        ui::FeatureFill::Pink => [
            ui::FeatureFill::Teal,
            ui::FeatureFill::Peach,
            ui::FeatureFill::Ochre,
            ui::FeatureFill::Cream,
        ],
        ui::FeatureFill::Teal => [
            ui::FeatureFill::Pink,
            ui::FeatureFill::Peach,
            ui::FeatureFill::Ochre,
            ui::FeatureFill::Cream,
        ],
        ui::FeatureFill::Peach => [
            ui::FeatureFill::Pink,
            ui::FeatureFill::Teal,
            ui::FeatureFill::Ochre,
            ui::FeatureFill::Cream,
        ],
        _ => [
            ui::FeatureFill::Pink,
            ui::FeatureFill::Teal,
            ui::FeatureFill::Peach,
            ui::FeatureFill::Ochre,
        ],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn hero_banner(
    phase: &Phase,
    elapsed: Duration,
    wave: [f32; crate::capture::WAVE_BARS],
    has_key: bool,
    hotkey_code: i64,
    trigger_mode: TriggerMode,
    strategy_label: &str,
    session: Entity<Session>,
    view: Entity<SettingsView>,
    cx: &App,
) -> impl IntoElement {
    let fill = hero_fill(phase);
    let ink = fill.ink(cx);
    let muted = fill.muted(cx);
    let card = ui::feature_surface(fill, cx)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(24.0))
        .p(px(24.0));

    match phase {
        Phase::Listening => {
            let stop_session = session.clone();
            let cancel_session = session;
            card.child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(44.0))
                                .rounded_full()
                                .bg(ui::on_primary(cx).opacity(0.22))
                                .child(
                                    div().size(px(10.0)).rounded_full().bg(ui::on_primary(cx)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex_col()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap(px(12.0))
                                        .child(
                                            div()
                                                .text_size(px(32.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(ink)
                                                .child(format!("正在聆听 · {}", mmss(elapsed))),
                                        )
                                        .child(inline_wave_view(wave, ui::on_primary(cx))),
                                )
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(muted)
                                        .child("说完后再次按快捷键或点击右侧「完成」即可转写并贴入目标窗口"),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .gap(px(12.0))
                        .child(button("hero-cancel", "取消", false, move |_, cx| {
                            cancel_session.update(cx, |s, cx| s.cancel_recording(cx));
                        }))
                        .child(button_on_color("hero-stop", "完成转写", cx, move |_, cx| {
                            stop_session.update(cx, |s, cx| s.toggle(cx));
                        })),
                )
        }
        Phase::Working(stage) => card.child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(px(16.0))
                .child(Spinner::new().with_size(Size::Medium))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(32.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(ink)
                                .child(stage.label()),
                        )
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(muted)
                                .child(format!(
                                    "处理管线第 {} / 3 步 · 完成后自动贴入最前面的窗口",
                                    stage.index() + 1
                                )),
                        ),
                ),
        ),
        Phase::Message(notice) => {
            let (icon, color) = notice_look(notice, cx);
            card.child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w(px(0.0))
                    .gap(px(16.0))
                    .child(Icon::new(icon).size(px(18.0)).text_color(color))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(32.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ink)
                                    .child(ellipsize(notice.text(), 56)),
                            )
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(muted)
                                    .child("内容已同步归档到口述历史"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .child(button("hero-again", "再次口述", false, move |_, cx| {
                session.update(cx, |s, cx| s.toggle(cx));
            })),
            )
        }
        Phase::Idle => card
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_start()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .h(px(32.0))
                            .px(px(10.0))
                            .rounded(px(8.0))
                            .bg(ui::well_bg(cx))
                            .text_size(px(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ink)
                            .child(hotkey::hotkey_badge(hotkey_code)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(32.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ink)
                                    .child("随时随地语音口述"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(ui::pill(
                                        strategy_label.to_string(),
                                        ui::fg(cx),
                                        ui::well_bg(cx),
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(muted)
                                            .child(format!(
                                                "当前触发方式：{} · {}",
                                                hotkey::hotkey_label(hotkey_code),
                                                trigger_mode.label()
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(div().flex_shrink_0().child(if has_key {
                button("hero-start", "开始口述", true, move |_, cx| {
                    session.update(cx, |s, cx| s.toggle(cx));
                })
            } else {
                button("hero-config-key", "配置 API Key", true, move |_, cx| {
                    view.update(cx, |v, cx| {
                        v.active_tab = NavTab::Settings;
                        cx.notify();
                    });
                })
            })),
    }
}

pub fn metric_card(
    title: &'static str,
    value: String,
    subtitle: String,
    icon: LucideIcon,
    fill: ui::FeatureFill,
    cx: &App,
) -> impl IntoElement {
    let ink = fill.ink(cx);
    let muted = fill.muted(cx);
    ui::feature_surface(fill, cx)
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(240.0))
        .gap(px(8.0))
        .rounded(px(16.0))
        .p(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .child(title),
                )
                .child(Icon::new(icon).size(px(16.0)).text_color(ink)),
        )
        .child(
            div()
                .text_size(px(24.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ink)
                .child(value),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::NORMAL)
                .text_color(muted)
                .child(subtitle),
        )
}

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
                        .child(ui::pill(
                            mode_label,
                            ui::accent(cx),
                            ui::accent_soft(cx),
                        ))
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
                                if is_expanded { "收起原文" } else { "对比原文" },
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
                                if is_repolishing { "润色中…" } else { "重润色" },
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
                                    let corrected =
                                        correction_input.read(cx).value().to_string();
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
                                if is_raw_copied { "已复制原文" } else { "复制原文" },
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

pub fn mode_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(px(32.0))
        .px(px(12.0))
        .rounded(px(9999.0))
        .whitespace_nowrap()
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
            if !active {
                st.bg(ui::card_hover(cx))
            } else {
                st
            }
        })
        .text_size(px(14.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if active {
            ui::on_accent(cx)
        } else {
            ui::fg(cx)
        })
        .on_click(move |_, window, cx| action(window, cx))
        .child(label.into())
}

pub fn small_action_pill(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &'static str,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(4.0))
        .h(px(22.0))
        .px(px(7.5))
        .rounded(px(6.0))
        .bg(ui::well_bg(cx))
        .cursor_pointer()
        .hover(|st| st.bg(ui::card_hover(cx)))
        .text_size(rems(0.6875))
        .text_color(ui::fg2(cx))
        .on_click(move |_, window, cx| action(window, cx))
        .child(Icon::new(icon).size(px(11.0)).text_color(ui::fg2(cx)))
        .child(label)
}

pub fn inline_wave_view(wave: [f32; crate::capture::WAVE_BARS], color: Hsla) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(20.0))
        .children((0..crate::capture::WAVE_BARS * 2).map(move |index| {
            let source = if index < crate::capture::WAVE_BARS {
                crate::capture::WAVE_BARS - 1 - index
            } else {
                index - crate::capture::WAVE_BARS
            };
            let level = wave[source].clamp(0.0, 1.0).powf(0.65);
            let height = 3.0 + level * 15.0;
            div()
                .w(px(2.5))
                .h(px(height))
                .rounded(px(1.25))
                .bg(color)
        }))
}

pub fn status_pill(phase: &Phase, elapsed: Duration, cx: &App) -> impl IntoElement {
    let (color, label) = match phase {
        Phase::Idle => (ui::ok(cx), "就绪".to_string()),
        Phase::Listening => (ui::live(cx), format!("正在听 {}", mmss(elapsed))),
        Phase::Working(stage) => (ui::brand_teal(cx), stage.label().to_string()),
        Phase::Message(Notice::Error(_)) => (ui::error(cx), "需要留意".to_string()),
        Phase::Message(Notice::Warn(_)) => (ui::warn(cx), "需要留意".to_string()),
        Phase::Message(_) => (ui::ok(cx), "已完成".to_string()),
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

pub fn notice_look(notice: &Notice, cx: &App) -> (IconName, Hsla) {
    match notice {
        Notice::Done(_) => (IconName::Check, ui::ok(cx)),
        Notice::Copied => (IconName::Copy, ui::fg2(cx)),
        Notice::Warn(_) => (IconName::TriangleAlert, ui::warn(cx)),
        Notice::Error(_) => (IconName::CircleAlert, ui::error(cx)),
    }
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

pub fn candidate_row(
    candidate: PhraseCandidate,
    last: bool,
    session: Entity<Session>,
    hairline: Hsla,
    hover: Hsla,
    dim: Hsla,
    cx: &App,
) -> impl IntoElement {
    let id = candidate.id;
    let kind = TermKind::from_str(&candidate.kind).label();
    let source = match candidate.source.as_str() {
        "edit" => "来自修正",
        "preset" => "预设命中",
        "llm" => "AI 提取",
        _ => "候选",
    };
    let confidence = (candidate.confidence.clamp(0.0, 1.0) * 100.0).round() as i32;
    let quote = candidate
        .evidence
        .first()
        .map(|item| item.quote.clone())
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| "来自最近一次口述".into());
    let quote = ellipsize(quote.trim(), 72);
    let meta = format!(
        "{} · 出现 {} 次 · {}% 置信",
        source, candidate.hits, confidence
    );
    let accept_id = format!("accept-candidate-{id}");
    let ignore_id = format!("ignore-candidate-{id}");
    let accept_session = session.clone();
    let ignore_session = session.clone();

    div()
        .id(SharedString::from(format!("candidate-{id}")))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(10.0))
        .when(!last, |row| row.border_b_1().border_color(hairline))
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .min_w(px(0.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(rems(0.8125))
                                .font_weight(FontWeight::MEDIUM)
                                .child(candidate.phrase.clone()),
                        )
                        .child(ui::pill(
                            kind,
                            ui::accent(cx),
                            ui::accent_soft(cx),
                        ))
                        .child(
                            div()
                                .text_size(rems(0.65625))
                                .text_color(dim)
                                .child(meta),
                        ),
                )
                .child(
                    div()
                        .text_size(rems(0.6875))
                        .text_color(dim)
                        .child(quote),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(small_action_pill(
                    accept_id,
                    IconName::Check,
                    "接受",
                    move |_, cx| {
                        accept_session.update(cx, |session, cx| {
                            session.accept_candidate(id, cx);
                        });
                    },
                    cx,
                ))
                .child(small_action_pill(
                    ignore_id,
                    IconName::Close,
                    "忽略",
                    move |_, cx| {
                        ignore_session.update(cx, |session, cx| {
                            session.ignore_candidate(id, cx);
                        });
                    },
                    cx,
                )),
        )
}

pub fn phrase_row(
    id: i64,
    label: String,
    source: String,
    hits: i64,
    last: bool,
    session: Entity<Session>,
    hairline: Hsla,
    hover: Hsla,
    dim: Hsla,
) -> impl IntoElement {
    let accessible = format!("删除 {label}");
    let group_id = SharedString::from(format!("phrase-row-{id}"));
    div()
        .id(SharedString::from(format!("phrase-{id}")))
        .group(group_id.clone())
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(9.0))
        .min_h(px(38.0))
        .when(!last, |row| row.border_b_1().border_color(hairline))
        .hover(move |style| style.bg(hover))
        .child(div().flex_1().text_size(rems(0.8125)).child(label))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .when(hits > 1, |row| {
                    row.child(
                        div()
                            .text_size(rems(0.65625))
                            .text_color(dim)
                            .child(format!("{hits} 次")),
                    )
                })
                .child(
                    div()
                        .text_size(rems(0.65625))
                        .text_color(dim)
                        .child(if source == "manual" { "手动" } else { "自动" }),
                ),
        )
        .child(
            div()
                .id(SharedString::from(format!("delete-{id}")))
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.0))
                .rounded(px(6.0))
                .opacity(0.35)
                .group_hover(group_id, |style| style.opacity(0.95))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .aria_label(accessible)
                .on_click(move |_, _, cx| {
                    session.update(cx, |session, cx| session.delete_phrase(id, cx));
                })
                .child(Icon::new(IconName::Close).size(px(12.0)).text_color(dim)),
        )
}

pub fn empty_state(
    icon: IconName,
    title: &'static str,
    body: &'static str,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(5.0))
        .py(px(24.0))
        .child(Icon::new(icon).size(px(18.0)).text_color(ui::fg3(cx)))
        .child(
            div()
                .text_size(rems(0.78125))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::fg2(cx))
                .child(title),
        )
        .child(
            div()
                .max_w(px(360.0))
                .text_size(rems(0.71875))
                .text_color(ui::fg3(cx))
                .child(body),
        )
}

pub fn icon_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: &'static str,
    action: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|style| style.bg(ui::card_hover(cx)))
        .aria_label(label)
        .on_click(move |_, window, cx| action(window, cx))
        .child(Icon::new(icon).size(px(13.0)).text_color(ui::fg2(cx)))
}

fn clay_button(button: Button) -> Button {
    button
        .with_size(Size::Medium)
        .rounded(px(12.0))
        .h(px(44.0))
        .px(px(20.0))
        .font_weight(FontWeight::SEMIBOLD)
}

pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Button {
    let button = Button::new(id).label(label);
    let hairline: Hsla = rgb(0xe5e5e5).into();
    let button = if primary {
        button.primary()
    } else {
        button.secondary().border_1().border_color(hairline)
    };
    clay_button(button).on_click(move |_, window, cx| action(window, cx))
}

/// White button used on pink and teal feature cards.
pub fn button_on_color(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Button {
    let variant = ButtonCustomVariant::new(cx)
        .color(rgb(0xffffff).into())
        .foreground(rgb(0x0a0a0a).into())
        .hover(rgb(0xf5f0e0).into())
        .active(rgb(0xebe6d6).into())
        .shadow(false);
    clay_button(Button::new(id).label(label).custom(variant))
        .on_click(move |_, window, cx| action(window, cx))
}

pub fn ellipsize(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}
