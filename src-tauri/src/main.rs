#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod files;
mod hotkeys;
mod localwhisper;
mod prompts;
mod providers;
mod secrets;
mod store;

use std::collections::HashMap;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    LogicalSize, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "panel", "Open Peek AI", true, None::<&str>)?;
    let bubble = MenuItem::with_id(app, "bubble", "Show / hide bubble", true, None::<&str>)?;
    let protect = MenuItem::with_id(app, "protect", "Hide / show in screen share", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &bubble, &protect, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .tooltip("Peek AI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "panel" => commands::toggle(app),
            "bubble" => commands::toggle_bubble(app),
            "protect" => commands::toggle_protection(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                commands::toggle(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(audio::ListenState::default())
        .manage(hotkeys::Hotkeys::default())
        // Starting the app a 2nd time just opens the one that is already running.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            commands::show_all(app);
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(
            ShortcutBuilder::new()
                .with_handler(|app, sc, ev| {
                    if ev.state() != ShortcutState::Pressed {
                        return;
                    }
                    let action = {
                        let state = app.state::<hotkeys::Hotkeys>();
                        let list = state.0.lock().unwrap_or_else(|e| e.into_inner());
                        list.iter().find(|(s, _)| s == sc).map(|(_, a)| a.clone())
                    };
                    if let Some(a) = action {
                        hotkeys::dispatch(app, &a);
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| match event {
            // Closing a window (e.g. Alt+F4) only hides it. Quit is in the tray menu.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.label() == "panel" {
                    commands::close_panel(window.app_handle());
                } else {
                    let _ = window.hide();
                }
            }
            WindowEvent::Moved(p) if window.label() == "bubble" => store::note_bubble(p.x, p.y),
            WindowEvent::Resized(s) if window.label() == "panel" => store::note_panel(s.width, s.height),
            _ => {}
        })
        .setup(|app| {
            // Every window: hidden from screen capture, not in the taskbar.
            for label in ["bubble", "panel", "snip"] {
                if let Some(w) = app.get_webview_window(label) {
                    let _ = w.set_content_protected(true);
                    let _ = w.set_skip_taskbar(true);
                    let _ = w.set_always_on_top(true);
                }
            }
            // Keep the bubble an exact square so it stays a circle.
            if let Some(b) = app.get_webview_window("bubble") {
                let _ = b.set_min_size(Some(LogicalSize::new(32.0, 32.0)));
                let _ = b.set_size(LogicalSize::new(64.0, 64.0));
            }
            store::restore(app.handle());
            store::start_saver(app.handle().clone());
            // Default hotkeys now; the panel re-applies the user's own ones when it starts.
            let _ = hotkeys::apply(app.handle(), &HashMap::new());
            build_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ask,
            commands::ask_image,
            commands::detect_answer,
            commands::translate_text,
            commands::list_models,
            commands::toggle_panel,
            commands::hide_panel,
            commands::hide_bubble,
            commands::save_key,
            commands::delete_key,
            commands::key_status,
            commands::share_keys,
            commands::start_listening,
            commands::stop_listening,
            commands::get_autostart,
            commands::set_autostart,
            commands::set_protection,
            commands::get_protection,
            hotkeys::set_hotkeys,
            hotkeys::pause_hotkeys,
            files::take_screenshot,
            files::start_snip,
            files::cancel_snip,
            files::finish_snip,
            files::load_bytes,
            files::load_path,
            localwhisper::ensure_local_whisper,
            localwhisper::stop_local_whisper,
            store::load_state,
            store::save_state,
        ])
        .build(tauri::generate_context!())
        .expect("error while starting app")
        .run(|_app, event| {
            // Quitting: also stop the hidden Local Whisper server we may have started.
            if let tauri::RunEvent::Exit = event {
                localwhisper::stop();
            }
        });
}
