use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::{Icon, IconName, Sizable, Size};
use gpui_kit::{
    App, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rems, rgb,
};

use crate::ui;

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
