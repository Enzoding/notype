use std::process::Command;

use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::IconName;
use gpui_kit::component::Sizable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{div, px, rems, Context, FontWeight, IntoElement, ParentElement, Styled};

use crate::hotkey::{self, HOTKEY_OPTIONS};
use crate::mining::AutoLearnMode;
use crate::store::{Store, TriggerMode};
use crate::ui;
use crate::views::components::{
    button, choice_row, ellipsize, icon_button, inline_wave_view, mode_chip, setting_row,
};
use crate::views::SettingsView;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Channel,
    Recording,
    Input,
    Writing,
    System,
}
impl SettingsSection {
    pub const ALL: [Self; 5] = [
        Self::Channel,
        Self::Recording,
        Self::Input,
        Self::Writing,
        Self::System,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Channel => "AI 渠道",
            Self::Recording => "录音与恢复",
            Self::Input => "快捷键与麦克风",
            Self::Writing => "润色与学习",
            Self::System => "本机",
        }
    }
}

impl SettingsView {
    pub fn settings_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let active = self.settings_section;
        div()
            .flex()
            .flex_wrap()
            .flex_shrink_0()
            .px(px(28.0))
            .py(px(12.0))
            .gap(px(8.0))
            .children(SettingsSection::ALL.map(|section| {
                let view = view.clone();
                button(
                    format!("settings-section-{section:?}"),
                    section.label(),
                    active == section,
                    move |_, cx| {
                        view.update(cx, |v, cx| {
                            v.settings_section = section;
                            cx.notify();
                        });
                    },
                )
                .h(px(32.0))
                .rounded(px(9999.0))
            }))
    }
    pub fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let session_handle = self.session.clone();
        let custom_prompt_input = self.custom_prompt_input.clone();
        let data_dir = self.session.read(cx).data_dir_display.clone();

        let (
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
            auto_learn_mode,
            learn_from_edits,
        ) = {
            let s = self.session.read(cx);
            (
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
                s.auto_learn_mode,
                s.learn_from_edits,
            )
        };

        let ax_trusted = self.session.read(cx).accessibility_trusted;
        let microphone_access = self.session.read(cx).microphone_access;
        let section = self.settings_section;

        div()
            .flex()
            .flex_col()
            .gap(px(32.0))
            .when(section==SettingsSection::Channel,|col|col.child(self.provider_settings(cx)))
            .when(section==SettingsSection::Recording,|col|col.child(self.recovery_settings(cx)))
            // 1. Hotkey & Accessibility Section
            .when(section==SettingsSection::Input, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("权限与快捷键", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(setting_row("麦克风权限","用于录制你的声音。",true,cx).child(
                                div().flex().items_center().gap(px(12.0))
                                    .child(ui::pill(microphone_access.label(),if microphone_access.granted(){ui::ok(cx)}else{ui::warn(cx)},if microphone_access.granted(){ui::ok_soft(cx)}else{ui::warn_soft(cx)}))
                                    .child(button("microphone-permission",if microphone_access.granted(){"管理麦克风权限"}else{"授权麦克风"},false,|_,_|{crate::permissions::request_microphone();}))
                            ))
                            .child(
                                setting_row(
                                    "辅助功能",
                                    "识别快捷键，并把文字贴进当前窗口。",
                                    false,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .child(ui::pill(
                                            if ax_trusted {
                                                "已授权"
                                            } else {
                                                "未授权"
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
                                            if ax_trusted {
                                                "打开辅助功能"
                                            } else {
                                                "授予辅助功能权限"
                                            },
                                            false,
                                            |_, _| {
                                                hotkey::request_accessibility_permission();
                                                let _ = Command::new("open")
                                                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                                                    .spawn();
                                            },
                                        )),
                                ),
                            )
                            // Trigger Key Row
                            .child(
                                setting_row(
                                    "口述快捷键",
                                    "在任何应用里按下，开始口述。",
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row()
                                        .children(HOTKEY_OPTIONS.iter().map(|(code, label, _badge)| {
                                            let code_val = *code;
                                            let active = hotkey_code == code_val;
                                            let s = session_handle.clone();
                                            mode_chip(
                                                format!("hk-{code_val}"),
                                                (*label).to_string(),
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
                                    "开始方式",
                                    "再按一次结束，或松开后转写。",
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row()
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
,
                    ),
            ))
            // 2. Microphone & Audio Section
            .when(section==SettingsSection::Input, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("麦克风", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Microphone device selector
                            .child(
                                setting_row(
                                    "麦克风",
                                    "选择口述时用的输入设备。",
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row()
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
                                    "试音",
                                    "对着麦克风说话，确认波形有反应。",
                                    true,
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .when(mic_testing, |row| row.child(inline_wave_view(wave, ui::live(cx))))
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
                                    "播放提示音",
                                    "开始、结束和贴入完成时播放一声提示。",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-sound-cue")
                                        .checked(sound_feedback)
                                        .accessibility_label("播放提示音")
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
                                    "保存录音",
                                    "保存每次口述，之后可在口述历史里回放。",
                                    false,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-save-audio")
                                        .checked(save_audio)
                                        .accessibility_label("保存录音")
                                        .on_change(move |checked, _, cx| {
                                            let val = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_save_audio(val, cx);
                                            });
                                        })
                                }),
                            ),
                    ),
            ))
            // 3. Polish Strategy & Style Rules Section
            .when(section==SettingsSection::Writing, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("润色", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Strategy selector
                            .child(
                                setting_row(
                                    "润色方式",
                                    "自动会按当前应用选择，也可以固定成一种。",
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row()
                                        .children(
                                            [
                                                ("auto", "自动"),
                                                ("prose", "正文"),
                                                ("literal", "代码"),
                                                ("email", "邮件"),
                                                ("chat", "聊天"),
                                                ("raw", "只转写"),
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
                                    "中英文之间加空格",
                                    "打开后，中文和英文、数字之间会空一格，例如「使用 Rust 开发」。",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-space-cjk")
                                        .checked(space_cjk)
                                        .accessibility_label("中英文之间加空格")
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
                                    .gap(px(12.0))
                                    .px(px(16.0))
                                    .py(px(14.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("补充要求"),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .text_size(px(13.0))
                                                    .text_color(ui::fg2(cx))
                                                    .child(if custom_prompt.is_empty() {
                                                        "留空则使用默认润色。".to_string()
                                                    } else {
                                                        format!("正在使用：{}", ellipsize(&custom_prompt, 26))
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .child(Input::new(&custom_prompt_input).large().w_full()),
                                            )
                                            .child({
                                                let s = session_handle.clone();
                                                let inp = custom_prompt_input.clone();
                                                button(
                                                    "save-custom-prompt",
                                                    "保存要求",
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
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .text_color(ui::fg2(cx))
                                                    .child("示例"),
                                            )
                                            .children(
                                                [
                                                    ("默认", ""),
                                                    ("精炼", "尽量精炼简短，删去客套话，直奔主题。"),
                                                    ("书面", "使用克制、专业的书面表达，适合技术文档与正式汇报。"),
                                                    ("分点", "遇到多项内容时，优先使用清晰的要点列表（- ）排版。"),
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
            ))
            // 4. Personal dictionary learning
            .when(section==SettingsSection::Writing, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("个人词库", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(
                                setting_row(
                                    "自动学习词条",
                                    "从历史成稿里提取产品名、人名与术语。高置信直接加入，其余进「待确认」。",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-auto-learn")
                                        .checked(auto_learn_mode != AutoLearnMode::Off)
                                        .accessibility_label("自动学习词条")
                                        .on_change(move |checked, _, cx| {
                                            let mode = if *checked {
                                                AutoLearnMode::Auto
                                            } else {
                                                AutoLearnMode::Off
                                            };
                                            s.update(cx, |session, cx| {
                                                session.set_auto_learn_mode(mode, cx);
                                            });
                                        })
                                }),
                            )
                            .child(
                                setting_row(
                                    "学习方式",
                                    "自动入库更省事；只给建议更可控。",
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row().children(
                                        [AutoLearnMode::Auto, AutoLearnMode::Suggest]
                                            .into_iter()
                                            .map(|mode| {
                                                let active = auto_learn_mode == mode;
                                                let s = session_handle.clone();
                                                mode_chip(
                                                    format!("settings-learn-{}", mode.as_str()),
                                                    mode.label(),
                                                    active,
                                                    move |_, cx| {
                                                        s.update(cx, |session, cx| {
                                                            session.set_auto_learn_mode(mode, cx);
                                                        });
                                                    },
                                                    cx,
                                                )
                                            }),
                                    ),
                                ),
                            )
                            .child(
                                setting_row(
                                    "从粘贴后的修正中学习",
                                    "粘贴完成 8 秒后读取一次前台输入框，只提取你改动里的术语与拼写，不保存整段内容。",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-learn-edits")
                                        .checked(learn_from_edits)
                                        .accessibility_label("从粘贴后的修正中学习")
                                        .on_change(move |checked, _, cx| {
                                            let enabled = *checked;
                                            s.update(cx, |session, cx| {
                                                session.set_learn_from_edits(enabled, cx);
                                            });
                                        })
                                }),
                            )
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(12.0))
                                    .text_size(px(13.0))
                                    .line_height(rems(1.45))
                                    .text_color(ui::fg3(cx))
                                    .child("AI 提取会把历史文本发送到当前 AI Gateway；不发送录音。关闭自动学习后，手动词条仍然生效。"),
                            ),
                    ),
            ))
            // 5. System & Storage Section
            .when(section==SettingsSection::System, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label("本机", None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(
                                setting_row(
                                    "在 Dock 中显示",
                                    "打开后，图标出现在 Dock。菜单栏里的图标仍然在。",
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    Switch::new("switch-show-dock")
                                        .checked(show_dock)
                                        .accessibility_label("在 Dock 中显示")
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
                                    "数据位置",
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
            ))
    }
}
