//! Starts the Local Whisper server (installed by tools/install-whisper.ps1) hidden in the background,
//! and stops it when the app quits. No black window needed.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
const URL: &str = "http://127.0.0.1:8080/";
const MODEL_ORDER: [&str; 4] = [
    "ggml-large-v3-turbo-q8_0.bin",
    "ggml-large-v3-turbo-q5_0.bin",
    "ggml-large-v3-turbo.bin",
    "ggml-large-v3.bin",
];

fn find(dir: &Path, ok: &dyn Fn(&str) -> bool, depth: u32, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            find(&p, ok, depth - 1, out);
        } else if p.file_name().and_then(|n| n.to_str()).map(ok).unwrap_or(false) {
            out.push(p);
        }
    }
}

/// Best model in the folder (large-v3-turbo q8 first).
pub fn pick_model(dir: &Path) -> Option<PathBuf> {
    let mut models = Vec::new();
    find(dir, &|n| n.starts_with("ggml-") && n.ends_with(".bin"), 2, &mut models);
    for want in MODEL_ORDER {
        if let Some(m) = models.iter().find(|p| p.file_name().map(|n| n == want).unwrap_or(false)) {
            return Some(m.clone());
        }
    }
    models.sort();
    models.into_iter().next()
}

/// whisper-server programs in the folder, graphics-card (GPU) build first.
pub fn servers(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    find(dir, &|n| n.eq_ignore_ascii_case("whisper-server.exe") || n == "whisper-server", 4, &mut v);
    v.sort_by_key(|p| if p.to_string_lossy().contains("bin-gpu") { 0 } else { 1 });
    v
}

async fn alive(client: &reqwest::Client) -> bool {
    client.get(URL).send().await.is_ok()
}

fn spawn(exe: &Path, model: &Path) -> std::io::Result<Child> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(2).clamp(2, 8);
    let mut cmd = Command::new(exe);
    cmd.arg("-m")
        .arg(model)
        .args(["-l", "auto", "-t", &threads.to_string(), "--host", "127.0.0.1", "--port", "8080"])
        .args(["--inference-path", "/v1/audio/transcriptions"])
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no black console window
    }
    cmd.spawn()
}

fn child_exited() -> bool {
    match CHILD.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        Some(c) => c.try_wait().ok().flatten().is_some(),
        None => true,
    }
}

/// Stop the server we started (does nothing if it was started some other way).
pub fn stop() {
    if let Some(mut c) = CHILD.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// Make sure Local Whisper is running. Starts it hidden if needed and waits until it is ready.
#[tauri::command]
pub async fn ensure_local_whisper(dir: String) -> Result<String, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?;
    if alive(&client).await {
        return Ok("Local Whisper is running".into());
    }
    let d = PathBuf::from(dir.trim());
    if !d.is_dir() {
        return Err(format!("Local Whisper folder not found: {}. Run tools\\Install Local Whisper.bat first.", d.display()));
    }
    let model = pick_model(&d).ok_or("No Whisper model (.bin) in that folder. Run the installer again.")?;
    let exes = servers(&d);
    if exes.is_empty() {
        return Err("whisper-server.exe not found in that folder. Run the installer again.".into());
    }
    for exe in exes {
        stop();
        let child = spawn(&exe, &model).map_err(|e| format!("Could not start Local Whisper: {e}"))?;
        *CHILD.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        let gpu = exe.to_string_lossy().contains("bin-gpu");
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(90) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if alive(&client).await {
                return Ok(format!("Local Whisper started ({})", if gpu { "graphics card" } else { "CPU" }));
            }
            if child_exited() {
                break; // e.g. the graphics-card build cannot start: try the CPU build
            }
        }
    }
    stop();
    Err("Local Whisper could not start. Double-click 'Start Local Whisper' on the Desktop to see the error.".into())
}

#[tauri::command]
pub fn stop_local_whisper() {
    stop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_best_model_and_gpu_server_first() {
        let d = std::env::temp_dir().join(format!("lw-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for f in ["ggml-small.bin", "ggml-large-v3-turbo-q8_0.bin", "bin-cpu/Release/whisper-server.exe", "bin-gpu/Release/whisper-server.exe"] {
            let p = d.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        assert!(pick_model(&d).unwrap().ends_with("ggml-large-v3-turbo-q8_0.bin"));
        let s = servers(&d);
        assert_eq!(s.len(), 2);
        assert!(s[0].to_string_lossy().contains("bin-gpu"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
