use serde::Deserialize;
use serde_json::{json, Value};

/// One AI connection chosen by the user: any OpenAI-compatible URL + model.
/// The API key is NOT here. It lives in Windows Credential Manager under `id`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub id: String,
    pub base_url: String,
    pub model: String,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// True only for http://localhost or http://127.0.0.1 (exact host match).
pub fn is_local(url: &str) -> bool {
    let u = url.trim().to_lowercase();
    let Some(rest) = u.strip_prefix("http://") else { return false };
    let host = rest.split(|c| c == '/' || c == ':').next().unwrap_or("");
    host == "localhost" || host == "127.0.0.1"
}

/// Keys are only ever sent over https (or to the user's own machine).
fn check_url(url: &str) -> Result<String, String> {
    let u = url.trim().trim_end_matches('/').to_string();
    if u.to_lowercase().starts_with("https://") || is_local(&u) {
        Ok(u)
    } else {
        Err("Base URL must start with https:// (http:// is only allowed for localhost)".into())
    }
}

/// Short display name for a provider, taken from its URL.
pub fn host(url: &str) -> String {
    let u = url.trim();
    let u = u.split("://").nth(1).unwrap_or(u);
    u.split('/').next().unwrap_or(u).to_string()
}

pub async fn chat(
    client: &reqwest::Client,
    slot: &Slot,
    key: Option<&str>,
    messages: &[Value],
    max_tokens: u32,
) -> Result<String, String> {
    let base = check_url(&slot.base_url)?;
    let mut rb = client.post(format!("{base}/chat/completions")).json(&json!({
        "model": slot.model.trim(),
        "messages": messages,
        "temperature": 0.6,
        "max_tokens": max_tokens
    }));
    if let Some(k) = key {
        rb = rb.bearer_auth(k);
    }
    let resp = rb.send().await.map_err(net_err)?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);

    if !status.is_success() {
        return Err(http_err(status.as_u16(), &body));
    }
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "empty reply".to_string())
}

/// Same as `chat`, but asks for a stream and calls `on_delta` with each new piece of text.
/// Gives up if nothing arrives for 20 s (dead connection), but long answers can take longer in total.
pub async fn chat_stream(
    client: &reqwest::Client,
    slot: &Slot,
    key: Option<&str>,
    messages: &[Value],
    max_tokens: u32,
    on_delta: &(dyn Fn(&str) + Send + Sync),
) -> Result<String, String> {
    use std::time::Duration;
    // Vision and busy free models can need a long time before the first word (they read the
    // image first). After the text starts, a long silence means the connection is stuck.
    let first = Duration::from_secs(75);
    let gap = Duration::from_secs(25);
    let base = check_url(&slot.base_url)?;
    let mut rb = client.post(format!("{base}/chat/completions")).json(&json!({
        "model": slot.model.trim(),
        "messages": messages,
        "temperature": 0.6,
        "max_tokens": max_tokens,
        "stream": true
    }));
    if let Some(k) = key {
        rb = rb.bearer_auth(k);
    }
    let mut resp = match tokio::time::timeout(first, rb.send()).await {
        Ok(r) => r.map_err(net_err)?,
        Err(_) => return Err("timed out".into()),
    };
    let status = resp.status();
    if !status.is_success() {
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        return Err(http_err(status.as_u16(), &body));
    }
    let mut buf: Vec<u8> = Vec::new(); // bytes, so a letter split across two pieces is not broken
    let mut full = String::new();
    loop {
        let wait = if full.is_empty() { first } else { gap };
        let piece = match tokio::time::timeout(wait, resp.chunk()).await {
            Ok(Ok(Some(b))) => b,
            Ok(Ok(None)) => break,
            Ok(Err(e)) => {
                if full.is_empty() { return Err(net_err(e)); }
                break; // keep what we already have
            }
            Err(_) => {
                if full.is_empty() { return Err("timed out".into()); }
                break;
            }
        };
        buf.extend_from_slice(&piece);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(data) = line.trim().strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" || data.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            if v.get("error").is_some() && full.is_empty() {
                return Err(http_err(500, &v));
            }
            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                if !t.is_empty() {
                    full.push_str(t);
                    on_delta(t);
                }
            }
        }
    }
    let text = full.trim().to_string();
    if text.is_empty() { Err("empty reply".into()) } else { Ok(text) }
}

/// Lists model names from GET {base}/models. Also proves the key works.
pub async fn list_models(
    client: &reqwest::Client,
    slot: &Slot,
    key: Option<&str>,
) -> Result<Vec<String>, String> {
    let base = check_url(&slot.base_url)?;
    let mut rb = client.get(format!("{base}/models"));
    if let Some(k) = key {
        rb = rb.bearer_auth(k);
    }
    let resp = rb.send().await.map_err(net_err)?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(http_err(status.as_u16(), &body));
    }
    let mut names: Vec<String> = body["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["id"].as_str())
                .map(|s| s.strip_prefix("models/").unwrap_or(s).to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err("This provider did not return a model list. Type the model name yourself.".into());
    }
    Ok(names)
}

/// POST {base}/audio/transcriptions (OpenAI-style, e.g. Groq or OpenAI Whisper).
pub async fn transcribe(
    client: &reqwest::Client,
    slot: &Slot,
    key: Option<&str>,
    wav: &[u8],
    prev: &str,
    lang: Option<&str>,
) -> Result<String, String> {
    let base = check_url(&slot.base_url)?;
    let part = reqwest::multipart::Part::bytes(wav.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", slot.model.trim().to_string())
        .text("response_format", "json")
        .text("temperature", "0");
    // The last words heard help the model continue a sentence that was split.
    let prev: String = prev.trim().chars().rev().take(200).collect::<Vec<_>>().into_iter().rev().collect();
    let form = if prev.is_empty() { form } else { form.text("prompt", prev) };
    // A fixed language stops the model from guessing wrong (e.g. Hindi written as Urdu).
    let form = match lang { Some(l) => form.text("language", l.to_string()), None => form };
    let mut rb = client.post(format!("{base}/audio/transcriptions")).multipart(form);
    if let Some(k) = key {
        rb = rb.bearer_auth(k);
    }
    let resp = rb.send().await.map_err(net_err)?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(http_err(status.as_u16(), &body));
    }
    body["text"].as_str().map(|s| s.to_string()).ok_or_else(|| "unexpected reply".to_string())
}

fn net_err(e: reqwest::Error) -> String {
    if e.is_timeout() { "timed out".into() } else { "network error (check URL / internet)".into() }
}

fn http_err(code: u16, body: &Value) -> String {
    // Most APIs reply {"error":{"message":..}}; Google sometimes wraps it in a list.
    let e = if body.is_array() { &body[0]["error"] } else { &body["error"] };
    let msg: String = e["message"].as_str().unwrap_or("").chars().take(120).collect();
    let hint = if code == 404 { " (model not found? Open Settings, click Load, pick a listed model)" } else { "" };
    format!("HTTP {code} {msg}{hint}").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_check_is_exact() {
        assert!(is_local("http://localhost:11434/v1"));
        assert!(is_local("HTTP://127.0.0.1:8080"));
        assert!(!is_local("http://localhost.evil.com/v1"));
        assert!(!is_local("http://localhost@evil.com/v1"));
        assert!(!is_local("https://localhost/v1"));
    }

    #[test]
    fn keys_only_over_https_or_local() {
        assert!(check_url("https://api.groq.com/openai/v1/").is_ok());
        assert_eq!(check_url("https://api.groq.com/openai/v1/").unwrap(), "https://api.groq.com/openai/v1");
        assert!(check_url("http://api.groq.com/v1").is_err());
        assert!(check_url("http://localhost:11434/v1").is_ok());
        assert!(check_url("ftp://x.com").is_err());
    }

    #[test]
    fn error_message_is_read_from_object_or_list() {
        let obj = json!({"error": {"message": "bad key"}});
        let list = json!([{"error": {"message": "model gone"}}]);
        assert!(http_err(401, &obj).contains("bad key"));
        assert!(http_err(404, &list).contains("model gone"));
        assert!(http_err(404, &Value::Null).contains("Load"));
    }

    #[test]
    fn ids_and_host() {
        assert!(valid_id("answer-lx3k9a2b"));
        assert!(!valid_id("../x"));
        assert!(!valid_id(""));
        assert_eq!(host("https://api.groq.com/openai/v1"), "api.groq.com");
    }
}
