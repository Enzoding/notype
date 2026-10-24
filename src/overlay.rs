use std::time::Duration;

use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, IconName, Sizable, Size};
use gpui_kit::{
    Animation, AnimationExt, App, BoxShadow, Context, Entity, FontWeight, Hsla, IntoElement,
    ParentElement, Render, Styled, Window, div, hsla, linear_color_stop, linear_gradient, px,
    rems, rgb,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSWindow, NSWindowButton, NSWindowTitleVisibility};

use crate::hotkey;
use crate::polish;
use crate::session::{Notice, Phase, Session, Stage, mmss};
use crate::ui;
use crate::views::components::ellipsize;

const VOICE_TITLE: &str = "OpenTypeless Voice";
const SETTINGS_TITLE: &str = "OpenTypeless";

/// Keeps the standard traffic lights but drops the duplicated title text, so
/// the window's own header can carry the name.
pub fn prepare_settings_window() {
    with_window(SETTINGS_TITLE, |window| {
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    });
}

pub fn prepare_voice_window() {
    with_voice_window(|window| {
        quiet_panel(window);
        window.setAlphaValue(0.0);
    });
}

pub fn set_voice_visible(visible: bool) {
    with_voice_window(|window| {
        quiet_panel(window);
        window.setAlphaValue(if visible { 1.0 } else { 0.0 });
        if visible {
            window.orderFrontRegardless();
        }
    });
}

fn quiet_panel(window: &NSWindow) {
    window.setHidesOnDeactivate(false);
    window.setIgnoresMouseEvents(true);
    window.setTitlebarAppearsTransparent(true);
    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
}

fn with_voice_window(apply: impl Fn(&NSWindow)) {
    with_window(VOICE_TITLE, apply);
}

fn with_window(title: &str, apply: impl Fn(&NSWindow)) {
    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(marker);
    for window in app.windows() {
        if window.title().to_string() == title {
            apply(&window);
        }
    }
}

pub struct OverlayView {
    session: Entity<Session>,
}

impl OverlayView {
    pub fn new(session: Entity<Session>, cx: &mut Context<Self>) -> Self {
        cx.observe(&session, |_, _, cx| cx.notify()).detach();
        Self { session }
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (phase, wave, elapsed, active_bundle, polish_strategy, hotkey_code, appearance, exiting) = {
            let session = self.session.read(cx);
            (
                session.phase.clone(),
                session.wave,
                session.elapsed(),
                session.active_bundle.clone(),
                session.polish_strategy.clone(),
                session.hotkey_code,
                session.appearance,
                session.exiting,
            )
        };

        let app_name = polish::friendly_app_name(&active_bundle);
        let mode_label = polish::resolve_mode(&polish_strategy, &active_bundle)
            .map(|m| m.short_label())
            .unwrap_or("原文");
        let context_tag = format!("{app_name} · {mode_label}");
        let hk_badge = hotkey::hotkey_badge(hotkey_code);

        let content = match &phase {
            Phase::Idle => div(),
            Phase::Listening => listening_view(wave, elapsed, &context_tag, hk_badge),
            Phase::Working(stage) => working_view(*stage, &context_tag),
            Phase::Message(notice) => message_view(notice, cx),
        };

        div()
            .flex()
            .flex_col()
            .justify_end()
            .items_center()
            .size_full()
            .pb(px(10.0))
            .child(content.with_animation(
                ("hud", appearance),
                Animation::new(if exiting {
                    Duration::from_millis(150)
                } else {
                    Duration::from_millis(180)
                })
                .with_easing(move |t| {
                    let eased = gpui_kit::base::animation::ease_out_cubic(t);
                    if exiting {
                        1.0 - eased
                    } else {
                        eased
                    }
                }),
                |element, delta| element.opacity(delta),
            ))
    }
}

fn hud_shell() -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .h(px(54.0))
        .px(px(16.0))
        .rounded(px(18.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgb(0x1d1d22), 0.0),
            linear_color_stop(rgb(0x111114), 1.0),
        ))
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.09))
        .shadow(vec![
            BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 1.0, 0.12)).inset(),
            BoxShadow::new(px(0.0), px(14.0), hsla(0.0, 0.0, 0.0, 0.58))
                .blur_radius(px(30.0))
                .spread_radius(px(-6.0)),
        ])
        .text_size(rems(0.8125))
        .text_color(ui::hud_fg())
}

fn listening_view(
    wave: [f32; crate::capture::WAVE_BARS],
    elapsed: Duration,
    context_tag: &str,
    hk_badge: &str,
) -> gpui_kit::Div {
    hud_shell()
        .gap(px(12.0))
        .child(div().size(px(8.0)).rounded_full().bg(ui::hud_live()))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(mmss(elapsed)),
        )
        .child(wave_view(wave))
        .child(
            div()
                .px(px(7.0))
                .py(px(2.5))
                .rounded(px(6.0))
                .bg(hsla(0.0, 0.0, 1.0, 0.08))
                .text_size(rems(0.6875))
                .text_color(ui::hud_dim())
                .child(context_tag.to_string()),
        )
        .child(
            div()
                .text_size(rems(0.6875))
                .text_color(ui::hud_fg_dim())
                .child(hk_badge.to_string()),
        )
}

fn wave_view(wave: [f32; crate::capture::WAVE_BARS]) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(28.0))
        .children((0..crate::capture::WAVE_BARS * 2).map(move |index| {
            let source = if index < crate::capture::WAVE_BARS {
                crate::capture::WAVE_BARS - 1 - index
            } else {
                index - crate::capture::WAVE_BARS
            };
            let level = wave[source].clamp(0.0, 1.0).powf(0.65);
            let height = 4.0 + level * 22.0;
            let center = crate::capture::WAVE_BARS as f32 - 0.5;
            let distance = (index as f32 - center).abs() / crate::capture::WAVE_BARS as f32;
            let alpha = 0.38 + 0.62 * (1.0 - distance).clamp(0.0, 1.0);
            div()
                .w(px(2.5))
                .h(px(height))
                .rounded(px(1.25))
                .bg(hsla(0.0, 0.0, 1.0, alpha))
        }))
}

fn working_view(stage: Stage, context_tag: &str) -> gpui_kit::Div {
    hud_shell()
        .flex_col()
        .items_stretch()
        .justify_center()
        .h(px(64.0))
        .min_w(px(260.0))
        .gap(px(9.0))
        .px(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(Spinner::new().with_size(Size::Small))
                .child(div().font_weight(FontWeight::MEDIUM).child(stage.label()))
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(rems(0.6875))
                        .text_color(ui::hud_fg_dim())
                        .child(format!("{context_tag} · {}/3", stage.index() + 1)),
                ),
        )
        .child(segments(stage))
}

fn segments(stage: Stage) -> gpui_kit::Div {
    let active = stage.index();
    div().flex().gap(px(3.0)).children((0..3).map(move |index| {
        let color = if index < active {
            ui::hud_accent()
        } else if index == active {
            hsla(0.0, 0.0, 1.0, 0.72)
        } else {
            hsla(0.0, 0.0, 1.0, 0.16)
        };
        div().flex_1().h(px(2.5)).rounded(px(1.25)).bg(color)
    }))
}

fn hud_notice_look(notice: &Notice) -> (IconName, Hsla) {
    match notice {
        Notice::Done(_) => (IconName::Check, ui::hud_ok()),
        Notice::Copied => (IconName::Copy, ui::hud_dim()),
        Notice::Warn(_) => (IconName::TriangleAlert, ui::hud_warn()),
        Notice::Error(_) => (IconName::CircleAlert, ui::hud_live()),
    }
}

fn message_view(notice: &Notice, _cx: &App) -> gpui_kit::Div {
    let (icon, color) = hud_notice_look(notice);
    let shell = hud_shell()
        .gap(px(9.0))
        .child(Icon::new(icon).size(px(15.0)).text_color(color));
    match notice {
        Notice::Error(_) => shell.child(
            div()
                .max_w(px(340.0))
                .text_size(rems(0.8125))
                .child(ellipsize(notice.text(), 48)),
        ),
        _ => shell.child(div().child(notice.text().to_string())),
    }
}
