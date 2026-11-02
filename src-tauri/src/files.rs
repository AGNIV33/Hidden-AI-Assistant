//! Reading files the user adds (PDF, DOCX, text, images) and taking screenshots.

use std::io::{Cursor, Read};
use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage};
use serde::Serialize;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

const MAX_BYTES: usize = 12 * 1024 * 1024; // biggest file we accept
const MAX_TEXT: usize = 200_000; // most text we keep per file

#[derive(Serialize)]
pub struct Loaded {
    pub name: String,
    /// "text" or "image"
    pub kind: String,
    pub text: String,
    /// JPEG as base64 (only for images)
    pub image: String,
}

pub fn extract(name: &str, bytes: &[u8]) -> Result<Loaded, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("{name} is too big (max 12 MB)"));
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "txt" | "md" | "markdown" | "csv" | "json" | "log" | "xml" | "html" | "htm" | "yaml" | "yml" => {
            text_file(name, String::from_utf8_lossy(bytes).into_owned())
        }
        "pdf" => {
            let data = bytes.to_vec();
            // A broken PDF must never crash the app.
            match std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&data)) {
                Ok(Ok(t)) => text_file(name, t),
                Ok(Err(e)) => Err(format!("Could not read {name}: {e}")),
                Err(_) => Err(format!("Could not read {name} (unsupported PDF)")),
            }
        }
        "docx" => docx_text(bytes).and_then(|t| text_file(name, t)),
        "png" | "jpg" | "jpeg" | "webp" | "gif" => {
            let img = image::load_from_memory(bytes).map_err(|_| format!("Could not read image {name}"))?;
            Ok(Loaded { name: name.into(), kind: "image".into(), text: String::new(), image: jpeg_b64(img, 1280)? })
        }
        _ => Err(format!("{name}: not supported. Use PDF, DOCX, TXT, MD, CSV, JSON or an image.")),
    }
}

fn text_file(name: &str, raw: String) -> Result<Loaded, String> {
    let cleaned = clean(&raw);
    if cleaned.trim().is_empty() {
        return Err(format!("{name}: no readable text found (a scanned PDF has no text)"));
    }
    Ok(Loaded {
        name: name.into(),
        kind: "text".into(),
        text: cleaned.chars().take(MAX_TEXT).collect(),
        image: String::new(),
    })
}

/// Trim line ends and squeeze runs of blank lines.
fn clean(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut blank = 0;
    for line in t.replace('\r', "").lines() {
        let l = line.trim_end();
        if l.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn docx_text(bytes: &[u8]) -> Result<String, String> {
    let bad = || "Not a valid .docx file".to_string();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| bad())?;
    let file = zip.by_name("word/document.xml").map_err(|_| bad())?;
    let mut xml = String::new();
    file.take(40_000_000).read_to_string(&mut xml).map_err(|_| bad())?;
    Ok(xml_to_text(&xml))
}

/// Pull the visible text out of Word's XML: text inside <w:t>, new line after each <w:p>.
fn xml_to_text(xml: &str) -> String {
    let mut out = String::new();
    let mut in_text = false;
    let mut i = 0;
    while i < xml.len() {
        if xml.as_bytes()[i] == b'<' {
            let Some(rel) = xml[i..].find('>') else { break };
            let end = i + rel;
            let tag = &xml[i + 1..end];
            let closing = tag.starts_with('/');
            let self_closing = tag.ends_with('/');
            let name = tag
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("");
            match (closing, name) {
                (false, "w:t") if !self_closing => in_text = true,
                (true, "w:t") => in_text = false,
                (true, "w:p") => out.push('\n'),
                (false, "w:tab") => out.push('\t'),
                (false, "w:br") | (false, "w:cr") => out.push('\n'),
                _ => {}
            }
            i = end + 1;
        } else {
            let next = xml[i..].find('<').map(|n| i + n).unwrap_or(xml.len());
            if in_text {
                out.push_str(&decode(&xml[i..next]));
            }
            i = next;
        }
    }
    out
}

fn decode(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Shrink to at most `max_w` wide and save as JPEG (small enough to send to a vision model).
pub fn jpeg_b64(img: DynamicImage, max_w: u32) -> Result<String, String> {
    let img = if img.width() > max_w {
        let h = ((img.height() as f64 * max_w as f64) / img.width() as f64).round().max(1.0) as u32;
        img.resize_exact(max_w, h, FilterType::Triangle)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, 75).encode_image(&rgb).map_err(|e| e.to_string())?;
    Ok(STANDARD.encode(buf))
}

/// Make a JPEG (base64) smaller until it fits `max_len` characters. Text stays readable down to ~800 px.
pub fn shrink_b64(b64: &str, max_len: usize) -> String {
    if b64.len() <= max_len {
        return b64.to_string();
    }
    let Ok(bytes) = STANDARD.decode(b64) else { return b64.to_string() };
    let Ok(img) = image::load_from_memory(&bytes) else { return b64.to_string() };
    let mut best = b64.to_string();
    for (w, q) in [(1280, 65), (1100, 60), (960, 60), (800, 55), (640, 50)] {
        let small = if img.width() > w {
            let h = ((img.height() as f64 * w as f64) / img.width() as f64).round().max(1.0) as u32;
            img.resize_exact(w, h, FilterType::Triangle)
        } else {
            img.clone()
        };
        let mut buf = Vec::new();
        if JpegEncoder::new_with_quality(&mut buf, q).encode_image(&small.to_rgb8()).is_err() {
            continue;
        }
        best = STANDARD.encode(buf);
        if best.len() <= max_len {
            break;
        }
    }
    best
}

/// Screenshot of the screen the mouse is on. Our own windows are excluded from capture.
pub fn capture_b64(app: &AppHandle) -> Result<String, String> {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| xcap::Monitor::from_point(p.x as i32, p.y as i32).ok())
        .or_else(|| {
            xcap::Monitor::all().ok().and_then(|v| v.into_iter().find(|m| m.is_primary().unwrap_or(false)))
        })
        .ok_or_else(|| "No screen found".to_string())?;
    let shot = monitor.capture_image().map_err(|e| e.to_string())?;
    jpeg_b64(DynamicImage::ImageRgba8(shot), 1280)
}

/// Hotkey version: capture in the background, then tell the panel.
pub fn screenshot_async(app: AppHandle) {
    std::thread::spawn(move || match capture_b64(&app) {
        Ok(b64) => {
            let _ = app.emit("screenshot", b64);
        }
        Err(e) => {
            let _ = app.emit("app-error", format!("Screenshot failed: {e}"));
        }
    });
}

/* ---------- select an area (like the Windows snipping tool) ---------- */

/// The screen the overlay is covering: (x, y, width, height) in real pixels.
static SNIP: Mutex<Option<(i32, i32, u32, u32)>> = Mutex::new(None);

/// Cover the screen under the mouse with the see-through selection overlay.
pub fn open_snip(app: &AppHandle) -> Result<(), String> {
    let w = app.get_webview_window("snip").ok_or("Selection window missing")?;
    let cur = app.cursor_position().map_err(|e| e.to_string())?;
    let mons = app.available_monitors().map_err(|e| e.to_string())?;
    let m = mons
        .iter()
        .find(|m| {
            let (p, s) = (m.position(), m.size());
            cur.x >= p.x as f64 && cur.y >= p.y as f64 && cur.x < (p.x + s.width as i32) as f64 && cur.y < (p.y + s.height as i32) as f64
        })
        .or(mons.first())
        .ok_or("No screen found")?;
    let (p, s) = (*m.position(), *m.size());
    *SNIP.lock().unwrap_or_else(|e| e.into_inner()) = Some((p.x, p.y, s.width, s.height));
    let _ = w.emit("snip-start", ());
    w.set_position(p).map_err(|e| e.to_string())?;
    w.set_size(s).map_err(|e| e.to_string())?;
    w.show().map_err(|e| e.to_string())?;
    let _ = w.set_focus();
    Ok(())
}

pub fn open_snip_async(app: AppHandle) {
    if let Err(e) = open_snip(&app) {
        let _ = app.emit("app-error", format!("Select area failed: {e}"));
    }
}

#[tauri::command]
pub async fn start_snip(app: AppHandle) -> Result<(), String> {
    open_snip(&app)
}

#[tauri::command]
pub fn cancel_snip(app: AppHandle) {
    SNIP.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(w) = app.get_webview_window("snip") {
        let _ = w.hide();
    }
    focus_panel(&app);
}

/// Give the keyboard back to the Peek AI window so you can type at once.
fn focus_panel(app: &AppHandle) {
    if let Some(p) = app.get_webview_window("panel") {
        if p.is_visible().unwrap_or(false) {
            let _ = p.set_focus();
        }
    }
}

/// The user finished dragging: grab that part of the screen and send it to the panel.
#[tauri::command]
pub async fn finish_snip(app: AppHandle, x: f64, y: f64, w: f64, h: f64) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("snip") {
        let _ = win.hide();
    }
    let Some(rect) = SNIP.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return Err("No area selected".into());
    };
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || crop_screen(rect, x, y, w, h)).await.map_err(|e| e.to_string())?;
    match result {
        Ok(b64) => {
            let _ = app2.emit("screenshot", b64);
            focus_panel(&app2);
        }
        Err(e) => {
            let _ = app2.emit("app-error", format!("Select area failed: {e}"));
        }
    }
    Ok(())
}

fn crop_screen(rect: (i32, i32, u32, u32), x: f64, y: f64, w: f64, h: f64) -> Result<String, String> {
    let (mx, my, mw, mh) = rect;
    let monitor = xcap::Monitor::from_point(mx + (mw / 2) as i32, my + (mh / 2) as i32).map_err(|e| e.to_string())?;
    let shot = monitor.capture_image().map_err(|e| e.to_string())?;
    // The captured image can differ in size from the screen numbers (scaling), so convert.
    let fx = shot.width() as f64 / mw.max(1) as f64;
    let fy = shot.height() as f64 / mh.max(1) as f64;
    let cx = ((x * fx).max(0.0) as u32).min(shot.width().saturating_sub(1));
    let cy = ((y * fy).max(0.0) as u32).min(shot.height().saturating_sub(1));
    let cw = ((w * fx) as u32).clamp(1, shot.width() - cx);
    let ch = ((h * fy) as u32).clamp(1, shot.height() - cy);
    let part = image::imageops::crop_imm(&shot, cx, cy, cw, ch).to_image();
    jpeg_b64(DynamicImage::ImageRgba8(part), 1600)
}

#[tauri::command]
pub async fn take_screenshot(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || capture_b64(&app)).await.map_err(|e| e.to_string())?
}

/// File picked with the 📎 button (the page sends the bytes as base64).
#[tauri::command]
pub async fn load_bytes(name: String, data: String) -> Result<Loaded, String> {
    let bytes = STANDARD.decode(data.as_bytes()).map_err(|_| "Could not read the file".to_string())?;
    tauri::async_runtime::spawn_blocking(move || extract(&name, &bytes)).await.map_err(|e| e.to_string())?
}

/// File dropped onto the window (we get its path).
#[tauri::command]
pub async fn load_path(path: String) -> Result<Loaded, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
        let meta = std::fs::metadata(p).map_err(|_| format!("Could not open {name}"))?;
        if !meta.is_file() {
            return Err(format!("{name} is not a file"));
        }
        if meta.len() as usize > MAX_BYTES {
            return Err(format!("{name} is too big (max 12 MB)"));
        }
        let bytes = std::fs::read(p).map_err(|_| format!("Could not read {name}"))?;
        extract(&name, &bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn docx(xml: &str) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        w.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(xml.as_bytes()).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn big_images_are_shrunk_for_nvidia() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(1920, 1080, |x, y| image::Rgb([(x * 7 % 255) as u8, (y * 13 % 255) as u8, ((x ^ y) % 255) as u8])));
        let big = jpeg_b64(img, 1920).unwrap();
        assert!(big.len() > 180_000, "test image should start big ({})", big.len());
        let small = shrink_b64(&big, 180_000);
        assert!(small.len() < big.len());
        assert_eq!(shrink_b64("abc", 180_000), "abc");
    }

    #[test]
    fn reads_docx_text() {
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>Hello &amp; welcome</w:t></w:r></w:p>
            <w:p><w:r><w:t xml:space="preserve">Second </w:t></w:r><w:r><w:t>line</w:t></w:r></w:p>
            <w:p><w:pPr/><w:r><w:tab/><w:t>नमस्ते</w:t></w:r></w:p>
            </w:body></w:document>"#;
        let l = extract("cv.docx", &docx(xml)).unwrap();
        assert_eq!(l.kind, "text");
        assert!(l.text.contains("Hello & welcome"));
        assert!(l.text.contains("Second line"));
        assert!(l.text.contains("नमस्ते"));
    }

    #[test]
    fn text_files_and_errors() {
        let l = extract("notes.md", b"# Title\r\n\r\n\r\n\r\nBody  \r\n").unwrap();
        assert_eq!(l.text, "# Title\n\nBody\n");
        assert!(extract("x.exe", b"MZ").is_err());
        assert!(extract("empty.txt", b"   \n").is_err());
        assert!(extract("bad.docx", b"not a zip").is_err());
        assert!(extract("bad.pdf", b"not a pdf").is_err());
    }

    #[test]
    fn images_become_small_jpegs() {
        let img = DynamicImage::new_rgb8(3000, 1500);
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let l = extract("shot.png", &png).unwrap();
        assert_eq!(l.kind, "image");
        let jpeg = STANDARD.decode(l.image).unwrap();
        let back = image::load_from_memory(&jpeg).unwrap();
        assert_eq!(back.width(), 1280);
        assert_eq!(back.height(), 640);
    }
}
