//! Global hotkeys the user can change in Settings.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut};

use crate::{commands, files};

// Ctrl+Alt combos: Teams (Ctrl+Shift+M mute, Ctrl+Shift+H hang up...) and Zoom (Alt+...) do not use these.
pub const DEFAULTS: [(&str, &str); 8] = [
    ("toggle", "Ctrl+Alt+Space"),
    ("screenshot", "Ctrl+Alt+S"),
    ("snip", "Ctrl+Alt+X"),
    ("listen", "Ctrl+Alt+L"),
    ("mode", "Ctrl+Alt+M"),
    ("bubble", "Ctrl+Alt+B"),
    ("protect", "Ctrl+Alt+H"),
    ("quit", "Ctrl+Alt+Q"),
];

/// The hotkeys that are active right now: (key combo, action name).
#[derive(Default)]
pub struct Hotkeys(pub Mutex<Vec<(Shortcut, String)>>);

pub fn parse(s: &str) -> Result<Shortcut, String> {
    let sc = Shortcut::from_str(s).map_err(|e| format!("'{s}' is not a valid key combination ({e})"))?;
    if !(sc.mods.contains(Modifiers::CONTROL) || sc.mods.contains(Modifiers::ALT)) {
        return Err(format!("'{s}': include Ctrl or Alt so normal typing is not blocked"));
    }
    Ok(sc)
}

/// Replace all hotkeys. Empty/missing entries use the default. Returns {action: error} for any that failed.
pub fn apply(app: &AppHandle, wanted: &HashMap<String, String>) -> HashMap<String, String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut errors = HashMap::new();
    let mut active: Vec<(Shortcut, String)> = Vec::new();
    for (action, default) in DEFAULTS {
        let combo = wanted.get(action).map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or(default);
        match parse(combo) {
            Ok(sc) => {
                if active.iter().any(|(s, _)| s == &sc) {
                    errors.insert(action.to_string(), format!("{combo} is already used by another action"));
                    continue;
                }
                match gs.register(sc.clone()) {
                    Ok(()) => active.push((sc, action.to_string())),
                    Err(_) => {
                        errors.insert(action.to_string(), format!("{combo} is taken by another app"));
                    }
                }
            }
            Err(e) => {
                errors.insert(action.to_string(), e);
            }
        }
    }
    *app.state::<Hotkeys>().0.lock().unwrap_or_else(|e| e.into_inner()) = active;
    errors
}

pub fn dispatch(app: &AppHandle, action: &str) {
    match action {
        "toggle" => commands::toggle(app),
        "screenshot" => files::screenshot_async(app.clone()),
        "snip" => files::open_snip_async(app.clone()),
        "listen" => {
            let _ = app.emit("toggle-listening", ());
        }
        "mode" => {
            let _ = app.emit("cycle-mode", ());
        }
        "bubble" => commands::toggle_bubble(app),
        "protect" => commands::toggle_protection(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Turn all hotkeys off (used while the user is recording a new key combination).
#[tauri::command]
pub fn pause_hotkeys(app: AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    app.state::<Hotkeys>().0.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[tauri::command]
pub fn set_hotkeys(app: AppHandle, map: HashMap<String, String>) -> HashMap<String, String> {
    apply(&app, &map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_what_the_settings_screen_produces() {
        for ok in ["Ctrl+Shift+Space", "Ctrl+Shift+KeyS", "Ctrl+Alt+F5", "Alt+Digit1", "Ctrl+Shift+ArrowUp", "Ctrl+Backquote"] {
            assert!(parse(ok).is_ok(), "{ok} should parse");
        }
        for bad in ["Space", "Shift+KeyA", "Ctrl+", "Ctrl+Shift+Nope", ""] {
            assert!(parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn defaults_are_valid_and_unique() {
        let mut seen = Vec::new();
        for (_, combo) in DEFAULTS {
            let sc = parse(combo).unwrap_or_else(|e| panic!("{e}"));
            assert!(!seen.contains(&sc));
            seen.push(sc);
        }
    }
}
