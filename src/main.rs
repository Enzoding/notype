mod audio;
mod capture;
mod front;
mod gateway;
mod hotkey;
mod overlay;
mod paste;
mod polish;
mod secrets;
mod session;
mod store;
mod ui;
mod views;

use std::time::Duration;

use gpui_kit::{
    ActivationPolicy, AnyWindowHandle, App, AppContext, Bounds, TitlebarOptions, WindowBounds,
    WindowKind, WindowOptions, application, assets, init, open_window, point, px, size,
};
use session::Session;
use views::SettingsView;

fn main() {
    application().with_assets(assets::AllAssets).run(|cx| {
        init(cx);

        let session = cx.new(|cx| Session::new(cx));
        let (show_dock, initial_code, initial_mode) = {
            let s = session.read(cx);
            (s.show_dock, s.hotkey_code, s.trigger_mode)
        };
        cx.set_activation_policy(if show_dock {
            ActivationPolicy::Regular
        } else {
            ActivationPolicy::Accessory
        });
        session.update(cx, |session, cx| session.watch_level(cx));

        let mut settings = open_settings(cx, session.clone());
        open_voice_bar(cx, session.clone());
        overlay::prepare_voice_window();
        if let Some(tray) = install_tray() {
            std::mem::forget(tray);
        } else {
            eprintln!("状态栏图标没有创建成功");
        }

        let events = session.clone();
        cx.spawn(async move |cx| {
            let hotkeys = hotkey::listen(initial_code, initial_mode);
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let mut hk_events = Vec::new();
                while let Ok(ev) = hotkeys.try_recv() {
                    hk_events.push(ev);
                }
                let mut menu_ids = Vec::new();
                while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    menu_ids.push(event.id.0);
                }
                if hk_events.is_empty() && menu_ids.is_empty() {
                    continue;
                }
                let events = events.clone();
                settings = cx.update(|cx| {
                    for ev in hk_events {
                        events.update(cx, |session, cx| session.handle_hotkey_event(ev, cx));
                    }
                    for id in menu_ids {
                        match id.as_str() {
                            "toggle" => events.update(cx, |session, cx| session.toggle(cx)),
                            "quit" => cx.quit(),
                            "show" => settings = reveal_settings(cx, events.clone(), settings),
                            "dock" => {
                                events.update(cx, |session, cx| {
                                    session.set_dock(!session.show_dock, cx);
                                });
                            }
                            _ => {}
                        }
                    }
                    settings
                });
            }
        })
        .detach();

        reveal_settings(cx, session, settings);
    });
}

fn open_settings(cx: &mut App, session: gpui_kit::Entity<Session>) -> AnyWindowHandle {
    let initial_prompt = session.read(cx).custom_prompt.clone();
    let (window, _) = open_window(settings_options(cx), cx, move |window, cx| {
        gpui_kit::component::Theme::change(window.appearance(), Some(window), cx);
        let key_input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx).placeholder("vck_…")
        });
        let phrase_input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("输入专有名词（支持逗号分隔批量添加），如 OpenTypeless, GPUI")
        });
        let studio_input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("在此输入或粘贴一段口语草稿，或点击上方「示例草稿」测试润色效果…")
        });
        let history_search_input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("搜索历史口述内容或应用名…")
        });
        let custom_prompt_input = cx.new(|cx| {
            let mut st = gpui_kit::component::input::InputState::new(window, cx)
                .placeholder("例如：尽量精炼简短，遇到多项内容使用列表排版…");
            if !initial_prompt.is_empty() {
                st.set_value(&initial_prompt, window, cx);
            }
            st
        });
        cx.new(|cx| {
            SettingsView::new(
                session.clone(),
                key_input,
                phrase_input,
                studio_input,
                history_search_input,
                custom_prompt_input,
                cx,
            )
        })
    })
    .expect("主工作台窗口没有打开");
    overlay::prepare_settings_window();
    window
}

fn open_voice_bar(cx: &mut App, session: gpui_kit::Entity<Session>) {
    open_window(voice_options(cx), cx, |window, cx| {
        window.set_window_title("OpenTypeless Voice");
        cx.new(|cx| overlay::OverlayView::new(session.clone(), cx))
    })
    .expect("口述小条没有打开");
}

fn reveal_settings(
    cx: &mut App,
    session: gpui_kit::Entity<Session>,
    current: AnyWindowHandle,
) -> AnyWindowHandle {
    let alive = current
        .update(cx, |_, window, _| {
            window.activate_window();
        })
        .is_ok();
    let handle = if alive {
        current
    } else {
        open_settings(cx, session)
    };
    cx.activate(true);
    handle
}

fn settings_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(920.0), px(650.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("OpenTypeless".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        focus: true,
        show: true,
        is_resizable: true,
        window_min_size: Some(size(px(780.0), px(540.0))),
        ..Default::default()
    }
}

fn voice_options(cx: &App) -> WindowOptions {
    let width = px(580.0);
    let height = px(120.0);
    let origin = cx
        .primary_display()
        .map(|display| {
            let bounds = display.visible_bounds();
            point(
                bounds.origin.x + (bounds.size.width - width) / 2.0,
                bounds.origin.y + bounds.size.height - height - px(16.0),
            )
        })
        .unwrap_or_else(|| point(px(200.0), px(200.0)));
    WindowOptions {
        kind: WindowKind::PopUp,
        focus: false,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: gpui_kit::WindowBackgroundAppearance::Transparent,
        titlebar: Some(TitlebarOptions {
            title: Some("OpenTypeless Voice".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin,
            size: size(width, height),
        })),
        ..Default::default()
    }
}

fn install_tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::TrayIconBuilder;
    use tray_icon::menu::{Menu, MenuItem};
    let menu = Menu::new();
    let show = MenuItem::with_id("show", "打开工作台", true, None);
    let toggle = MenuItem::with_id("toggle", "开始或停止口述", true, None);
    let dock = MenuItem::with_id("dock", "切换 Dock 图标", true, None);
    let quit = MenuItem::with_id("quit", "退出 OpenTypeless", true, None);
    menu.append(&show).ok()?;
    menu.append(&toggle).ok()?;
    menu.append(&dock).ok()?;
    menu.append(&quit).ok()?;
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(status_icon()?)
        .with_icon_as_template(true)
        .with_tooltip("OpenTypeless")
        .build()
        .ok()
}

fn status_icon() -> Option<tray_icon::Icon> {
    let size = 18usize;
    let mut pixels = vec![0u8; size * size * 4];
    let bars = [(2usize, 6usize), (5, 10), (8, 14), (11, 10), (14, 6)];
    for (x, bar_h) in bars {
        let y0 = (size - bar_h) / 2;
        for y in y0..(y0 + bar_h) {
            for dx in 0..2 {
                let index = (y * size + x + dx) * 4;
                pixels[index] = 255;
                pixels[index + 1] = 255;
                pixels[index + 2] = 255;
                pixels[index + 3] = 230;
            }
        }
    }
    tray_icon::Icon::from_rgba(pixels, size as u32, size as u32).ok()
}
