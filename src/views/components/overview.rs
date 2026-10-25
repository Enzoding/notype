use std::time::Duration;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable, Size};
use gpui_kit::{
    App, Entity, FontWeight, IntoElement, ParentElement, Styled, div, px,
};

use crate::hotkey;
use crate::session::{Notice, Phase, Session, mmss};
use crate::store::TriggerMode;
use crate::ui;
use crate::views::components::primitives::{button, button_on_color, ellipsize};
use crate::views::components::settings::inline_wave_view;
use crate::views::components::shell::notice_look;
use crate::views::{NavTab, SettingsView};

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
