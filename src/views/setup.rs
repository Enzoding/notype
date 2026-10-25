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
            missing.push(format!("麦克风{}", session.microphone_access.label()));
        }
        if !setup.accessibility_granted {
            missing.push("辅助功能待授权".to_string());
        }
        let permissions = if missing.is_empty() {
            "麦克风、辅助功能均已授权".to_string()
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
                                .child("当前状态"),
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
                        format!("{} 项待完成", setup.missing_count())
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
                "API Key",
                setup.key_configured,
                if setup.key_configured {
                    "当前渠道已配置".into()
                } else {
                    "填写当前渠道的 API Key".into()
                },
            ),
            (
                SetupTarget::Permissions,
                "所需权限",
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
                                    .child(if complete { "已完成" } else { "待完成" }),
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
                            "管理"
                        } else if matches!(target, SetupTarget::Key) {
                            "去配 Key"
                        } else {
                            "去授权"
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
