use gpui_kit::component::input::Input;
use gpui_kit::component::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rems,
};

use crate::paste;
use crate::ui;
use crate::views::SettingsView;
use crate::views::components::{button, empty_state, phrase_row};

pub const PRESET_PHRASES: &[&str] = &[
    "OpenTypeless",
    "GPUI",
    "Rust",
    "TypeScript",
    "Next.js",
    "Vercel",
    "DeepSeek",
    "Claude",
    "Cursor",
    "Xcode",
    "macOS",
    "GitHub",
    "API Key",
    "Prompt",
];

impl SettingsView {
    pub fn render_dictionary(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let phrase_input = self.phrase_input.clone();
        let copied_token = self.copied_token.clone();
        let phrases = self.session.read(cx).phrases.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            // 1. Add Phrase & Batch Toolbar Card
            .child(
                ui::card(cx)
                    .p(px(16.0))
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(rems(0.84375))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("添加专属词条"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.71875))
                                            .text_color(ui::fg2(cx))
                                            .child("支持一次输入多个词条（以逗号或换行分隔），转写声学模型与润色大模型会同时锁定拼写。"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child({
                                        let s = session_handle.clone();
                                        let v = view.clone();
                                        button(
                                            "import-phrases-clipboard",
                                            "从剪贴板导入",
                                            false,
                                            move |_, cx| {
                                                if let Some(clip) = paste::read_from_clipboard() {
                                                    s.update(cx, |session, cx| {
                                                        session.add_phrase(&clip, cx);
                                                    });
                                                    v.update(cx, |view, cx| {
                                                        view.flash_copied("dict-imported", cx);
                                                    });
                                                }
                                            },
                                        )
                                    })
                                    .when(!phrases.is_empty(), |row| {
                                        let all_text = phrases
                                            .iter()
                                            .map(|p| p.phrase.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        let v = view.clone();
                                        let is_copied =
                                            copied_token.as_deref() == Some("dict-exported");
                                        row.child(button(
                                            "export-phrases-clipboard",
                                            if is_copied { "已复制全部词条" } else { "导出全部" },
                                            false,
                                            move |_, cx| {
                                                if paste::copy_to_clipboard(&all_text) {
                                                    v.update(cx, |view, cx| {
                                                        view.flash_copied("dict-exported", cx);
                                                    });
                                                }
                                            },
                                        ))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(Input::new(&phrase_input).w_full()),
                            )
                            .child({
                                let input = phrase_input.clone();
                                let target = session_handle.clone();
                                button("add-phrase-btn", "添加词条", true, move |window, cx| {
                                    let value = input.read(cx).value().to_string();
                                    target.update(cx, |session, cx| session.add_phrase(&value, cx));
                                    input.update(cx, |inp, cx| inp.set_value("", window, cx));
                                })
                            }),
                    )
                    // Preset quick-add chips
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(rems(0.6875))
                                    .text_color(ui::fg3(cx))
                                    .child("常用推荐："),
                            )
                            .children(PRESET_PHRASES.iter().enumerate().map(|(idx, word)| {
                                let word_str = *word;
                                let exists = phrases.iter().any(|p| p.phrase.eq_ignore_ascii_case(word_str));
                                let s = session_handle.clone();
                                div()
                                    .id(SharedString::from(format!("preset-phrase-{idx}")))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .px(px(8.0))
                                    .py(px(3.0))
                                    .rounded(px(6.0))
                                    .bg(if exists {
                                        ui::accent_soft(cx)
                                    } else {
                                        ui::well_bg(cx)
                                    })
                                    .text_size(rems(0.6875))
                                    .text_color(if exists {
                                        ui::accent(cx)
                                    } else {
                                        ui::fg2(cx)
                                    })
                                    .when(!exists, |chip| {
                                        chip.cursor_pointer()
                                            .hover(|st| st.bg(ui::card_hover(cx)))
                                            .on_click(move |_, _, cx| {
                                                s.update(cx, |session, cx| {
                                                    session.add_phrase(word_str, cx);
                                                });
                                            })
                                    })
                                    .child(if exists {
                                        format!("✓ {word_str}")
                                    } else {
                                        format!("+ {word_str}")
                                    })
                            })),
                    ),
            )
            // 2. Active Glossary List
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        ui::section_label(
                            "已启用词条",
                            Some(format!("共 {} 项", phrases.len())),
                        )
                        .text_color(ui::fg2(cx))
                        .justify_between(),
                    )
                    .child(
                        ui::card(cx)
                            .when(phrases.is_empty(), |card| {
                                card.child(empty_state(
                                    IconName::BookOpen,
                                    "词库还是空的",
                                    "点击上方推荐词条或手动输入产品名、人名、技术术语，防止同音字误识别。",
                                    cx,
                                ))
                            })
                            .children({
                                let count = phrases.len();
                                let session = session_handle.clone();
                                let hairline = ui::hairline(cx);
                                let hover = ui::card_hover(cx);
                                let dim = ui::fg2(cx);
                                phrases.into_iter().enumerate().map(move |(index, phrase)| {
                                    phrase_row(
                                        phrase.id,
                                        phrase.phrase,
                                        index + 1 == count,
                                        session.clone(),
                                        hairline,
                                        hover,
                                        dim,
                                    )
                                })
                            }),
                    ),
            )
    }
}
