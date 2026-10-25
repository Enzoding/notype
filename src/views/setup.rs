//! Workbench setup is deliberately limited to the channel key and required permissions.
use super::{components::button, settings::SettingsSection, NavTab, SettingsView};
use crate::providers::SetupReadiness;
use crate::ui;
use gpui_kit::{div, px, Context, FontWeight, IntoElement, ParentElement, Styled, Window};
#[derive(Clone, Copy, Debug)]
pub(super) enum SetupTarget {
    Key,
    Permissions,
}
impl SettingsView {
    fn open_configuration(
        &mut self,
        target: SetupTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_tab = NavTab::Settings;
        match target {
            SetupTarget::Key => {
                self.settings_section = SettingsSection::Channel;
                self.key_editing = true;
                self.key_input
                    .update(cx, |state, cx| state.focus(window, cx));
            }
            SetupTarget::Permissions => self.settings_section = SettingsSection::Input,
        }
        cx.notify();
    }
    pub fn setup_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session = self.session.read(cx);
        let kind = session.connections.kind;
        let setup = SetupReadiness {
            key_configured: !session.connections.provider_keys[kind.index()].is_empty(),
            microphone_granted: session.microphone_access.granted(),
            accessibility_granted: session.accessibility_trusted,
        };
        let mut missing = Vec::new();
        if !setup.microphone_granted {
            missing.push(crate::i18n::t_args(
                "setup.mic_status",
                &[("status", session.microphone_access.label())],
            ));
        }
        if !setup.accessibility_granted {
            missing.push(crate::i18n::t("setup.accessibility_pending").to_string());
        }
        let permissions = if missing.is_empty() {
            crate::i18n::t("setup.all_permissions_granted").to_string()
        } else {
            missing.join(" · ")
        };
        let mut card = ui::card(cx).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .p(px(16.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(16.0))
                                .child(crate::i18n::t("setup.current_status")),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(ui::fg2(cx))
                                .child(kind.label()),
                        ),
                )
                .child(ui::pill(
                    if setup.ready() {
                        "OK".to_string()
                    } else {
                        crate::i18n::t_args(
                            "setup.items_pending",
                            &[("n", &setup.missing_count().to_string())],
                        )
                    },
                    if setup.ready() {
                        ui::ok(cx)
                    } else {
                        ui::warn(cx)
                    },
                    if setup.ready() {
                        ui::ok_soft(cx)
                    } else {
                        ui::warn_soft(cx)
                    },
                )),
        );
        for (target, label, complete, detail) in [
            (
                SetupTarget::Key,
                crate::i18n::t("setup.api_key_label"),
                setup.key_configured,
                if setup.key_configured {
                    crate::i18n::t("setup.key_configured").into()
                } else {
                    crate::i18n::t("setup.key_missing").into()
                },
            ),
            (
                SetupTarget::Permissions,
                crate::i18n::t("setup.permissions_label"),
                setup.permissions_ready(),
                permissions,
            ),
        ] {
            let view = view.clone();
            card = card.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_t_1()
                    .border_color(ui::hairline(cx))
                    .child(div().w(px(110.0)).text_size(px(14.0)).child(label))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(120.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(if complete { ui::ok(cx) } else { ui::warn(cx) })
                                    .child(if complete {
                                        crate::i18n::t("setup.status_completed")
                                    } else {
                                        crate::i18n::t("setup.status_pending")
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(ui::fg2(cx))
                                    .child(detail),
                            ),
                    )
                    .child(button(
                        format!("setup-{target:?}"),
                        if complete {
                            crate::i18n::t("setup.action_manage")
                        } else if matches!(target, SetupTarget::Key) {
                            crate::i18n::t("setup.action_config_key")
                        } else {
                            crate::i18n::t("setup.action_authorize")
                        },
                        !complete,
                        move |window, cx| {
                            view.update(cx, |v, cx| v.open_configuration(target, window, cx));
                        },
                    )),
            );
        }
        card
    }
}
