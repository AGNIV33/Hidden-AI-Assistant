use keyring::Entry;

use crate::providers::{is_local, valid_id, Slot};

// Kept from the old name on purpose, so keys saved before the rename still work.
const SERVICE: &str = "HiddenAIAssistant";

/// One credential per connection id (e.g. "answer-lx3k9a2b").
fn entry(id: &str) -> Result<Entry, String> {
    if !valid_id(id) {
        return Err("Bad connection id".into());
    }
    Entry::new(SERVICE, id).map_err(|e| e.to_string())
}

/// Save an API key in Windows Credential Manager.
pub fn set(id: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Key is empty".into());
    }
    entry(id)?.set_password(key).map_err(|e| e.to_string())
}

pub fn get(id: &str) -> Option<String> {
    entry(id).ok()?.get_password().ok().filter(|k| !k.is_empty())
}

pub fn delete(id: &str) -> Result<(), String> {
    match entry(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Credential name shared by every connection to the same provider (e.g. "host-api-groq-com").
pub fn host_id(base_url: &str) -> String {
    let h: String = crate::providers::host(base_url)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .take(58)
        .collect();
    format!("host-{h}")
}

/// Saved key for a connection: its own key, or any key saved for the same provider.
/// Local servers (e.g. Ollama, Local Whisper) may have none.
pub fn key_for(slot: &Slot) -> Result<Option<String>, String> {
    let key = get(&slot.id).or_else(|| get(&host_id(&slot.base_url)));
    if key.is_none() && !is_local(&slot.base_url) {
        return Err("no key saved".into());
    }
    Ok(key)
}
