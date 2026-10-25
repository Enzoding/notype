use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rems, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};

use crate::session::format_duration_ms;
use crate::ui;
use crate::views::components::{empty_state, history_card_row, metric_card};
use crate::views::SettingsView;

impl SettingsView {
    pub fn render_overview(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let copied_token = self.copied_token.clone();

        let (stats, phrases_len, recent_history, playing_id, audio_ids) = {
            let s = self.session.read(cx);
            (
                s.stats.clone(),
                s.phrases.len(),
                s.history.iter().take(5).cloned().collect::<Vec<_>>(),
                s.playing_id,
                s.history
                    .iter()
                    .take(5)
                    .filter(|item| s.audio_ids.contains(&item.id))
                    .map(|item| item.id)
                    .collect::<std::collections::HashSet<_>>(),
            )
        };

        let expanded_ids = self.expanded_raw_ids.clone();
        let correction_input = self.correction_input.clone();
        let editing_history_id = self.editing_history_id;
        let repolishing_id = self.session.read(cx).repolishing_id;

        let fills = [
            ui::FeatureFill::Teal,
            ui::FeatureFill::Peach,
            ui::FeatureFill::Lavender,
            ui::FeatureFill::Ochre,
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(28.0))
            .child(self.setup_status(cx))
            // 1. 4-Column Key Metrics Strip
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.0))
                    .child(metric_card(
                        crate::i18n::t("overview.metrics.total_sessions"),
                        format!("{}", stats.total_sessions),
                        if stats.total_duration_ms > 0 {
                            crate::i18n::t_args(
                                "overview.metrics.total_duration",
                                &[("duration", &format_duration_ms(stats.total_duration_ms))],
                            )
                        } else {
                            crate::i18n::t("overview.metrics.shortcut_hint").into()
                        },
                        LucideIcon::Mic,
                        fills[0],
                        cx,
                    ))
                    .child(metric_card(
                        crate::i18n::t("overview.metrics.total_chars"),
                        format!("{}", stats.total_chars),
                        if stats.saved_minutes >= 0.1 {
                            crate::i18n::t_args(
                                "overview.metrics.saved_typing_time",
                                &[("minutes", &format!("{:.1}", stats.saved_minutes))],
                            )
                        } else {
                            crate::i18n::t("overview.metrics.auto_cleanup_hint").into()
                        },
                        LucideIcon::Sparkles,
                        fills[1],
                        cx,
                    ))
                    .child(metric_card(
                        crate::i18n::t("overview.metrics.speech_rate"),
                        if stats.avg_cpm > 0 {
                            format!("{}", stats.avg_cpm)
                        } else {
                            "—".into()
                        },
                        crate::i18n::t("overview.metrics.cpm_unit").into(),
                        LucideIcon::Gauge,
                        fills[2],
                        cx,
                    ))
                    .child(metric_card(
                        crate::i18n::t("overview.metrics.vocabulary_terms"),
                        format!("{phrases_len}"),
                        crate::i18n::t("overview.metrics.spelling_lock_hint").into(),
                        LucideIcon::BookOpen,
                        fills[3],
                        cx,
                    )),
            )
            // 2. Recent Dictation History Preview
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                ui::section_label(
                                    crate::i18n::t("overview.recent_dictations"),
                                    None,
                                )
                                .text_color(ui::fg2(cx)),
                            )
                            .when(!recent_history.is_empty(), |row| {
                                let v = view.clone();
                                row.child(
                                    div()
                                        .id("goto-all-history")
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .cursor_pointer()
                                        .text_size(rems(0.71875))
                                        .text_color(ui::accent(cx))
                                        .on_click(move |_, _, cx| {
                                            v.update(cx, |view, cx| {
                                                view.active_tab = crate::views::NavTab::History;
                                                cx.notify();
                                            });
                                        })
                                        .child(crate::i18n::t("overview.view_all_history"))
                                        .child(
                                            Icon::new(IconName::ChevronRight)
                                                .size(px(12.0))
                                                .text_color(ui::accent(cx)),
                                        ),
                                )
                            }),
                    )
                    .child(
                        ui::card(cx)
                            .when(recent_history.is_empty(), |card| {
                                card.child(empty_state(
                                    IconName::FileText,
                                    crate::i18n::t("overview.empty_title"),
                                    crate::i18n::t("overview.empty_subtitle"),
                                    cx,
                                ))
                            })
                            .children({
                                let count = recent_history.len();
                                let s = session_handle.clone();
                                let v = view.clone();
                                let copied = copied_token.clone();
                                let exp_ids = expanded_ids.clone();
                                let a_ids = audio_ids.clone();
                                recent_history
                                    .into_iter()
                                    .enumerate()
                                    .map(move |(idx, item)| {
                                        let has_audio = a_ids.contains(&item.id);
                                        let is_playing = playing_id == Some(item.id);
                                        let is_expanded = exp_ids.contains(&item.id);
                                        let is_repolishing = repolishing_id == Some(item.id);
                                        let is_editing = editing_history_id == Some(item.id);
                                        history_card_row(
                                            item,
                                            idx + 1 == count,
                                            has_audio,
                                            is_playing,
                                            is_expanded,
                                            is_repolishing,
                                            is_editing,
                                            copied.clone(),
                                            correction_input.clone(),
                                            s.clone(),
                                            v.clone(),
                                            cx,
                                        )
                                    })
                            }),
                    ),
            )
    }
}
