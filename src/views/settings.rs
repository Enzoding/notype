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
            Self::Channel => crate::i18n::t("settings.section.channel"),
            Self::Recording => crate::i18n::t("settings.section.recording"),
            Self::Input => crate::i18n::t("settings.section.input"),
            Self::Writing => crate::i18n::t("settings.section.writing"),
            Self::System => crate::i18n::t("settings.section.system"),
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
            ui_language,
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
                s.ui_language,
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
                    .child(ui::section_label(crate::i18n::t("settings.input.section_title"), None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(setting_row(
                                crate::i18n::t("settings.input.mic_permission"),
                                crate::i18n::t("settings.input.mic_desc"),
                                true,
                                cx,
                            ).child(
                                div().flex().items_center().gap(px(12.0))
                                    .child(ui::pill(microphone_access.label(),if microphone_access.granted(){ui::ok(cx)}else{ui::warn(cx)},if microphone_access.granted(){ui::ok_soft(cx)}else{ui::warn_soft(cx)}))
                                    .child(button(
                                        "microphone-permission",
                                        if microphone_access.granted() {
                                            crate::i18n::t("settings.input.manage_mic")
                                        } else {
                                            crate::i18n::t("settings.input.grant_mic")
                                        },
                                        false,
                                        |_,_|{crate::permissions::request_microphone();}
                                    ))
                            ))
                            .child(
                                setting_row(
                                    crate::i18n::t("settings.input.accessibility"),
                                    crate::i18n::t("settings.input.accessibility_desc"),
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
                                                crate::i18n::t("permission.authorized")
                                            } else {
                                                crate::i18n::t("permission.denied")
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
                                                crate::i18n::t("settings.input.open_accessibility")
                                            } else {
                                                crate::i18n::t("settings.input.grant_accessibility")
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
                                    crate::i18n::t("settings.input.hotkey"),
                                    crate::i18n::t("settings.input.hotkey_desc"),
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
                                    crate::i18n::t("settings.input.trigger_mode"),
                                    crate::i18n::t("settings.input.trigger_mode_desc"),
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
                            ),
                    ),
            ))
            // 2. Microphone & Audio Section
            .when(section==SettingsSection::Input, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label(crate::i18n::t("settings.audio.section_title"), None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Microphone device selector
                            .child(
                                setting_row(
                                    crate::i18n::t("settings.audio.device"),
                                    crate::i18n::t("settings.audio.device_desc"),
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row()
                                        .child({
                                            let s = session_handle.clone();
                                            mode_chip(
                                                "mic-default",
                                                crate::i18n::t("settings.audio.system_default"),
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
                                                crate::i18n::t("settings.audio.refresh_list"),
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
                                    crate::i18n::t("settings.audio.test"),
                                    crate::i18n::t("settings.audio.test_desc"),
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
                                                if mic_testing {
                                                    crate::i18n::t("settings.audio.stop_test")
                                                } else {
                                                    crate::i18n::t("settings.audio.start_test")
                                                },
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
                                    crate::i18n::t("settings.audio.sound_feedback"),
                                    crate::i18n::t("settings.audio.sound_feedback_desc"),
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    let label = crate::i18n::t("settings.audio.sound_feedback");
                                    Switch::new("switch-sound-cue")
                                        .checked(sound_feedback)
                                        .accessibility_label(label)
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
                                    crate::i18n::t("settings.audio.save_audio"),
                                    crate::i18n::t("settings.audio.save_audio_desc"),
                                    false,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    let label = crate::i18n::t("settings.audio.save_audio");
                                    Switch::new("switch-save-audio")
                                        .checked(save_audio)
                                        .accessibility_label(label)
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
                    .child(ui::section_label(crate::i18n::t("settings.writing.section_title"), None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            // Strategy selector
                            .child(
                                setting_row(
                                    crate::i18n::t("settings.writing.mode"),
                                    crate::i18n::t("settings.writing.mode_desc"),
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row()
                                        .children(
                                            [
                                                ("auto", crate::i18n::t("polish.mode.auto")),
                                                ("prose", crate::i18n::t("polish.mode.prose")),
                                                ("literal", crate::i18n::t("polish.mode.literal")),
                                                ("email", crate::i18n::t("polish.mode.email")),
                                                ("chat", crate::i18n::t("polish.mode.chat")),
                                                ("raw", crate::i18n::t("polish.mode.raw")),
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
                                    crate::i18n::t("settings.writing.space_cjk"),
                                    crate::i18n::t("settings.writing.space_cjk_desc"),
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    let label = crate::i18n::t("settings.writing.space_cjk");
                                    Switch::new("switch-space-cjk")
                                        .checked(space_cjk)
                                        .accessibility_label(label)
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
                                                    .child(crate::i18n::t("settings.writing.custom_prompt")),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .text_size(px(13.0))
                                                    .text_color(ui::fg2(cx))
                                                    .child(if custom_prompt.is_empty() {
                                                        crate::i18n::t("settings.writing.custom_prompt_empty").to_string()
                                                    } else {
                                                        let prompt_preview = ellipsize(&custom_prompt, 26);
                                                        crate::i18n::t_args(
                                                            "settings.writing.custom_prompt_active",
                                                            &[("prompt", &prompt_preview)],
                                                        )
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
                                                    crate::i18n::t("settings.writing.save_prompt"),
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
                                                    .child(crate::i18n::t("settings.writing.examples")),
                                            )
                                            .children({
                                                let ex_bullets = crate::i18n::t("settings.writing.example_bullets_text");
                                                let ex_concise = crate::i18n::t("settings.writing.example_concise_text");
                                                let ex_formal = crate::i18n::t("settings.writing.example_formal_text");
                                                [
                                                    (crate::i18n::t("settings.writing.example_default"), ""),
                                                    (crate::i18n::t("settings.writing.example_concise"), ex_concise),
                                                    (crate::i18n::t("settings.writing.example_formal"), ex_formal),
                                                    (crate::i18n::t("settings.writing.example_bullets"), ex_bullets),
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
                                                })
                                            }),
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
                    .child(ui::section_label(crate::i18n::t("settings.learning.section_title"), None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(
                                setting_row(
                                    crate::i18n::t("settings.learning.auto_learn"),
                                    crate::i18n::t("settings.learning.auto_learn_desc"),
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    let label = crate::i18n::t("settings.learning.auto_learn");
                                    Switch::new("switch-auto-learn")
                                        .checked(auto_learn_mode != AutoLearnMode::Off)
                                        .accessibility_label(label)
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
                                    crate::i18n::t("settings.learning.mode"),
                                    crate::i18n::t("settings.learning.mode_desc"),
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
                                    crate::i18n::t("settings.learning.learn_from_edits"),
                                    crate::i18n::t("settings.learning.learn_from_edits_desc"),
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    let label = crate::i18n::t("settings.learning.learn_from_edits");
                                    Switch::new("switch-learn-edits")
                                        .checked(learn_from_edits)
                                        .accessibility_label(label)
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
                                    .child(crate::i18n::t("settings.learning.privacy_hint")),
                            ),
                    ),
            ))
            // 5. System & Storage Section
            .when(section==SettingsSection::System, |col| col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(ui::section_label(crate::i18n::t("settings.system.section_title"), None).text_color(ui::fg2(cx)))
                    .child(
                        ui::card(cx)
                            .child(
                                setting_row(
                                    crate::i18n::t("settings.language.title"),
                                    crate::i18n::t("settings.language.subtitle"),
                                    true,
                                    cx,
                                )
                                .child(
                                    choice_row().children(
                                        [
                                            (crate::i18n::Preference::System, crate::i18n::t("settings.language.system")),
                                            (crate::i18n::Preference::ZhHans, "简体中文"),
                                            (crate::i18n::Preference::En, "English"),
                                        ]
                                        .into_iter()
                                        .map(|(pref, label)| {
                                            let active = ui_language == pref;
                                            let s = session_handle.clone();
                                            mode_chip(
                                                format!("lang-{pref:?}"),
                                                label,
                                                active,
                                                move |_, cx| {
                                                    s.update(cx, |session, cx| {
                                                        session.set_ui_language(pref, cx);
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
                                    crate::i18n::t("settings.system.dock"),
                                    crate::i18n::t("settings.system.dock_desc"),
                                    true,
                                    cx,
                                )
                                .child({
                                    let s = session_handle.clone();
                                    let label = crate::i18n::t("settings.system.dock");
                                    Switch::new("switch-show-dock")
                                        .checked(show_dock)
                                        .accessibility_label(label)
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
                                    crate::i18n::t("settings.system.data_dir"),
                                    &data_dir,
                                    false,
                                    cx,
                                )
                                .child(button(
                                    "open-data-dir",
                                    crate::i18n::t("settings.system.open_finder"),
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
