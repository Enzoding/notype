use std::time::Duration;

use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, IconName, Sizable, Size};
use gpui_kit::{
    div, hsla, px, rgb, Animation, AnimationExt, Context, Entity, FontWeight, Hsla, IntoElement,
    ParentElement, Render, Styled, Window,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSColor, NSEvent, NSFloatingWindowLevel, NSScreen, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{NSPoint, NSString};

use crate::session::{mmss, Notice, Phase, Session};
use crate::views::components::ellipsize;

const VOICE_TITLE: &str = "Notype Voice";
const SETTINGS_TITLE: &str = "Notype";

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
            place_on_active_display(window);
            window.orderFrontRegardless();
        }
    });
}

fn quiet_panel(window: &NSWindow) {
    window.setHidesOnDeactivate(false);
    window.setIgnoresMouseEvents(true);
    window.setLevel(NSFloatingWindowLevel);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    // A titled panel keeps a rectangular window border around the pill.
    // Borderless drops that chrome; the capsule is the only painted shape.
    let borderless = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    if window.styleMask() != borderless {
        let children = window
            .contentView()
            .map(|view| view.subviews().to_vec())
            .unwrap_or_default();
        window.setStyleMask(borderless);
        window.setTitle(&NSString::from_str(VOICE_TITLE));
        if let Some(content) = window.contentView() {
            reattach_views(&content, &children);
        }
    }
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);
}

fn reattach_views(content: &NSView, children: &[objc2::rc::Retained<NSView>]) {
    for child in children {
        content.addSubview(child);
    }
}

/// Moves the pill when the pointer has crossed onto another display.
/// Called throughout a session, not only when the capsule first appears.
pub fn track_voice_display() {
    with_voice_window(place_on_active_display);
}

/// Puts the pill at the bottom center of whichever display the pointer is on.
fn place_on_active_display(window: &NSWindow) {
    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let mouse = NSEvent::mouseLocation();
    let screens = NSScreen::screens(marker);
    let screen = screens
        .iter()
        .find(|screen| contains_point(screen.frame(), mouse))
        .or_else(|| NSScreen::mainScreen(marker));
    let Some(screen) = screen else {
        return;
    };
    let visible = screen.visibleFrame();
    let frame = window.frame();
    let x = visible.origin.x + (visible.size.width - frame.size.width) / 2.0;
    let y = visible.origin.y + 16.0;
    let origin = window.frame().origin;
    if (origin.x - x).abs() < 1.0 && (origin.y - y).abs() < 1.0 {
        return;
    }
    window.setFrameOrigin(NSPoint::new(x, y));
}

fn contains_point(frame: objc2_foundation::NSRect, point: NSPoint) -> bool {
    point.x >= frame.origin.x
        && point.x < frame.origin.x + frame.size.width
        && point.y >= frame.origin.y
        && point.y < frame.origin.y + frame.size.height
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
        let (phase, wave, elapsed, progress, appearance, exiting) = {
            let session = self.session.read(cx);
            (
                session.phase.clone(),
                session.wave,
                session.elapsed(),
                session.working_progress,
                session.appearance,
                session.exiting,
            )
        };

        let content = match &phase {
            Phase::Idle => div(),
            Phase::Listening => listening_view(wave, elapsed),
            Phase::Working(_) => working_view(progress),
            Phase::Message(notice) => message_view(notice),
        };

        div()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                content.with_animation(
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
                ),
            )
    }
}

fn hud_shell() -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .h(px(44.0))
        .px(px(16.0))
        .gap(px(10.0))
        .rounded_full()
        .bg(rgb(0x112822))
        .border_1()
        .border_color(hsla(150.0, 0.40, 0.45, 0.35))
        .text_size(px(14.0))
        .text_color(rgb(0xf2fcf6))
}

fn listening_view(wave: [f32; crate::capture::WAVE_BARS], elapsed: Duration) -> gpui_kit::Div {
    let near_limit = elapsed.as_secs() >= crate::tasks::MAX_RECORDING_SECS - 60;
    let timer = if near_limit {
        format!(
            "剩余 {}",
            mmss(Duration::from_secs(
                crate::tasks::MAX_RECORDING_SECS.saturating_sub(elapsed.as_secs())
            ))
        )
    } else {
        mmss(elapsed)
    };
    hud_shell()
        .child(div().size(px(8.0)).rounded_full().bg(rgb(0x34d399)))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(if near_limit {
                    rgb(0xfbbf24)
                } else {
                    rgb(0xf2fcf6)
                })
                .child(timer),
        )
        .child(wave_view(wave))
        .child(esc_hint())
}

fn wave_view(wave: [f32; crate::capture::WAVE_BARS]) -> gpui_kit::Div {
    const TOTAL_BARS: usize = 12;
    const HALF_BARS: usize = TOTAL_BARS / 2;

    div()
        .flex()
        .items_center()
        .gap(px(2.5))
        .h(px(30.0))
        .children((0..TOTAL_BARS).map(move |index| {
            let dist = if index < HALF_BARS {
                HALF_BARS - 1 - index
            } else {
                index - HALF_BARS
            };
            let norm_dist = dist as f32 / (HALF_BARS as f32 - 0.5);
            let source =
                (dist * crate::capture::WAVE_BARS / HALF_BARS).min(crate::capture::WAVE_BARS - 1);
            let raw_level = wave[source];

            // Substantially boosted sensitivity so voice syllables pop vigorously
            let boosted = (raw_level * 3.8).min(1.0);
            let level = boosted.powf(0.65);

            // Center bars bounce higher with maximum dynamic swing; outer bars flank smoothly
            let shape = 0.40 + 0.60 * (1.0 - norm_dist);
            let bar_level = level * shape;

            // Height expands from 3.0px dot up to 26.0px full jump (23px swing)
            let height = 3.0 + bar_level * 23.0;
            let alpha = 0.45 + 0.55 * (1.0 - norm_dist * 0.6);

            div()
                .w(px(2.8))
                .h(px(height))
                .rounded(px(1.4))
                .bg(rgb(0x34d399).opacity(alpha))
        }))
}

fn working_view(progress: f32) -> gpui_kit::Div {
    let total_width = 168.0;
    let clamped = progress.clamp(0.04, 1.0);
    let fill_w = (total_width * clamped).round();

    hud_shell()
        .w(px(total_width))
        .overflow_hidden()
        .relative()
        // Silky smooth background progress fill layer
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(fill_w))
                .rounded_full()
                .bg(rgb(0x194d3e))
                .border_r_1()
                .border_color(rgb(0x34d399).opacity(0.85)),
        )
        // Foreground content with spinner and "处理中"
        .child(
            div()
                .relative()
                .flex()
                .size_full()
                .items_center()
                .justify_between()
                .px(px(4.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(Spinner::new().with_size(Size::Small))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xf2fcf6))
                                .child("处理中"),
                        ),
                )
                .child(esc_hint()),
        )
}

fn esc_hint() -> gpui_kit::Div {
    div()
        .text_size(px(11.0))
        .text_color(hsla(150.0, 0.25, 0.75, 0.6))
        .child("esc")
}

fn hud_notice_look(notice: &Notice) -> (IconName, Hsla) {
    match notice {
        Notice::Done(_) => (IconName::Check, rgb(0x34d399).into()),
        Notice::Copied => (IconName::Copy, hsla(150.0, 0.2, 0.8, 0.85)),
        Notice::Warn(_) => (IconName::TriangleAlert, rgb(0xfbbf24).into()),
        Notice::Error(_) => (IconName::CircleAlert, rgb(0xf87171).into()),
    }
}

fn message_view(notice: &Notice) -> gpui_kit::Div {
    let (icon, color) = hud_notice_look(notice);
    hud_shell()
        .child(Icon::new(icon).size(px(16.0)).text_color(color))
        .child(
            div()
                .max_w(px(320.0))
                .text_color(rgb(0xf2fcf6))
                .child(ellipsize(notice.text(), 36)),
        )
}
