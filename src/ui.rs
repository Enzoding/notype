//! Clay tokens for the workspace.
//!
//! The floor stays cream in every appearance. Saturated feature cards carry
//! the color; semantic green, amber, and red stay separate from that palette.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px, rems,
    rgb,
};

/// Whether the window is currently in dark appearance.
#[allow(dead_code)]
pub fn is_dark(cx: &App) -> bool {
    cx.theme().is_dark()
}

fn ignore(cx: &App) {
    let _ = cx;
}

/// Headlines and primary text. `{colors.ink}` `#0a0a0a`.
pub fn fg(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x0a0a0a).into()
}

/// Sub-headings and secondary labels. `{colors.muted}` `#6a6a6a`.
pub fn fg2(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x6a6a6a).into()
}

/// Captions and fine print. `{colors.muted-soft}` `#9a9a9a`.
pub fn fg3(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x9a9a9a).into()
}

/// Running text. `{colors.body}` `#3a3a3a`.
#[allow(dead_code)]
pub fn body(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x3a3a3a).into()
}

/// Footer and sidebar. `{colors.surface-soft}` `#faf5e8`.
pub fn sidebar_bg(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xfaf5e8).into()
}

/// Page floor. `{colors.canvas}` `#fffaf0`.
pub fn canvas_bg(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xfffaf0).into()
}

/// 1px borders. `{colors.hairline}` `#e5e5e5`.
pub fn hairline(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xe5e5e5).into()
}

/// Emphasized cream edge. `{colors.surface-strong}` `#ebe6d6`.
pub fn border_strong(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xebe6d6).into()
}

/// Cream feature and testimonial cards. `{colors.surface-card}` `#f5f0e0`.
pub fn card_bg(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xf5f0e0).into()
}

/// Stronger cream wells. `{colors.surface-strong}` `#ebe6d6`.
pub fn well_bg(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xebe6d6).into()
}

/// Pressed or hovered cream. Same stronger cream, no extra shadow.
pub fn card_hover(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xebe6d6).into()
}

/// Primary CTAs. `{colors.primary}` `#0a0a0a`.
pub fn accent(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x0a0a0a).into()
}

/// Text on primary buttons and on pink or teal cards. `{colors.on-primary}` `#ffffff`.
pub fn on_accent(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xffffff).into()
}

pub fn on_primary(cx: &App) -> Hsla {
    on_accent(cx)
}

pub fn accent_soft(cx: &App) -> Hsla {
    well_bg(cx)
}

/// Hot pink feature fill. `{colors.brand-pink}` `#ff4d8b`.
pub fn brand_pink(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xff4d8b).into()
}

/// Deep teal feature fill. `{colors.brand-teal}` `#1a3a3a`.
pub fn brand_teal(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x1a3a3a).into()
}

/// Lavender feature fill. `{colors.brand-lavender}` `#b8a4ed`.
pub fn brand_lavender(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xb8a4ed).into()
}

/// Peach feature fill. `{colors.brand-peach}` `#ffb084`.
pub fn brand_peach(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xffb084).into()
}

/// Ochre feature fill. `{colors.brand-ochre}` `#e8b94a`.
pub fn brand_ochre(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xe8b94a).into()
}

/// Mint illustration accent. `{colors.brand-mint}` `#a4d4c5`.
#[allow(dead_code)]
pub fn brand_mint(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xa4d4c5).into()
}

/// Coral highlight. `{colors.brand-coral}` `#ff6b5a`.
#[allow(dead_code)]
pub fn brand_coral(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xff6b5a).into()
}

/// Listening indicator. Same pink as the listening feature card.
pub fn live(cx: &App) -> Hsla {
    brand_pink(cx)
}

#[allow(dead_code)]
pub fn live_soft(cx: &App) -> Hsla {
    brand_pink(cx).opacity(0.18)
}

/// Success. `{colors.success}` `#22c55e`.
pub fn ok(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0x22c55e).into()
}

pub fn ok_soft(cx: &App) -> Hsla {
    ok(cx).opacity(0.16)
}

/// Warning. `{colors.warning}` `#f59e0b`. Not the ochre feature card.
pub fn warn(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xf59e0b).into()
}

pub fn warn_soft(cx: &App) -> Hsla {
    warn(cx).opacity(0.18)
}

/// Validation and failure. `{colors.error}` `#ef4444`.
pub fn error(cx: &App) -> Hsla {
    ignore(cx);
    rgb(0xef4444).into()
}

/// The floating HUD stays cream in both appearances, so it reads as the same object over the desktop.
pub fn hud_bg() -> Hsla {
    rgb(0xfffaf0).into()
}

pub fn hud_live() -> Hsla {
    rgb(0xff4d8b).into()
}

pub fn hud_error() -> Hsla {
    rgb(0xef4444).into()
}

pub fn hud_ok() -> Hsla {
    rgb(0x22c55e).into()
}

pub fn hud_warn() -> Hsla {
    rgb(0xf59e0b).into()
}

pub fn hud_dim() -> Hsla {
    rgb(0x3a3a3a).into()
}

pub fn hud_fg() -> Hsla {
    rgb(0x0a0a0a).into()
}

pub fn hud_fg_dim() -> Hsla {
    rgb(0x6a6a6a).into()
}

/// Align stock controls with the cream canvas after the system theme loads.
pub fn sync_controls(cx: &mut App) {
    gpui_kit::component::Theme::update(cx, |theme| {
        theme.colors.background = rgb(0xfffaf0).into();
        theme.colors.foreground = rgb(0x0a0a0a).into();
        theme.colors.muted_foreground = rgb(0x6a6a6a).into();
        theme.colors.primary = rgb(0x0a0a0a).into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.primary_hover = rgb(0x1a1a1a).into();
        theme.colors.primary_active = rgb(0x0a0a0a).into();
        theme.colors.button_primary = rgb(0x0a0a0a).into();
        theme.colors.button_primary_foreground = rgb(0xffffff).into();
        theme.colors.button_primary_hover = rgb(0x1a1a1a).into();
        theme.colors.button_primary_active = rgb(0x0a0a0a).into();
        theme.colors.secondary = rgb(0xfffaf0).into();
        theme.colors.secondary_foreground = rgb(0x0a0a0a).into();
        theme.colors.secondary_hover = rgb(0xf5f0e0).into();
        theme.colors.secondary_active = rgb(0xebe6d6).into();
        theme.colors.button_secondary = rgb(0xfffaf0).into();
        theme.colors.button_secondary_foreground = rgb(0x0a0a0a).into();
        theme.colors.button_secondary_hover = rgb(0xf5f0e0).into();
        theme.colors.button_secondary_active = rgb(0xebe6d6).into();
        theme.colors.border = rgb(0xe5e5e5).into();
        theme.colors.input = rgb(0xe5e5e5).into();
        theme.colors.ring = rgb(0x0a0a0a).into();
        theme.colors.caret = rgb(0x0a0a0a).into();
        theme.colors.selection = rgb(0xb8a4ed).into();
        theme.colors.success = rgb(0x22c55e).into();
        theme.colors.warning = rgb(0xf59e0b).into();
        theme.colors.danger = rgb(0xef4444).into();
        theme.radius = px(12.);
        theme.shadow = false;
    });
}

/// The six feature-card fills. Pink and teal take white type; the rest take ink.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FeatureFill {
    Pink,
    Teal,
    Lavender,
    Peach,
    Ochre,
    Cream,
}

impl FeatureFill {
    pub fn bg(self, cx: &App) -> Hsla {
        match self {
            Self::Pink => brand_pink(cx),
            Self::Teal => brand_teal(cx),
            Self::Lavender => brand_lavender(cx),
            Self::Peach => brand_peach(cx),
            Self::Ochre => brand_ochre(cx),
            Self::Cream => card_bg(cx),
        }
    }

    pub fn on_dark(self) -> bool {
        matches!(self, Self::Pink | Self::Teal)
    }

    pub fn ink(self, cx: &App) -> Hsla {
        if self.on_dark() {
            on_primary(cx)
        } else {
            fg(cx)
        }
    }

    pub fn muted(self, cx: &App) -> Hsla {
        if self.on_dark() {
            on_primary(cx).opacity(0.82)
        } else {
            fg2(cx)
        }
    }
}

/// Saturated feature card: 24px radius, no shadow, no hairline.
pub fn feature_surface(fill: FeatureFill, cx: &App) -> Div {
    div()
        .rounded(px(24.0))
        .bg(fill.bg(cx))
        .text_color(fill.ink(cx))
        .overflow_hidden()
}

/// A section heading: quiet, structured, and spaced cleanly above its card.
pub fn section_label(text: impl Into<SharedString>, trailing: Option<String>) -> Div {
    let row = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(12.0))
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
        .rounded(px(16.0))
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
        .h(px(22.0))
        .px(px(10.0))
        .rounded(px(9999.0))
        .bg(bg_color)
        .text_size(px(13.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg_color)
        .child(label.into())
}
