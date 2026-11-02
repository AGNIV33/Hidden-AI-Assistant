//! Listening mode: capture system audio ("them") and optionally the mic ("me"),
//! cut it into speech chunks, turn each chunk into text, and emit `transcript` events.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::providers::{self, Slot};
use crate::secrets;

const RATE: usize = 16_000; // what speech-to-text models like
const FRAME: usize = RATE / 50; // 20 ms
const END_SILENCE: usize = 40; // 0.8 s of quiet ends a chunk (normal pauses between words stay inside)
const SOFT_MAX: usize = 350; // after 7 s, cut at the next tiny pause (keeps text flowing)
const MAX_FRAMES: usize = 750; // a chunk is never longer than 15 s
const WHOLE_MAX: usize = RATE * 60 * 8; // one recording keeps at most 8 minutes of speech
const MIN_SPEECH: usize = 12; // ignore blips shorter than ~0.25 s
const PRE_ROLL: usize = 8; // keep 0.16 s before speech starts

/// Holds the stop flag of the running listener (if any).
#[derive(Default)]
pub struct ListenState(pub Mutex<Option<Arc<AtomicBool>>>);

/// Start listening. "them" = whatever plays on the default speakers/headphones.
/// "me" = the default microphone (only when `mic` is true).
/// `whole` = mic-button mode: keep all speech and turn it into text in one go when stopped.
pub fn start(app: AppHandle, stt: Vec<Slot>, mic: bool, spoken: String, whole: bool) -> Result<Arc<AtomicBool>, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stt = Arc::new(stt);
    let spoken: Arc<str> = Arc::from(spoken.as_str());
    let sources: &[(&'static str, bool, bool)] = &[("them", true, true), ("me", false, mic)];
    for &(label, loopback, wanted) in sources {
        if !wanted {
            continue;
        }
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (app2, stop2, stt2, sp2) = (app.clone(), stop.clone(), stt.clone(), spoken.clone());
        std::thread::spawn(move || run_source(app2, label, loopback, stop2, stt2, sp2, whole, ready_tx));
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                if label == "them" {
                    stop.store(true, Ordering::Relaxed);
                    return Err(e);
                }
                let _ = app.emit("listen-error", format!("Microphone off: {e}"));
            }
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                return Err("Audio device did not start in time".into());
            }
        }
    }
    Ok(stop)
}

fn run_source(
    app: AppHandle,
    label: &'static str,
    loopback: bool,
    stop: Arc<AtomicBool>,
    stt: Arc<Vec<Slot>>,
    spoken: Arc<str>,
    whole: bool,
    ready: mpsc::Sender<Result<(), String>>,
) {
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let (stream, in_rate) = match open_stream(&app, loopback, tx) {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    // Speech-to-text runs in its own thread: capturing never waits for the internet.
    let (wav_tx, wav_rx) = mpsc::channel::<Vec<u8>>();
    let app2 = app.clone();
    let worker = std::thread::spawn(move || stt_worker(app2, label, stt, spoken, whole, wav_rx));

    let mut chunker = Chunker::new(in_rate);
    let mut all: Vec<i16> = Vec::new(); // mic-button mode: every piece of speech, joined
    let take = |w: Vec<u8>, all: &mut Vec<i16>| {
        if !whole {
            let _ = wav_tx.send(w);
            return;
        }
        if all.len() >= WHOLE_MAX {
            return;
        }
        if !all.is_empty() {
            all.extend(std::iter::repeat(0).take(RATE / 4)); // short gap between pieces
        }
        all.extend(w[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])));
        let _ = app.emit("listen-level", json!({ "speaker": label, "secs": all.len() / RATE }));
    };
    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(samples) => {
                for w in chunker.push(&samples) {
                    take(w, &mut all);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
    // Stopped: use what is still buffered (the sentence being spoken right now).
    while let Ok(samples) = rx.try_recv() {
        for w in chunker.push(&samples) {
            take(w, &mut all);
        }
    }
    if let Some(w) = chunker.flush() {
        take(w, &mut all);
    }
    if whole && !all.is_empty() {
        let _ = wav_tx.send(wav(&all)); // the whole recording, sent once
    }
    drop(wav_tx); // worker finishes the queue, then says "done"
    let _ = worker.join();
}

/// Turns chunks into text in order, reusing one connection, and gives the model
/// the previous words so sentences split across chunks still read naturally.
fn stt_worker(app: AppHandle, label: &'static str, stt: Arc<Vec<Slot>>, spoken: Arc<str>, whole: bool, rx: mpsc::Receiver<Vec<u8>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(if whole { 180 } else { 25 }))
        .pool_idle_timeout(Duration::from_secs(90))
        .build();
    let Ok(client) = client else {
        let _ = app.emit("listen-done", label);
        return;
    };
    let mut prev = String::new();
    let lang = LangPlan::new(&spoken);
    let mut last_lang: Option<&'static str> = None; // language of the last good chunk
    for wav in rx {
        let secs = (wav.len().saturating_sub(44)) as f32 / (RATE as f32 * 2.0);
        let res = tauri::async_runtime::block_on(async {
            let p = seed_prompt(&prev, lang.forced);
            let mut r = transcribe_any(&client, &stt, &wav, &p, lang.forced).await;
            // Auto mode: if the text came out in a script nobody speaks here
            // (Urdu, Gujarati, Chinese...), the model guessed wrong. Ask again with a fixed language.
            if lang.forced.is_none() && !lang.allowed.is_empty() {
                if let Ok(t) = &r {
                    if !t.trim().is_empty() && !script_lang(t).is_some_and(|l| lang.allowed.contains(&l)) {
                        let retry = last_lang.unwrap_or("hi");
                        let p = seed_prompt(&prev, Some(retry));
                        if let Ok(t2) = transcribe_any(&client, &stt, &wav, &p, Some(retry)).await {
                            r = Ok(t2);
                        }
                    }
                }
            }
            r
        });
        if let Ok(t) = &res {
            if let Some(l) = script_lang(t) {
                if lang.allowed.is_empty() || lang.allowed.contains(&l) {
                    last_lang = Some(l);
                }
            }
        }
        match res {
            Ok(text) => {
                let t = text.trim();
                if t.is_empty() || is_filler(t, secs) {
                    continue;
                }
                let _ = app.emit("transcript", json!({ "speaker": label, "text": t }));
                prev.push(' ');
                prev.push_str(t);
                if prev.len() > 400 {
                    let cut = prev.char_indices().map(|(i, _)| i).find(|&i| i >= prev.len() - 300).unwrap_or(0);
                    prev = prev[cut..].to_string();
                }
            }
            Err(e) => report(&app, format!("Speech-to-text failed: {e}")),
        }
    }
    let _ = app.emit("listen-done", label);
}

fn open_stream(
    app: &AppHandle,
    loopback: bool,
    tx: mpsc::Sender<Vec<f32>>,
) -> Result<(Stream, u32), String> {
    let host = cpal::default_host();
    let device = if loopback { host.default_output_device() } else { host.default_input_device() }
        .ok_or_else(|| {
            if loopback { "No audio output device found".to_string() } else { "No microphone found".to_string() }
        })?;
    // On Windows, opening an OUTPUT device as an input records what it is playing (loopback).
    let supported = if loopback { device.default_output_config() } else { device.default_input_config() }
        .map_err(|e| e.to_string())?;
    let channels = supported.channels() as usize;
    let rate = supported.sample_rate().0;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();

    let err_app = app.clone();
    let err_cb = move |e: cpal::StreamError| {
        let _ = err_app.emit("listen-error", format!("Audio stopped: {e}"));
    };
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, channels, tx, err_cb),
        SampleFormat::I16 => build::<i16>(&device, &config, channels, tx, err_cb),
        SampleFormat::U16 => build::<u16>(&device, &config, channels, tx, err_cb),
        other => Err(format!("Unsupported audio format: {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate))
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: mpsc::Sender<Vec<f32>>,
    err_cb: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let ch = channels.max(1);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                // Mix all channels down to mono.
                let mono: Vec<f32> = data
                    .chunks(ch)
                    .map(|f| f.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / f.len() as f32)
                    .collect();
                let _ = tx.send(mono);
            },
            err_cb,
            None,
        )
        .map_err(|e| e.to_string())
}

/* ---------- cut audio into speech chunks ---------- */

struct Chunker {
    ratio: f64,       // input samples per 16 kHz sample
    pos: f64,         // read position inside `buf`
    buf: Vec<f32>,    // input waiting to be resampled
    acc: Vec<f32>,    // 16 kHz samples waiting to form a frame
    noise: f32,       // slow estimate of the background level
    pre: VecDeque<Vec<i16>>,
    cur: Vec<i16>,
    in_speech: bool,
    quiet: usize,
    speech: usize,
}

impl Chunker {
    fn new(in_rate: u32) -> Self {
        Chunker {
            ratio: in_rate as f64 / RATE as f64,
            pos: 0.0,
            buf: Vec::new(),
            acc: Vec::new(),
            noise: 0.0,
            pre: VecDeque::new(),
            cur: Vec::new(),
            in_speech: false,
            quiet: 0,
            speech: 0,
        }
    }

    /// Feed mono samples at the device rate. Returns finished chunks as WAV files.
    fn push(&mut self, input: &[f32]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(input);
        loop {
            let end = self.pos + self.ratio;
            let b = end.ceil() as usize;
            if b > self.buf.len() {
                break;
            }
            let a = (self.pos as usize).min(b.saturating_sub(1));
            let slice = &self.buf[a..b];
            self.acc.push(slice.iter().sum::<f32>() / slice.len() as f32);
            self.pos = end;
        }
        let used = self.pos as usize;
        self.buf.drain(..used);
        self.pos -= used as f64;

        let mut out = Vec::new();
        while self.acc.len() >= FRAME {
            let frame: Vec<f32> = self.acc.drain(..FRAME).collect();
            if let Some(w) = self.frame(&frame) {
                out.push(w);
            }
        }
        out
    }

    /// The speech chunk that has started but not ended yet (used when listening stops).
    fn flush(&mut self) -> Option<Vec<u8>> {
        let out = if self.in_speech && self.speech >= MIN_SPEECH { Some(wav(&self.cur)) } else { None };
        self.in_speech = false;
        self.cur.clear();
        self.speech = 0;
        self.quiet = 0;
        out
    }

    fn frame(&mut self, f: &[f32]) -> Option<Vec<u8>> {
        let rms = (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt();
        let speech = rms > (self.noise * 3.0).max(0.01);
        if !speech {
            self.noise = if self.noise == 0.0 { rms } else { 0.98 * self.noise + 0.02 * rms };
        }
        let pcm: Vec<i16> = f.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();

        if !self.in_speech {
            if speech {
                self.in_speech = true;
                self.cur.clear();
                for p in self.pre.drain(..) {
                    self.cur.extend(p);
                }
                self.cur.extend(pcm);
                self.speech = 1;
                self.quiet = 0;
            } else {
                self.pre.push_back(pcm);
                if self.pre.len() > PRE_ROLL {
                    self.pre.pop_front();
                }
            }
            return None;
        }

        self.cur.extend(pcm);
        if speech {
            self.speech += 1;
            self.quiet = 0;
        } else {
            self.quiet += 1;
        }
        let frames = self.cur.len() / FRAME;
        if self.quiet >= END_SILENCE || frames >= MAX_FRAMES || (frames >= SOFT_MAX && !speech) {
            self.in_speech = false;
            let out = if self.speech >= MIN_SPEECH { Some(wav(&self.cur)) } else { None };
            self.cur.clear();
            self.speech = 0;
            self.quiet = 0;
            return out;
        }
        None
    }
}

/// 16-bit mono 16 kHz WAV file in memory.
fn wav(pcm: &[i16]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + pcm.len() * 2);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&(RATE as u32).to_le_bytes());
    v.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/* ---------- speech to text ---------- */

static LAST_ERR: Mutex<Option<Instant>> = Mutex::new(None);

fn report(app: &AppHandle, msg: String) {
    // At most one error toast every 10 seconds.
    let mut last = LAST_ERR.lock().unwrap_or_else(|e| e.into_inner());
    if last.map(|t| t.elapsed() > Duration::from_secs(10)).unwrap_or(true) {
        *last = Some(Instant::now());
        let _ = app.emit("listen-error", msg);
    }
}

/// Try each Speech-to-text connection in order (main first, then backups).
async fn transcribe_any(client: &reqwest::Client, stt: &[Slot], wav: &[u8], prev: &str, lang: Option<&str>) -> Result<String, String> {
    if stt.is_empty() {
        return Err("No Speech-to-text model set".into());
    }
    let mut errors = Vec::new();
    for (i, s) in stt.iter().enumerate() {
        let label = format!("#{} {}", i + 1, providers::host(&s.base_url));
        if s.model.trim().is_empty() {
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
        let mut r = providers::transcribe(client, s, key.as_deref(), wav, prev, lang).await;
        if matches!(&r, Err(e) if e.starts_with("HTTP 429") || e.starts_with("HTTP 503")) {
            tokio::time::sleep(Duration::from_millis(1500)).await; // free plan limit: wait, try once more
            r = providers::transcribe(client, s, key.as_deref(), wav, prev, lang).await;
        }
        match r {
            Ok(t) => return Ok(t),
            Err(e) => errors.push(format!("{label}: {e}")),
        }
    }
    Err(errors.join("; "))
}

/// Which languages to expect, from the "People speak" setting.
struct LangPlan {
    forced: Option<&'static str>,
    allowed: &'static [&'static str],
}
impl LangPlan {
    fn new(spoken: &str) -> Self {
        match spoken {
            "en" => Self { forced: Some("en"), allowed: &["en"] },
            "hi" => Self { forced: Some("hi"), allowed: &["hi"] },
            "bn" => Self { forced: Some("bn"), allowed: &["bn"] },
            "any" => Self { forced: None, allowed: &[] },
            _ => Self { forced: None, allowed: &["en", "hi", "bn"] },
        }
    }
}

/// Guess the language from the letters used: Devanagari = Hindi, Bengali script = Bengali,
/// Latin = English. Anything else (Urdu, Tamil, Chinese...) = "other".
fn script_lang(text: &str) -> Option<&'static str> {
    let (mut hi, mut bn, mut en, mut other) = (0, 0, 0, 0);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        match c as u32 {
            0x0900..=0x097F | 0xA8E0..=0xA8FF => hi += 1,
            0x0980..=0x09FF => bn += 1,
            _ if c.is_ascii_alphabetic() => en += 1,
            _ => other += 1,
        }
    }
    // Mostly-wrong script among the non-English letters = the model guessed the wrong language.
    if other >= 3 && other > hi + bn {
        return Some("other");
    }
    let best = [(hi, "hi"), (bn, "bn"), (en, "en")].into_iter().max_by_key(|x| x.0)?;
    if best.0 == 0 { None } else { Some(best.1) }
}

/// The words heard before help the model. With nothing heard yet, a few words in the
/// right script nudge it to write Hindi in Devanagari and Bengali in Bengali script.
fn seed_prompt(prev: &str, lang: Option<&str>) -> String {
    if !prev.trim().is_empty() {
        return prev.to_string();
    }
    match lang {
        Some("hi") => "नमस्ते, आज की मीटिंग में हम बात करेंगे।".into(),
        Some("bn") => "নমস্কার, আজকের মিটিংয়ে আমরা কথা বলব।".into(),
        _ => String::new(),
    }
}

/// Speech models invent stock phrases from very short noise. Drop those.
fn is_filler(text: &str, secs: f32) -> bool {
    if secs >= 3.0 {
        return false;
    }
    let n: String = text.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    ["thankyou", "thanks", "thankyouforwatching", "thanksforwatching", "bye", "you", "okay", "ok", "uh", "um", "hmm"]
        .contains(&n.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, secs: f32, amp: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| amp * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn script_detection() {
        assert_eq!(script_lang("आप कैसे हैं?"), Some("hi"));
        assert_eq!(script_lang("আপনি কেমন আছেন?"), Some("bn"));
        assert_eq!(script_lang("How are you?"), Some("en"));
        assert_eq!(script_lang("آپ کیسے ہیں"), Some("other"));
        assert_ne!(script_lang("Project का deadline कब है?"), Some("other"));
        assert_eq!(script_lang("Project کا deadline کب ہے"), Some("other"));
        assert_eq!(script_lang("123 ..."), None);
        assert!(LangPlan::new("auto3").allowed.contains(&"bn"));
        assert_eq!(LangPlan::new("bn").forced, Some("bn"));
    }

    #[test]
    fn one_utterance_makes_one_wav_chunk() {
        let mut c = Chunker::new(48_000);
        let mut chunks = Vec::new();
        for part in [vec![0.0; 48_000], tone(48_000, 1.5, 0.3), vec![0.0; 48_000 * 2]] {
            for piece in part.chunks(480) {
                chunks.extend(c.push(piece));
            }
        }
        assert_eq!(chunks.len(), 1);
        let w = &chunks[0];
        assert_eq!(&w[0..4], b"RIFF");
        assert!(w.len() > 44 + 16_000 * 2 * 3 / 2, "chunk should hold ~1.5 s of audio");
    }

    #[test]
    fn silence_and_tiny_blips_make_nothing() {
        let mut c = Chunker::new(44_100);
        let mut chunks = Vec::new();
        for part in [vec![0.0; 44_100 * 2], tone(44_100, 0.05, 0.3), vec![0.0; 44_100 * 2]] {
            chunks.extend(c.push(&part));
        }
        assert!(chunks.is_empty());
    }

    #[test]
    fn stopping_mid_sentence_keeps_it() {
        let mut c = Chunker::new(16_000);
        let mut chunks = Vec::new();
        for part in [vec![0.0; 16_000], tone(16_000, 1.0, 0.3)] {
            chunks.extend(c.push(&part));
        }
        assert!(chunks.is_empty(), "still talking, nothing finished yet");
        assert!(c.flush().is_some(), "stop should hand over the unfinished sentence");
        assert!(c.flush().is_none());
    }

    #[test]
    fn long_speech_is_split_into_short_pieces() {
        let mut c = Chunker::new(16_000);
        let mut chunks = Vec::new();
        // 10 s of talking with a tiny dip every 2.5 s (like gaps between words)
        for i in 0..4 {
            chunks.extend(c.push(&tone(16_000, 2.5, 0.3)));
            if i < 3 { chunks.extend(c.push(&vec![0.0; 480])); }
        }
        chunks.extend(c.push(&vec![0.0; 16_000]));
        assert!(chunks.len() >= 2, "got {} chunks", chunks.len());
        for w in &chunks {
            assert!((w.len() - 44) / 2 <= 16_000 * 8 + 16_000, "chunk longer than ~8 s");
        }
    }

    #[test]
    fn filler_filter() {
        assert!(is_filler("Thank you.", 1.0));
        assert!(!is_filler("Thank you.", 5.0));
        assert!(!is_filler("What is your notice period?", 1.0));
    }
}
