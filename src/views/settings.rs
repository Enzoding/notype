use std::process::Command;

use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{Context, FontWeight, IntoElement, ParentElement, Styled, div, px, rems};

use crate::hotkey::{self, HOTKEY_OPTIONS};
use crate::secrets;
use crate::store::{Store, TriggerMode};
use crate::ui;
use crate::views::SettingsView;
use crate::views::components::{
    button, ellipsize, icon_button, inline_wave_view, key_row, mode_chip, path_row, setting_row,
};

impl SettingsView {
    pub fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let session_handle = self.session.clone();
        let key_input = self.key_input.clone();
        let custom_prompt_input = self.custom_prompt_input.clone();
        let key_path = secrets::env_path().display().to_string();
        let data_dir = Store::data_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~/Library/Application Support/OpenTypeless".into());

        let (
            has_key,
            key_status,
            gateway_test_status,
            gateway_testing,
            hotkey_code,
            trigger_mode,
            polish_strategy,
            custom_prompt,
            space_cjk,
            sound_feedback,
            save_audio,
            mic_device,
            available_mics,
            mic_testing,
            wave,
            show_dock,
            transcribe_model,
            polish_model,
        ) = {
            let s = self.session.read(cx);
            (
                s.has_key(),
                s.key_status.clone(),
                s.gateway_test_status.clone(),
                s.gateway_testing,
                s.hotkey_code,
                s.trigger_mode,
                s.polish_strategy.clone(),
                s.custom_prompt.clone(),
                s.space_cjk,
                s.sound_feedback,
                s.save_audio,
                s.mic_device.clone(),
                s.available_mics.clone(),
                s.mic_testing,
                s.wave,
                s.show_dock,
                s.transcribe_model.clone(),
                s.polish_model.clone(),
            )
        };

        let key_editing = self.key_editing || !has_key;
        let ax_trusted = hotkey::is_accessibility_trusted() || hotkey::is_tap_active();

        div()
            .flex()
            .flex_col()
            .gap(px(22.0))
            // 1. Hotkey & Accessibility Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("快捷键与触发方式", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Trigger Key Row
                            .child(
                                setting_row(
                                    "全局口述快捷键",
                                    "在任意应用中按下此按键即可唤起口述小条",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .children(HOTKEY_OPTIONS.iter().map(|(code, label, badge)| {
                                            let code_val = *code;
                                            let active = hotkey_code == code_val;
                                            let s = session_handle.clone();
                                            mode_chip(
                                                format!("hk-{code_val}"),
                                                format!("{label} ({badge})"),
                                                active,
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.set_hotkey_code(code_val, cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        })),
                                ),
                            )
                            // Trigger Mode Row
                            .child(
                                setting_row(
                                    "触发手势模式",
                                    "选择单击切换或按住说话（松开即自动转写贴入）",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .children(
                                            [TriggerMode::Toggle, TriggerMode::Hold]
                                                .into_iter()
                                                .map(|m| {
                                                    let active = trigger_mode == m;
                                                    let s = session_handle.clone();
                                                    mode_chip(
                                                        format!("tm-{}", m.as_str()),
                                                        m.short_label(),
                                                        active,
                                                        move |_, cx| {
                                                            s.update(cx, |session, cx| {
                                                                session.set_trigger_mode(m, cx);
                                                            });
                                                        },
                                                        cx,
                                                    )
                                                }),
                                        ),
                                ),
                            )
                            // Accessibility Permission Row
                            .child(
                                setting_row(
                                    "macOS 辅助功能权限",
                                    "用于监听全局快捷键及自动执行 ⌘V 贴入前景窗口",
                                    false,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(ui::pill(
                                            if ax_trusted {
                                                "已授权并生效"
                                            } else {
                                                "需要授权"
                                            },
                                            if ax_trusted {
                                                ui::ok(cx)
                                            } else {
                                                ui::warn(cx)
                                            },
                                            if ax_trusted {
                                                ui::ok_soft(cx)
                                            } else {
                                                ui::warn_soft(cx)
                                            },
                                        ))
                                        .child(button(
                                            "open-ax-settings",
                                            "系统设置",
                                            false,
                                            |_, _| {
                                                let _ = Command::new("open")
                                                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                                                    .spawn();
                                            },
                                        )),
                                ),
                            ),
                    ),
            )
            // 2. Microphone & Audio Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("麦克风与音频反馈", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Microphone device selector
                            .child(
                                setting_row(
                                    "音频输入设备",
                                    &format!("当前默认设备：{}", crate::capture::default_device_name()),
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .flex_wrap()
                                        .gap(px(6.0))
                                        .child({
                                            let s = session_handle.clone();
                                            mode_chip(
                                                "mic-default",
                                                "系统默认",
                                                mic_device.is_empty(),
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.set_mic_device("", cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        })
                                        .children(available_mics.into_iter().enumerate().map(
                                            |(idx, dev_name)| {
                                                let active = mic_device == dev_name;
                                                let s = session_handle.clone();
                                                let name_clone = dev_name.clone();
                                                mode_chip(
                                                    format!("mic-dev-{idx}"),
                                                    ellipsize(&dev_name, 22),
                                                    active,
                                                    move |_, cx| {
                                                        s.update(cx, |session, cx| {
                                                            session.set_mic_device(&name_clone, cx);
                                                        });
                                                    },
                                                    cx,
                                                )
                                            },
                                        ))
                                        .child({
                                            let s = session_handle.clone();
                                            icon_button(
                                                "refresh-mics",
                                                IconName::RefreshCw,
                                                "刷新麦克风列表",
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.refresh_mics(cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        }),
                                ),
                            )
                            // Live Microphone Test Row
                            .child(
                                setting_row(
                                    "实时电平测试",
                                    "开启后对着麦克风说话，检查收音波纹是否灵敏",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .when(mic_testing, |row| row.child(inline_wave_view(wave, cx)))
                                        .child({
                                            let s = session_handle.clone();
                                            button(
                                                "toggle-mic-test",
                                                if mic_testing { "停止测试" } else { "测试麦克风" },
                                                mic_testing,
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.toggle_mic_test(cx);
                                                    });
                                                },
                                            )
                                        }),
                                ),
                            )
                            // Sound cues switch
                            .child(
                                setting_row(
                                    "状态提示音效",
                                    "开始录音、结束录音与贴入完成时播放轻柔的 macOS 原生提示音",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-sound-cue")
                                        .checked(sound_feedback)
                                        .accessibility_label("状态提示音效")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_sound_feedback(val, cx);
                                            });
                                        })
                                }),
                            )
                            // Save audio files switch
                            .child(
                                setting_row(
                                    "保留本地录音文件",
                                    "将每次口述保存为 16kHz WAV 文件，支持在「口述历史」中随时回放",
                                    false,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-save-audio")
                                        .checked(save_audio)
                                        .accessibility_label("保留本地录音文件")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_save_audio(val, cx);
                                            });
                                        })
                                }),
                            ),
                    ),
            )
            // 3. Polish Strategy & Style Rules Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("润色策略与文风偏好", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Strategy selector
                            .child(
                                setting_row(
                                    "默认润色策略",
                                    "默认根据最前面的应用（如 VS Code、邮件、微信、备忘录）自动切换模式",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .flex_wrap()
                                        .gap(px(6.0))
                                        .children(
                                            [
                                                ("auto", "智能识别应用"),
                                                ("prose", "流畅正文"),
                                                ("literal", "代码终端"),
                                                ("email", "商务邮件"),
                                                ("chat", "即时通讯"),
                                                ("raw", "仅转写不润色"),
                                            ]
                                            .into_iter()
                                            .map(|(code, label)| {
                                                let active = polish_strategy == code;
                                                let s = session_handle.clone();
                                                mode_chip(
                                                    format!("pol-strat-{code}"),
                                                    label,
                                                    active,
                                                    move |_, cx| {
                                                        s.update(cx, |session, cx| {
                                                            session.set_polish_strategy(code, cx);
                                                        });
                                                    },
                                                    cx,
                                                )
                                            }),
                                        ),
                                ),
                            )
                            // CJK spacing switch
                            .child(
                                setting_row(
                                    "中英文与数字自动留白",
                                    "在中文与英文单词、数字之间自动插入半角空格（如「使用 Rust 开发」）",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-space-cjk")
                                        .checked(space_cjk)
                                        .accessibility_label("中英文与数字自动留白")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_space_cjk(val, cx);
                                            });
                                        })
                                }),
                            )
                            // Custom style instruction
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.0))
                                    .px(px(14.0))
                                    .py(px(12.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(rems(0.8125))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("自定义润色指令"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(rems(0.6875))
                                                    .text_color(ui::fg3(cx))
                                                    .child(if custom_prompt.is_empty() {
                                                        "当前使用系统默认指令".to_string()
                                                    } else {
                                                        format!("已生效：{}", ellipsize(&custom_prompt, 26))
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
                                                    .child(Input::new(&custom_prompt_input).w_full()),
                                            )
                                            .child({
                                                let s = session_handle.clone();
                                                let inp = custom_prompt_input.clone();
                                                button(
                                                    "save-custom-prompt",
                                                    "应用指令",
                                                    false,
                                                    move |_, cx| {
                                                        let val = inp.read(cx).value().to_string();
                                                        s.update(cx, |session, cx| {
                                                            session.set_custom_prompt(&val, cx);
                                                        });
                                                    },
                                                )
                                            }),
                                    )
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
                                                    .child("快捷风格："),
                                            )
                                            .children(
                                                [
                                                    ("默认纯净", ""),
                                                    ("精炼简短", "尽量精炼简短，删去客套话，直奔主题。"),
                                                    ("书面专业", "使用克制、专业的书面表达，适合技术文档与正式汇报。"),
                                                    ("结构化要点", "遇到多项内容时，优先使用清晰的要点列表（- ）排版。"),
                                                ]
                                                .into_iter()
                                                .enumerate()
                                                .map(|(idx, (title, rule))| {
                                                    let active = custom_prompt == rule;
                                                    let s = session_handle.clone();
                                                    let inp = custom_prompt_input.clone();
                                                    mode_chip(
                                                        format!("prompt-preset-{idx}"),
                                                        title,
                                                        active,
                                                        move |window, cx| {
                                                            inp.update(cx, |input, cx| {
                                                                input.set_value(rule, window, cx);
                                                            });
                                                            s.update(cx, |session, cx| {
                                                                session.set_custom_prompt(rule, cx);
                                                            });
                                                        },
                                                        cx,
                                                    )
                                                }),
                                            ),
                                    ),
                            ),
                    ),
            )
            // 4. AI Gateway & Models Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("Vercel AI Gateway 连接", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(key_row(
                                key_editing,
                                has_key,
                                gateway_testing,
                                &key_input,
                                &session_handle,
                                &view,
                                cx,
                            ))
                            .child(path_row(
                                &key_path,
                                &key_status,
                                gateway_test_status,
                                cx,
                            ))
                            .child(
                                setting_row(
                                    "处理管线模型",
                                    &format!("转写：{transcribe_model}  ·  润色：{polish_model}"),
                                    false,
                                    cx,
                                )
                                .child(ui::pill(
                                    "AI Gateway v4",
                                    ui::accent(cx),
                                    ui::accent_soft(cx),
                                )),
                            ),
                    ),
            )
            // 5. System & Storage Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("系统与本机存储", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(
                                setting_row(
                                    "在 Dock 中显示应用图标",
                                    "关闭后仅保留顶部菜单栏图标与全局快捷键常驻",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-show-dock")
                                        .checked(show_dock)
                                        .accessibility_label("在 Dock 中显示应用图标")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_dock(val, cx);
                                            });
                                        })
                                }),
                            )
                            .child(
                                setting_row(
                                    "本机数据库与录音目录",
                                    &data_dir,
                                    false,
                                    cx,
                                )
                                .child(button(
                                    "open-data-dir",
                                    "在 Finder 中打开",
                                    false,
                                    move |_, _| {
                                        if let Ok(dir) = Store::data_dir() {
                                            let _ = Command::new("open").arg(dir).spawn();
                                        }
                                    },
                                )),
                            ),
                    ),
            )
    }
}
