use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::Icon;
use gpui_kit::{
    App, FontWeight, IntoElement, ParentElement, Styled, div, px,
};

use crate::ui;

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
