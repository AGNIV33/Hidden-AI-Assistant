use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::providers::{self, Slot};
use crate::{audio, prompts, secrets};

#[derive(Deserialize)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct AskReq {
    pub messages: Vec<Msg>,
    pub mode: String,
    /// Ordered list: first = main, the rest = automatic backups.
    pub slots: Vec<Slot>,
    /// Text from the session's reference files (already trimmed by the page).
    #[serde(default)]
    pub context: String,
    /// If set, the answer is also sent piece by piece as "ai-chunk" events with this id.
    #[serde(default)]
    pub stream_id: Option<String>,
}

#[derive(Serialize)]
pub struct AskResp {
    pub text: String,
    pub provider: String,
    pub model: String,
}

fn client(secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(secs))
        .build()
        .map_err(|e| e.to_string())
}

/// Try each connection in order; move to the next one on any failure.
/// Never waits longer than about 2.5 connections' worth of time in total.
async fn run_chain(
    slots: &[Slot],
    msgs: &[serde_json::Value],
    max_tokens: u32,
    timeout_secs: u64,
    on_delta: Option<&(dyn Fn(&str) + Send + Sync)>,
) -> Result<(String, String, String), String> {
    // Streaming answers may legitimately run longer (the text keeps coming), so allow more time.
    let total = Duration::from_secs(if on_delta.is_some() { 150 } else { timeout_secs * 2 + 10 });
    match tokio::time::timeout(total, run_chain_inner(slots, msgs, max_tokens, timeout_secs, on_delta)).await {
        Ok(r) => r,
        Err(_) => Err(format!("No answer after {} s. Check your internet and the model name in Settings.", total.as_secs())),
    }
}

/// Returns (text, provider name, model).
async fn run_chain_inner(
    slots: &[Slot],
    msgs: &[serde_json::Value],
    max_tokens: u32,
    timeout_secs: u64,
    on_delta: Option<&(dyn Fn(&str) + Send + Sync)>,
) -> Result<(String, String, String), String> {
    if slots.is_empty() {
        return Err("No answer model yet. Open Settings and add one.".into());
    }
    let client = if on_delta.is_some() { client(180)? } else { client(timeout_secs)? };
    let mut errors: Vec<String> = Vec::new();
    for (i, s) in slots.iter().enumerate() {
        let label = format!("#{} {}", i + 1, providers::host(&s.base_url));
        let model = s.model.trim();
        if model.is_empty() {
            errors.push(format!("{label}: no model name"));
            continue;
        }
        let key = match secrets::key_for(s) {
            Ok(k) => k,
            Err(e) => {
                errors.push(format!("{label}: {e}"));
                continue;
            }
        };
        let call = || async {
            match on_delta {
                Some(f) => providers::chat_stream(&client, s, key.as_deref(), msgs, max_tokens, f).await,
                None => providers::chat(&client, s, key.as_deref(), msgs, max_tokens).await,
            }
        };
        let mut result = call().await;
        // "Busy" (503) or "too many requests" (429): wait a moment and try once more before the backup.
        if matches!(&result, Err(e) if e.starts_with("HTTP 503") || e.starts_with("HTTP 429")) {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            result = call().await;
        }
        match result {
            Ok(text) => return Ok((text, providers::host(&s.base_url), model.to_string())),
            Err(e) => errors.push(format!("{label}: {e}")),
        }
    }
    Err(errors.join("\n"))
}

/// Callback that forwards streamed text to the page.
fn streamer(app: &AppHandle, id: Option<String>) -> Option<impl Fn(&str) + Send + Sync> {
    let app = app.clone();
    id.map(move |id| move |t: &str| {
        let _ = app.emit("ai-chunk", json!({ "id": id, "text": t }));
    })
}

#[tauri::command]
pub async fn ask(app: AppHandle, req: AskReq) -> Result<AskResp, String> {
    let (system, max_tokens) = prompts::for_mode(&req.mode);
    let system = prompts::with_context(system, &req.context);
    let mut msgs = vec![json!({"role": "system", "content": system})];
    for m in req.messages.iter().rev().take(10).rev() {
        if m.role == "user" || m.role == "assistant" {
            msgs.push(json!({"role": m.role, "content": m.content}));
        }
    }
    let f = streamer(&app, req.stream_id.clone());
    let (text, provider, model) =
        run_chain(&req.slots, &msgs, max_tokens, 20, f.as_ref().map(|f| f as &(dyn Fn(&str) + Send + Sync))).await?;
    Ok(AskResp { text, provider, model })
}

#[derive(Deserialize)]
pub struct DetectReq {
    pub transcript: String,
    pub mode: String,
    pub slots: Vec<Slot>,
    pub language: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub english: bool,
}

#[derive(Serialize)]
pub struct DetectResp {
    pub question: Option<String>,
    pub answer: Option<String>,
    pub provider: String,
    pub model: String,
}

/// Listening mode: look at the live transcript, find a question for the user, and answer it.
#[tauri::command]
pub async fn detect_answer(req: DetectReq) -> Result<DetectResp, String> {
    let (system, max_tokens) = prompts::for_detect(&req.mode, &req.language, req.english);
    let system = prompts::with_context(system, &req.context);
    let msgs = vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": format!("Transcript:\n{}", req.transcript)}),
    ];
    let (text, provider, model) = run_chain(&req.slots, &msgs, max_tokens, 20, None).await?;
    let (question, answer) = parse_detect(&text);
    Ok(DetectResp { question, answer, provider, model })
}

#[derive(Deserialize)]
pub struct TranslateReq {
    pub text: String,
    pub slots: Vec<Slot>,
}

/// Turn what was heard (Bengali, Hindi, mixed...) into natural English, using the Answers models.
#[tauri::command]
pub async fn translate_text(req: TranslateReq) -> Result<String, String> {
    let text: String = req.text.chars().take(6000).collect();
    let msgs = vec![
        json!({"role": "system", "content": prompts::TRANSLATE}),
        json!({"role": "user", "content": text}),
    ];
    let (out, _, _) = run_chain(&req.slots, &msgs, 1200, 30, None).await?;
    Ok(out.trim().to_string())
}

/// Reads "QUESTION: ... ANSWER: ..." or "NONE". Anything else counts as "no question".
fn parse_detect(text: &str) -> (Option<String>, Option<String>) {
    let t = text.trim();
    let up = t.to_ascii_uppercase(); // ASCII only, so byte positions stay the same
    if t.is_empty() || up.starts_with("NONE") {
        return (None, None);
    }
    let clean = |s: &str| s.trim_matches(|c: char| c == '*' || c.is_whitespace()).to_string();
    if let (Some(q), Some(a)) = (up.find("QUESTION:"), up.find("ANSWER:")) {
        if q < a {
            let (question, answer) = (clean(&t[q + 9..a]), clean(&t[a + 7..]));
            if !question.is_empty() && !answer.is_empty() {
                return (Some(question), Some(answer));
            }
        }
    }
    (None, None)
}

#[derive(Deserialize)]
pub struct ImageReq {
    /// JPEG as base64 (no "data:" prefix)
    pub image: String,
    /// What the user typed about the image (may be empty)
    pub instruction: String,
    pub mode: String,
    pub slots: Vec<Slot>,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub stream_id: Option<String>,
}

/// Screenshot / pasted image: a vision model reads it and answers.
#[tauri::command]
pub async fn ask_image(app: AppHandle, req: ImageReq) -> Result<AskResp, String> {
    if req.image.len() > 6_000_000 {
        return Err("The image is too large".into());
    }
    let (system, max_tokens) = prompts::for_image(&req.mode);
    let system = prompts::with_context(system, &req.context);
    let instruction = match req.instruction.trim() {
        "" => "Answer what this screenshot is asking.",
        t => t,
    };
    let msgs_for = |image: &str| {
        vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": [
                {"type": "text", "text": instruction},
                {"type": "image_url", "image_url": {"url": format!("data:image/jpeg;base64,{image}")}}
            ]}),
        ]
    };
    // NVIDIA only accepts small inline images (about 180 KB), so it gets a smaller copy.
    let small = if req.slots.iter().any(|s| providers::host(&s.base_url).contains("nvidia")) {
        let img = req.image.clone();
        tauri::async_runtime::spawn_blocking(move || crate::files::shrink_b64(&img, 175_000)).await.unwrap_or_else(|_| req.image.clone())
    } else {
        req.image.clone()
    };
    let f = streamer(&app, req.stream_id.clone());
    let f = f.as_ref().map(|f| f as &(dyn Fn(&str) + Send + Sync));
    // Try each vision model in order; each one gets the image size it can take.
    let deadline = std::time::Instant::now() + Duration::from_secs(150);
    let mut errors = Vec::new();
    for (i, s) in req.slots.iter().enumerate() {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        let image = if providers::host(&s.base_url).contains("nvidia") { &small } else { &req.image };
        let one = std::slice::from_ref(s);
        match tokio::time::timeout(left, run_chain_inner(one, &msgs_for(image), max_tokens, 60, f)).await {
            Ok(Ok((text, provider, model))) => return Ok(AskResp { text, provider, model }),
            Ok(Err(e)) => errors.push(e.replacen("#1", &format!("#{}", i + 1), 1)),
            Err(_) => errors.push(format!("#{} {}: timed out", i + 1, providers::host(&s.base_url))),
        }
    }
    Err(if errors.is_empty() { "No answer in time".into() } else { errors.join("; ") })
}

#[tauri::command(async)]
pub fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command(async)]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    (if on { launcher.enable() } else { launcher.disable() }).map_err(|e| e.to_string())
}

/// Load the model names a provider offers (also tests the URL + key).
#[tauri::command]
pub async fn list_models(slot: Slot) -> Result<Vec<String>, String> {
    let key = secrets::key_for(&slot)?;
    providers::list_models(&client(15)?, &slot, key.as_deref()).await
}

/// Start hearing the meeting (system audio, plus the mic if asked).
#[tauri::command]
pub async fn start_listening(
    app: AppHandle,
    state: tauri::State<'_, audio::ListenState>,
    stt: Vec<Slot>,
    mic: bool,
    spoken: Option<String>,
    whole: Option<bool>,
) -> Result<(), String> {
    if stt.is_empty() {
        return Err("No Speech-to-text model yet. Open Settings and add one.".into());
    }
    let stop = tauri::async_runtime::spawn_blocking(move || audio::start(app, stt, mic, spoken.unwrap_or_else(|| "auto3".into()), whole.unwrap_or(false)))
        .await
        .map_err(|e| e.to_string())??;
    let mut cur = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = cur.replace(stop) {
        old.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn stop_listening(state: tauri::State<'_, audio::ListenState>) {
    let mut cur = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(stop) = cur.take() {
        stop.store(true, Ordering::Relaxed);
    }
}

/// True = our windows are hidden from screen sharing / recording. Always starts true.
static PROTECTED: AtomicBool = AtomicBool::new(true);

/// Turn "hidden from screen share" on or off for every window, and tell the pages.
pub fn apply_protection(app: &AppHandle, on: bool) {
    PROTECTED.store(on, Ordering::Relaxed);
    for label in ["bubble", "panel", "snip"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_content_protected(on);
        }
    }
    let _ = app.emit("protection-changed", on);
}

pub fn toggle_protection(app: &AppHandle) {
    apply_protection(app, !PROTECTED.load(Ordering::Relaxed));
}

#[tauri::command]
pub fn set_protection(app: AppHandle, on: bool) {
    apply_protection(&app, on);
}

#[tauri::command]
pub fn get_protection() -> bool {
    PROTECTED.load(Ordering::Relaxed)
}

/// False after the user hides the bubble with "–" / tray / hotkey.
static BUBBLE_ON: AtomicBool = AtomicBool::new(true);

/// Move a window so a (w x h) box at (x, y) stays inside the monitor.
fn clamp_xy(win: &tauri::WebviewWindow, x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    if let Ok(Some(m)) = win.current_monitor() {
        let (mp, ms) = (m.position(), m.size());
        let max_x = mp.x + ms.width as i32 - w;
        let max_y = mp.y + ms.height as i32 - h;
        return (x.min(max_x).max(mp.x), y.min(max_y).max(mp.y));
    }
    (x, y)
}

/// Bubble -> window: the bubble disappears and the window opens where it was.
pub fn open_panel(app: &AppHandle) {
    let (Some(b), Some(p)) = (app.get_webview_window("bubble"), app.get_webview_window("panel"))
    else {
        return;
    };
    if let (Ok(bp), Ok(ps)) = (b.outer_position(), p.outer_size()) {
        let (x, y) = clamp_xy(&b, bp.x, bp.y, ps.width as i32, ps.height as i32);
        let _ = p.set_position(PhysicalPosition::new(x, y));
    }
    let _ = b.hide();
    let _ = p.show();
    let _ = p.set_focus();
}

/// Window -> bubble: the window closes and the bubble comes back where the window was.
pub fn close_panel(app: &AppHandle) {
    let (Some(b), Some(p)) = (app.get_webview_window("bubble"), app.get_webview_window("panel"))
    else {
        return;
    };
    if let (Ok(pp), Ok(bs)) = (p.outer_position(), b.outer_size()) {
        let (x, y) = clamp_xy(&p, pp.x, pp.y, bs.width as i32, bs.height as i32);
        let _ = b.set_position(PhysicalPosition::new(x, y));
    }
    let _ = p.hide();
    if BUBBLE_ON.load(Ordering::Relaxed) {
        let _ = b.show();
    }
}

pub fn toggle(app: &AppHandle) {
    let open = app
        .get_webview_window("panel")
        .map(|p| p.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if open { close_panel(app) } else { open_panel(app) }
}

/// Hide / show the bubble (tray menu, bubble hotkey). Only matters while the window is closed.
pub fn toggle_bubble(app: &AppHandle) {
    let on = !BUBBLE_ON.load(Ordering::Relaxed);
    BUBBLE_ON.store(on, Ordering::Relaxed);
    let panel_open = app
        .get_webview_window("panel")
        .map(|p| p.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if let Some(b) = app.get_webview_window("bubble") {
        if on && !panel_open { let _ = b.show(); } else if !on { let _ = b.hide(); }
    }
}

/// App started a second time: just open the window.
pub fn show_all(app: &AppHandle) {
    BUBBLE_ON.store(true, Ordering::Relaxed);
    open_panel(app);
}

#[tauri::command]
pub fn toggle_panel(app: AppHandle) {
    toggle(&app);
}

#[tauri::command]
pub fn hide_bubble(app: AppHandle) {
    BUBBLE_ON.store(false, Ordering::Relaxed);
    if let Some(b) = app.get_webview_window("bubble") {
        let _ = b.hide();
    }
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    close_panel(&app);
}

#[tauri::command(async)]
pub fn save_key(id: String, key: String, base_url: Option<String>) -> Result<(), String> {
    secrets::set(&id, &key)?;
    // Also remember it for the provider, so new backup slots can use it without pasting again.
    if let Some(url) = base_url.filter(|u| !u.trim().is_empty()) {
        let _ = secrets::set(&secrets::host_id(&url), &key);
    }
    Ok(())
}

#[tauri::command(async)]
pub fn delete_key(id: String) -> Result<(), String> {
    secrets::delete(&id)
}

#[tauri::command(async)]
pub fn key_status(slots: Vec<Slot>) -> HashMap<String, bool> {
    slots
        .iter()
        .map(|s| {
            let has = secrets::get(&s.id).or_else(|| secrets::get(&secrets::host_id(&s.base_url))).is_some();
            (s.id.clone(), has)
        })
        .collect()
}

/// Copy keys saved before key sharing existed to their provider, once at start-up.
#[tauri::command(async)]
pub fn share_keys(slots: Vec<Slot>) {
    for s in &slots {
        let host = secrets::host_id(&s.base_url);
        if secrets::get(&host).is_none() {
            if let Some(k) = secrets::get(&s.id) {
                let _ = secrets::set(&host, &k);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_detect;

    #[test]
    fn detect_reply_formats() {
        assert_eq!(parse_detect("NONE"), (None, None));
        assert_eq!(parse_detect("none."), (None, None));
        assert_eq!(parse_detect("There is no question here."), (None, None));
        let (q, a) = parse_detect("QUESTION: What is your notice period?\nANSWER: It is one month.");
        assert_eq!(q.as_deref(), Some("What is your notice period?"));
        assert_eq!(a.as_deref(), Some("It is one month."));
        let (q, a) = parse_detect("**QUESTION:** Why this role?\n**ANSWER:** Because it fits my skills.");
        assert_eq!(q.as_deref(), Some("Why this role?"));
        assert_eq!(a.as_deref(), Some("Because it fits my skills."));
        // Non-English text must not break byte positions.
        let (q, a) = parse_detect("QUESTION: आपका नाम क्या है?\nANSWER: मेरा नाम अग्निव है।");
        assert_eq!(q.as_deref(), Some("आपका नाम क्या है?"));
        assert!(a.unwrap().contains("अग्निव"));
    }
}
