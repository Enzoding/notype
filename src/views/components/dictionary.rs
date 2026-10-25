use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rems, App, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};

use crate::mining::TermKind;
use crate::session::Session;
use crate::store::PhraseCandidate;
use crate::ui;
use crate::views::components::primitives::{ellipsize, small_action_pill};

#[allow(dead_code)]
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
        "edit" => crate::i18n::t("dictionary.candidate_source_edit"),
        "preset" => crate::i18n::t("dictionary.candidate_source_preset"),
        "llm" => crate::i18n::t("dictionary.candidate_source_llm"),
        _ => crate::i18n::t("dictionary.candidate_source_candidate"),
    };
    let confidence = (candidate.confidence.clamp(0.0, 1.0) * 100.0).round() as i32;
    let quote = candidate
        .evidence
        .first()
        .map(|item| item.quote.clone())
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| crate::i18n::t("dictionary.candidate_default_quote").to_string());
    let quote = ellipsize(quote.trim(), 72);
    let hits_str = candidate.hits.to_string();
    let conf_str = confidence.to_string();
    let meta = crate::i18n::t_args(
        "dictionary.candidate_meta",
        &[
            ("source", source),
            ("hits", &hits_str),
            ("confidence", &conf_str),
        ],
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
                        .child(ui::pill(kind, ui::accent(cx), ui::accent_soft(cx)))
                        .child(div().text_size(rems(0.65625)).text_color(dim).child(meta)),
                )
                .child(div().text_size(rems(0.6875)).text_color(dim).child(quote)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(small_action_pill(
                    accept_id,
                    IconName::Check,
                    crate::i18n::t("dictionary.candidate_accept"),
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
                    crate::i18n::t("dictionary.candidate_ignore"),
                    move |_, cx| {
                        ignore_session.update(cx, |session, cx| {
                            session.ignore_candidate(id, cx);
                        });
                    },
                    cx,
                )),
        )
}

/// Theme colors shared by every phrase row, resolved once per list render.
#[derive(Clone, Copy)]
pub struct PhraseRowColors {
    pub hairline: Hsla,
    pub hover: Hsla,
    pub dim: Hsla,
}

pub fn phrase_row(
    id: i64,
    label: String,
    source: String,
    hits: i64,
    last: bool,
    session: Entity<Session>,
    colors: PhraseRowColors,
) -> impl IntoElement {
    let PhraseRowColors {
        hairline,
        hover,
        dim,
    } = colors;
    let accessible = crate::i18n::t_args("dictionary.delete_phrase", &[("label", &label)]);
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
                    let hits_str = hits.to_string();
                    row.child(div().text_size(rems(0.65625)).text_color(dim).child(
                        crate::i18n::t_args("dictionary.hits_count", &[("n", &hits_str)]),
                    ))
                })
                .child(div().text_size(rems(0.65625)).text_color(dim).child(
                    if source == "manual" {
                        crate::i18n::t("mining.source.manual")
                    } else {
                        crate::i18n::t("mining.auto_learn.auto")
                    },
                )),
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
