//! Saving to disk: chat sessions (sessions.json) and window position/size (window.json),
//! both inside the app's data folder (%APPDATA%\com.agniv.hiddenai).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Write to a temp file, then swap it in, so a crash never leaves half a file.
fn write_atomic(path: &Path, data: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

#[tauri::command(async)]
pub fn load_state(app: AppHandle) -> Result<String, String> {
    match fs::read_to_string(dir(&app)?.join("sessions.json")) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command(async)]
pub fn save_state(app: AppHandle, json: String) -> Result<(), String> {
    if json.len() > 60_000_000 {
        return Err("Saved chats are too large".into());
    }
    write_atomic(&dir(&app)?.join("sessions.json"), &json).map_err(|e| e.to_string())
}

/* ---------- window position and size ---------- */

#[derive(Default, Clone, Serialize, Deserialize)]
struct Geometry {
    bubble: Option<(i32, i32)>,
    panel: Option<(u32, u32)>,
}

static GEO: Mutex<Geometry> = Mutex::new(Geometry { bubble: None, panel: None });
static DIRTY: Mutex<Option<Instant>> = Mutex::new(None);

fn touch() {
    *DIRTY.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

pub fn note_bubble(x: i32, y: i32) {
    if x < -10_000 || y < -10_000 {
        return; // Windows parks hidden/minimized windows far off-screen
    }
    GEO.lock().unwrap_or_else(|e| e.into_inner()).bubble = Some((x, y));
    touch();
}

pub fn note_panel(w: u32, h: u32) {
    if w < 200 || h < 200 {
        return;
    }
    GEO.lock().unwrap_or_else(|e| e.into_inner()).panel = Some((w, h));
    touch();
}

/// Background thread: writes window.json shortly after the user stops moving/resizing.
pub fn start_saver(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(700));
        let due = {
            let mut d = DIRTY.lock().unwrap_or_else(|e| e.into_inner());
            match *d {
                Some(t) if t.elapsed() > Duration::from_millis(500) => {
                    *d = None;
                    true
                }
                _ => false,
            }
        };
        if due {
            let g = GEO.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let (Ok(d), Ok(json)) = (dir(&app), serde_json::to_string(&g)) {
                let _ = write_atomic(&d.join("window.json"), &json);
            }
        }
    });
}

/// Put the bubble and panel back where the user left them (only if still on a screen).
pub fn restore(app: &AppHandle) {
    let Ok(d) = dir(app) else { return };
    let Ok(text) = fs::read_to_string(d.join("window.json")) else { return };
    let Ok(g) = serde_json::from_str::<Geometry>(&text) else { return };
    *GEO.lock().unwrap_or_else(|e| e.into_inner()) = g.clone();

    if let (Some((x, y)), Some(b)) = (g.bubble, app.get_webview_window("bubble")) {
        let on_screen = app.available_monitors().unwrap_or_default().iter().any(|m| {
            let (p, s) = (m.position(), m.size());
            x >= p.x && y >= p.y && x + 20 <= p.x + s.width as i32 && y + 20 <= p.y + s.height as i32
        });
        if on_screen {
            let _ = b.set_position(PhysicalPosition::new(x, y));
        }
    }
    if let (Some((w, h)), Some(p)) = (g.panel, app.get_webview_window("panel")) {
        let fits = app.available_monitors().unwrap_or_default().iter().any(|m| {
            let s = m.size();
            w <= s.width && h <= s.height
        });
        if fits {
            let _ = p.set_size(PhysicalSize::new(w, h));
        }
    }
}
