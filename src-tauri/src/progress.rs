use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT: &str = "job-progress";
const MIN_GAP: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub job: String,
    pub current: u64,
    pub total: u64,
    pub filename: String,
    pub message: String,
}

struct EmitGate {
    last: Option<Instant>,
    last_job: String,
}

static GATE: Mutex<EmitGate> = Mutex::new(EmitGate {
    last: None,
    last_job: String::new(),
});

pub fn emit(app: &AppHandle, job: &str, current: u64, total: u64, filename: &str, message: &str) {
    let force = current <= 1 || (total > 0 && current >= total);
    if let Ok(mut gate) = GATE.lock() {
        let job_changed = gate.last_job != job;
        let due = gate
            .last
            .map(|prev| prev.elapsed() >= MIN_GAP)
            .unwrap_or(true);
        if !force && !job_changed && !due {
            return;
        }
        gate.last = Some(Instant::now());
        gate.last_job = job.to_string();
    }

    let payload = JobProgress {
        job: job.to_string(),
        current,
        total,
        filename: filename.to_string(),
        message: message.to_string(),
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(EVENT, &payload);
    } else {
        let _ = app.emit(EVENT, &payload);
    }
}
