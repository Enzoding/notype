//! Semantic tokens and surface primitives for the interface.
//!
//! Designed around Apple's Human Interface Guidelines (materials, depth,
//! optical hierarchy, and semantic status colors) so both dark and light
//! appearances feel native to macOS.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, hsla, px,
    rems, rgb,
};

/// Whether the window is currently in dark appearance.
pub fn is_dark(cx: &App) -> bool {
    cx.theme().is_dark()
}

/// Primary ink: titles, body text, primary values.
pub fn fg(cx: &App) -> Hsla {
    cx.theme().foreground
}

/// Secondary ink: descriptions, row metadata, inactive nav labels.
pub fn fg2(cx: &App) -> Hsla {
    cx.theme().muted_foreground
}

/// Tertiary ink: captions, timestamps, paths, model identifiers.
pub fn fg3(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.42)
    } else {
        hsla(0.0, 0.0, 0.0, 0.44)
    }
}

/// Structural sidebar surface: slightly heavier material than the main canvas.
pub fn sidebar_bg(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0x141417).into()
    } else {
        rgb(0xf2f2f5).into()
    }
}

/// Main content canvas background.
pub fn canvas_bg(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0x1a1a1e).into()
    } else {
        rgb(0xfafafc).into()
    }
}

/// Subtle hairline border for cards, dividers, and table rows.
pub fn hairline(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.075)
    } else {
        hsla(0.0, 0.0, 0.0, 0.08)
    }
}

/// Stronger border for active or emphasized surfaces.
pub fn border_strong(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.14)
    } else {
        hsla(0.0, 0.0, 0.0, 0.15)
    }
}

/// Grouped card surface: one tone step above the canvas.
pub fn card_bg(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.038)
    } else {
        rgb(0xffffff).into()
    }
}

/// Elevated or inset sub-surface inside a card (e.g. code/raw blocks, wells).
pub fn well_bg(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 0.0, 0.26)
    } else {
        hsla(0.0, 0.0, 0.0, 0.035)
    }
}

/// Interactive row or card hover surface.
pub fn card_hover(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.065)
    } else {
        hsla(0.0, 0.0, 0.0, 0.042)
    }
}

/// Apple system blue accent.
pub fn accent(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0x0a84ff).into()
    } else {
        rgb(0x007aff).into()
    }
}

pub fn accent_soft(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(210.0 / 360.0, 1.0, 0.55, 0.16)
    } else {
        hsla(211.0 / 360.0, 1.0, 0.50, 0.11)
    }
}

/// Recording / live / destructive red.
pub fn live(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0xff453a).into()
    } else {
        rgb(0xd70015).into()
    }
}

pub fn live_soft(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(3.0 / 360.0, 1.0, 0.61, 0.16)
    } else {
        hsla(354.0 / 360.0, 1.0, 0.42, 0.10)
    }
}

/// Ready / verified green.
pub fn ok(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0x30d158).into()
    } else {
        rgb(0x248a3d).into()
    }
}

pub fn ok_soft(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(135.0 / 360.0, 0.63, 0.50, 0.15)
    } else {
        hsla(135.0 / 360.0, 0.59, 0.34, 0.10)
    }
}

/// Working / warning amber.
pub fn warn(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0xff9f0a).into()
    } else {
        rgb(0xb25000).into()
    }
}

pub fn warn_soft(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(36.0 / 360.0, 1.0, 0.52, 0.16)
    } else {
        hsla(27.0 / 360.0, 1.0, 0.35, 0.10)
    }
}

// The floating HUD always uses a dark material over the desktop wallpaper.
pub fn hud_live() -> Hsla {
    rgb(0xff453a).into()
}

pub fn hud_ok() -> Hsla {
    rgb(0x30d158).into()
}

pub fn hud_warn() -> Hsla {
    rgb(0xff9f0a).into()
}

pub fn hud_accent() -> Hsla {
    rgb(0x0a84ff).into()
}

pub fn hud_dim() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.72)
}

pub fn hud_fg() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.95)
}

pub fn hud_fg_dim() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.48)
}

/// A section heading: quiet, structured, and spaced cleanly above its card.
pub fn section_label(text: impl Into<SharedString>, trailing: Option<String>) -> Div {
    let row = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(rems(0.75))
                .font_weight(FontWeight::SEMIBOLD)
                .child(text.into()),
        );
    match trailing {
        Some(text) => row.child(
            div()
                .text_size(rems(0.71875))
                .font_weight(FontWeight::NORMAL)
                .child(text),
        ),
        None => row,
    }
}

/// A grouped surface: one card per functional group, hairline border, clean clipping.
pub fn card(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(12.0))
        .bg(card_bg(cx))
        .border_1()
        .border_color(hairline(cx))
        .overflow_hidden()
}

/// A tactile macOS keyboard shortcut cap (`右 ⌥`, `⎋`, `⌘ V`).
pub fn kbd_cap(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(22.0))
        .px(px(7.0))
        .rounded(px(5.0))
        .bg(well_bg(cx))
        .border_1()
        .border_color(border_strong(cx))
        .text_size(rems(0.71875))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg(cx))
        .child(label.into())
}

/// A small semantic status or category pill.
pub fn pill(label: impl Into<SharedString>, fg_color: Hsla, bg_color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(20.0))
        .px(px(7.5))
        .rounded(px(6.0))
        .bg(bg_color)
        .text_size(rems(0.6875))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg_color)
        .child(label.into())
}
