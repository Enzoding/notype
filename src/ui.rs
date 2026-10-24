//! Semantic tokens for the interface.
//!
//! Colours go through roles here instead of literal values at each call site,
//! so the overlay and the settings window stay in step and the light theme
//! only has to be handled in one place.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Div, FontWeight, Hsla, ParentElement, SharedString, Styled, div, hsla, px, rems, rgb,
};

/// The window follows the system appearance; the HUD always uses the dark
/// material because it sits on top of an unknown wallpaper.
pub fn is_dark(cx: &App) -> bool {
    cx.theme().is_dark()
}

pub fn fg(cx: &App) -> Hsla {
    cx.theme().foreground
}

/// Secondary text: descriptions, row values, status labels.
pub fn fg2(cx: &App) -> Hsla {
    cx.theme().muted_foreground
}

/// Tertiary text: section labels, model identifiers, paths.
pub fn fg3(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.38)
    } else {
        hsla(0.0, 0.0, 0.0, 0.40)
    }
}

pub fn hairline(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.08)
    } else {
        hsla(0.0, 0.0, 0.0, 0.09)
    }
}

pub fn card_bg(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.032)
    } else {
        hsla(0.0, 0.0, 0.0, 0.018)
    }
}

pub fn card_hover(cx: &App) -> Hsla {
    if is_dark(cx) {
        hsla(0.0, 0.0, 1.0, 0.055)
    } else {
        hsla(0.0, 0.0, 0.0, 0.04)
    }
}

pub fn live(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0xff453a).into()
    } else {
        rgb(0xd70015).into()
    }
}

pub fn ok(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0x32d74b).into()
    } else {
        rgb(0x248a3d).into()
    }
}

pub fn warn(cx: &App) -> Hsla {
    if is_dark(cx) {
        rgb(0xff9f0a).into()
    } else {
        rgb(0xb25000).into()
    }
}

// The HUD always uses the dark material, so its state colours do not follow
// the window appearance.
pub fn hud_live() -> Hsla {
    rgb(0xff453a).into()
}

pub fn hud_ok() -> Hsla {
    rgb(0x32d74b).into()
}

pub fn hud_warn() -> Hsla {
    rgb(0xff9f0a).into()
}

pub fn hud_dim() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.72)
}

/// The HUD is always the same dark material, in either appearance.
pub fn hud_fg() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.94)
}

pub fn hud_fg_dim() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.45)
}

/// A section heading: small, quiet, and spaced away from its group.
pub fn section_label(text: impl Into<SharedString>, trailing: Option<String>) -> Div {
    let row = div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .text_size(rems(0.6875))
                .font_weight(FontWeight::MEDIUM)
                .child(text.into()),
        );
    match trailing {
        Some(text) => row.child(div().text_size(rems(0.6875)).child(text)),
        None => row,
    }
}

/// A grouped surface: one card per section, hairline border, no shadow.
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
