//! asciinema cast v2 session recording. One file per session, written by a
//! background task that drains an mpsc of RecordEvent. The hot path only does
//! a hashmap lookup + unbounded channel send.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Decode a terminal:output / terminal:input `payload.data` field into the raw
/// terminal bytes (as a lossy UTF-8 string for the cast file). Both the agent
/// (output) and the browser (input) base64-encode terminal data, so the cast
/// must decode it back or replays would show base64 gibberish. Falls back to
/// the raw string if it isn't valid base64.
pub fn decode_terminal_data(s: &str) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    B64
        .decode(s)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_else(|| s.to_string())
}

/// One terminal event to append to a session's cast file.
#[derive(Debug, Clone)]
pub enum RecordEvent {
    /// terminal:output payload.data
    Output(String),
    /// terminal:input payload.data
    Input(String),
}

impl RecordEvent {
    fn stream_char(&self) -> &'static str {
        match self {
            RecordEvent::Output(_) => "o",
            RecordEvent::Input(_) => "i",
        }
    }
    fn data(&self) -> &str {
        match self {
            RecordEvent::Output(s) => s,
            RecordEvent::Input(s) => s,
        }
    }
}

/// Records terminal I/O to asciinema cast v2 files under `dir`. When `None`
/// (the field on SharedState is `Option<Arc<Recorder>>`), recording is fully
/// disabled and the hot path is a single `Option::is_some` check.
pub struct Recorder {
    dir: PathBuf,
    writers: RwLock<HashMap<String, mpsc::UnboundedSender<RecordEvent>>>,
}

impl Recorder {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            writers: RwLock::new(HashMap::new()),
        }
    }

    /// Record an event for a session. Non-blocking: the fast path is a read
    /// lock + unbounded send; the first event for a session spawns a writer
    /// task that owns the file handle.
    pub fn record(&self, session_id: &str, ev: RecordEvent) {
        // Fast path: existing writer.
        {
            if let Ok(w) = self.writers.read() {
                if let Some(tx) = w.get(session_id) {
                    let _ = tx.send(ev);
                    return;
                }
            }
        }
        // Slow path: spawn a writer for this session.
        let mut w = match self.writers.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(tx) = w.get(session_id) {
            let _ = tx.send(ev);
            return;
        }
        let (tx, rx) = mpsc::unbounded_channel::<RecordEvent>();
        w.insert(session_id.to_string(), tx.clone());
        let _ = tx.send(ev); // first event
        let dir = self.dir.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            run_writer(dir, sid, rx).await;
        });
    }

    /// True if a writer task is currently open for this session.
    pub fn is_recording(&self, session_id: &str) -> bool {
        self.writers
            .read()
            .map(|w| w.contains_key(session_id))
            .unwrap_or(false)
    }

    /// Drop the sender for a session so the writer task drains, flushes, and
    /// exits. Safe to call for sessions that were never recorded.
    pub fn close(&self, session_id: &str) {
        if let Ok(mut w) = self.writers.write() {
            w.remove(session_id);
        }
    }
}

/// Compute unix seconds for the cast header timestamp.
fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Writer task: opens `{dir}/{sid}_{unix}.cast`, writes the v2 header, then
/// drains the channel writing one JSONL event per message with a 250ms
/// periodic flush. When the channel closes, final-flush and exit.
async fn run_writer(dir: PathBuf, sid: String, mut rx: mpsc::UnboundedReceiver<RecordEvent>) {
    let ts = unix_secs();
    let path = dir.join(format!("{}_{}.cast", sid, ts));

    let file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
    {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(session = %sid, path = %path.display(), err = %e, "recorder: open failed");
            return;
        }
    };
    let mut buf = tokio::io::BufWriter::new(file);

    // Header. Each writer owns a fresh file (the filename includes the spawn
    // timestamp), so always emit the v2 header as the first line.
    let header = serde_json::json!({
        "version": 2,
        "width": 80,
        "height": 24,
        "timestamp": ts,
    });
    if let Err(e) = buf
        .write_all(format!("{}\n", header).as_bytes())
        .await
    {
        tracing::error!(session = %sid, err = %e, "recorder: header write failed");
        return;
    }
    let _ = buf.flush().await;

    let start = Instant::now();
    let mut flush = tokio::time::interval(tokio::time::Duration::from_millis(250));
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            ev = rx.recv() => match ev {
                Some(ev) => {
                    let elapsed = start.elapsed().as_secs_f64();
                    let line = serde_json::json!([elapsed, ev.stream_char(), ev.data()]);
                    if let Err(e) = buf.write_all(format!("{}\n", line).as_bytes()).await {
                        tracing::error!(session = %sid, err = %e, "recorder: event write failed");
                        break;
                    }
                }
                None => break,
            },
            _ = flush.tick() => {
                let _ = buf.flush().await;
            }
        }
    }
    let _ = buf.flush().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn read_file(path: &std::path::Path) -> String {
        tokio::fs::read_to_string(path).await.unwrap()
    }

    fn tempdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("sr-rec-test-{}", unix_nanos()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn unix_nanos() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| format!("{}{}", d.as_secs(), d.subsec_nanos()))
            .unwrap_or_else(|_| "0".to_string())
    }

    #[tokio::test]
    async fn test_records_output_and_input_events() {
        let dir = tempdir();
        let rec = Recorder::new(dir.clone());
        rec.record("s1", RecordEvent::Output("hello\n".to_string()));
        rec.record("s1", RecordEvent::Input("ls".to_string()));
        rec.close("s1");
        // Give the writer task time to drain + flush.
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(files.len(), 1);
        let p = files[0].path(); let content = read_file(&p).await;
        let mut lines = content.lines();
        let header: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(header["version"], 2);
        assert_eq!(header["width"], 80);
        assert_eq!(header["height"], 24);

        let ev1: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(ev1[1], "o");
        assert_eq!(ev1[2], "hello\n");
        let ev2: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(ev2[1], "i");
        assert_eq!(ev2[2], "ls");
    }

    #[tokio::test]
    async fn test_json_escapes_special_chars() {
        let dir = tempdir();
        let rec = Recorder::new(dir.clone());
        rec.record("s2", RecordEvent::Output("a\"b\\c\n".to_string()));
        rec.close("s2");
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let p = files[0].path(); let content = read_file(&p).await;
        // The event line must be valid JSON with the data round-tripping.
        let evline = content.lines().nth(1).unwrap();
        let v: serde_json::Value = serde_json::from_str(evline).unwrap();
        assert_eq!(v[2], "a\"b\\c\n");
    }

    #[tokio::test]
    async fn test_is_recording_lifecycle() {
        let dir = tempdir();
        let rec = Recorder::new(dir.clone());
        assert!(!rec.is_recording("s3"));
        rec.record("s3", RecordEvent::Output("x".to_string()));
        assert!(rec.is_recording("s3"));
        rec.close("s3");
        assert!(!rec.is_recording("s3"));
    }

    #[test]
    fn test_decode_terminal_data_base64() {
        // "hello\n" base64
        assert_eq!(decode_terminal_data("aGVsbG8K"), "hello\n");
        // invalid base64 (hyphen) falls back to raw
        assert_eq!(decode_terminal_data("hello-world"), "hello-world");
        // empty
        assert_eq!(decode_terminal_data(""), "");
    }
}
