use super::{
    components::{button, choice_row, connection_status_row, key_row, mode_chip, setting_row},
    SettingsView,
};
use crate::providers::ProviderKind;
use crate::ui;
use gpui_kit::component::input::Input;
use gpui_kit::{div, px, Context, IntoElement, ParentElement, Styled};
impl SettingsView {
    pub fn provider_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.clone();
        let view = cx.entity();
        let s = session.read(cx);
        let editor = s.connections.kind;
        let has_key = !s.connections.key.is_empty();
        let key = s.connections.key.clone();
        let key_status = s.connections.key_status.clone();
        let editing = self.key_editing || !has_key;
        let testing = s.connections.gateway_testing;
        let status = s.connections.checks[editor.index()].polishing.clone();
        let transcription_status = s.connections.checks[editor.index()].transcription.clone();
        let model_status = s.connections.model_status.clone();
        let tm = s.connections.transcribe_model.clone();
        let pm = s.connections.polish_model.clone();
        let trans_models = if self.models_expanded {
            s.catalog(true)
        } else {
            Vec::new()
        };
        let chat_models = if self.models_expanded {
            s.catalog(false)
        } else {
            Vec::new()
        };
        let mut card = ui::card(cx).child(
            setting_row("当前渠道", "全部功能使用当前渠道。", true, cx).child(
                choice_row().children(ProviderKind::ALL.map(|kind| {
                    let session = session.clone();
                    let view = view.clone();
                    button(
                        format!("edit-{}", kind.id()),
                        kind.label(),
                        editor == kind,
                        move |window, cx| {
                            session.update(cx, |s, cx| s.select_provider(kind, cx));
                            let s = session.read(cx);
                            let tm = s.connections.transcribe_model.clone();
                            let pm = s.connections.polish_model.clone();
                            view.update(cx, |v, cx| {
                                v.key_editing = false;
                                v.key_input.update(cx, |i, cx| i.set_value("", window, cx));
                                v.transcribe_model_input
                                    .update(cx, |i, cx| i.set_value(tm, window, cx));
                                v.polish_model_input
                                    .update(cx, |i, cx| i.set_value(pm, window, cx));
                                cx.notify();
                            });
                        },
                    )
                    .h(px(32.0))
                    .rounded(px(9999.0))
                })),
            ),
        );
        card = card
            .child(key_row(
                editing,
                has_key,
                &key,
                &self.key_input,
                &session,
                &view,
                cx,
            ))
            .child(
                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .text_color(ui::fg2(cx))
                    .child(key_status),
            );
        for (is_trans, current, models, input, label) in [
            (
                true,
                tm,
                trans_models,
                self.transcribe_model_input.clone(),
                "转写模型",
            ),
            (
                false,
                pm,
                chat_models,
                self.polish_model_input.clone(),
                "润色模型",
            ),
        ] {
            let save_input = input.clone();
            let save_session = session.clone();
            card =
                card.child(
                    setting_row(
                        label,
                        if is_trans && editor == ProviderKind::OpenRouter {
                            "此模型用于当前渠道的语音识别。"
                        } else {
                            "填写模型 ID，或展开目录选择。"
                        },
                        true,
                        cx,
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(div().flex().gap(px(8.0)).child(Input::new(&input)).child(
                                button(
                                    format!("save-model-{is_trans}"),
                                    "保存模型",
                                    false,
                                    move |_, cx| {
                                        let value = save_input.read(cx).value().to_string();
                                        save_session.update(cx, |s, cx| {
                                            if is_trans {
                                                s.set_transcribe_model(&value, cx)
                                            } else {
                                                s.set_polish_model(&value, cx)
                                            }
                                        });
                                    },
                                ),
                            ))
                            .child(choice_row().children(models.into_iter().map(|model| {
                                let session = session.clone();
                                let input = input.clone();
                                let id = model.id;
                                let selected = current == id;
                                mode_chip(
                                    format!("model-{is_trans}-{id}"),
                                    id.clone(),
                                    selected,
                                    move |window, cx| {
                                        input.update(cx, |i, cx| i.set_value(&id, window, cx));
                                        session.update(cx, |s, cx| {
                                            if is_trans {
                                                s.set_transcribe_model(&id, cx)
                                            } else {
                                                s.set_polish_model(&id, cx)
                                            }
                                        });
                                    },
                                    cx,
                                )
                            }))),
                    ),
                );
        }
        let refresh = session.clone();
        let expand_view = view.clone();
        let test = session.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(ui::section_label("自带密钥 · 渠道与模型", None))
            .child(
                card.child(
                    setting_row(
                        "模型目录",
                        "按需展开，每类显示最多 6 项；更多模型可填写 ID。",
                        true,
                        cx,
                    )
                    .child(button(
                        "toggle-model-catalog",
                        if self.models_expanded {
                            "收起目录"
                        } else {
                            "展开目录"
                        },
                        false,
                        move |_, cx| {
                            expand_view.update(cx, |v, cx| {
                                v.models_expanded = !v.models_expanded;
                                cx.notify();
                            });
                        },
                    ))
                    .child(button(
                        "refresh-models",
                        "更新目录",
                        false,
                        move |_, cx| {
                            refresh.update(cx, |s, cx| s.refresh_models(cx));
                        },
                    )),
                )
                .child(
                    div()
                        .px(px(16.0))
                        .text_color(ui::fg2(cx))
                        .child(model_status),
                )
                .child(connection_status_row(testing, status, &session, cx))
                .child(
                    div()
                        .px(px(16.0))
                        .text_size(px(13.0))
                        .text_color(ui::fg2(cx))
                        .child(
                            transcription_status
                                .map(|(_, message)| message)
                                .unwrap_or_else(|| "转写模型尚未测试".into()),
                        ),
                )
                .child(
                    setting_row(
                        "转写测试",
                        "发送合成测试语音，可能产生少量渠道费用。",
                        false,
                        cx,
                    )
                    .child(button(
                        "test-transcription",
                        if testing {
                            "正在测试…"
                        } else {
                            "测试转写"
                        },
                        false,
                        move |_, cx| {
                            test.update(cx, |s, cx| s.test_transcription(cx));
                        },
                    )),
                ),
            )
    }
    pub fn recovery_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.clone();
        let refresh = session.clone();
        let view = cx.entity();
        let s = session.read(cx);
        let status = s.recovery_status.clone();
        let count = s.recovery_tasks.len();
        let rows = s
            .recovery_tasks
            .iter()
            .take(self.recovery_limit)
            .map(|task| {
                let retry = session.clone();
                let remove = session.clone();
                let retry_id = task.id.clone();
                let remove_id = task.id.clone();
                let copy_session = session.clone();
                let copy_id = task.id.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(16.0))
                    .border_b_1()
                    .border_color(ui::hairline(cx))
                    .child(format!("{} · {}", task.phase, task.id))
                    .child(div().text_color(ui::fg2(cx)).child(task.error.clone()))
                    .child(
                        div()
                            .text_color(ui::fg2(cx))
                            .child(task.transcript.chars().take(120).collect::<String>()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child(button(
                                format!("retry-{}", task.id),
                                "按当前配置重试",
                                false,
                                move |_, cx| {
                                    retry.update(cx, |s, cx| s.retry_task(&retry_id, cx));
                                },
                            ))
                            .child(button(
                                format!("copy-{}", task.id),
                                "复制已保存内容",
                                false,
                                move |_, cx| {
                                    if let Some(text) = copy_session
                                        .read(cx)
                                        .recovery_tasks
                                        .iter()
                                        .find(|task| task.id == copy_id)
                                        .map(|task| task.output.clone())
                                        .filter(|text| !text.is_empty())
                                    {
                                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                            text,
                                        ));
                                    }
                                },
                            ))
                            .child(button(
                                format!("delete-task-{}", task.id),
                                "删除草稿",
                                false,
                                move |_, cx| {
                                    remove.update(cx, |s, cx| s.delete_task(&remove_id, cx));
                                },
                            )),
                    )
            })
            .collect::<Vec<_>>();
        let mut content = ui::card(cx)
            .child(
                setting_row(
                    "录音与恢复",
                    "最长录音 10 分钟。中断草稿保留 7 天；重试使用当前渠道配置并保留已完成片段，结果不自动粘贴。",
                    true,
                    cx,
                )
                .child(button(
                    "refresh-recovery",
                    "刷新恢复任务",
                    false,
                    move |_, cx| {
                        refresh.update(cx, |s, cx| s.refresh_recovery(cx));
                    },
                )),
            )
            .child(div().px(px(16.0)).text_color(ui::fg2(cx)).child(status))
            .children(rows);
        if count > self.recovery_limit {
            content = content.child(button(
                "more-recovery",
                "显示更多",
                false,
                move |_, cx| {
                    view.update(cx, |v, cx| {
                        v.recovery_limit += 25;
                        cx.notify();
                    });
                },
            ));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(ui::section_label("未完成的口述", None))
            .child(content)
    }
}
