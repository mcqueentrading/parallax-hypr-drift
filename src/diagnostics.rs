use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static LOG_FILE: OnceLock<Option<Mutex<File>>> = OnceLock::new();
static LAST_HEARTBEAT: OnceLock<Mutex<Instant>> = OnceLock::new();

fn log_file() -> Option<&'static Mutex<File>> {
    LOG_FILE
        .get_or_init(|| {
            let path = std::env::var("DRIFTWM_DIAG_LOG")
                .unwrap_or_else(|_| "/tmp/parallax-hypr-drift.log".to_string());
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()
                .map(Mutex::new)
        })
        .as_ref()
}

pub fn log(event: impl AsRef<str>) {
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return;
    };
    let Some(file) = log_file() else {
        return;
    };
    if let Ok(mut file) = file.lock() {
        let _ = writeln!(
            file,
            "{}.{:03} pid={} {}",
            now.as_secs(),
            now.subsec_millis(),
            std::process::id(),
            event.as_ref()
        );
    }
}

pub fn heartbeat(event: impl FnOnce() -> String) {
    let last = LAST_HEARTBEAT.get_or_init(|| Mutex::new(Instant::now()));
    let Ok(mut last) = last.lock() else {
        return;
    };
    if last.elapsed() >= Duration::from_secs(1) {
        *last = Instant::now();
        log(event());
    }
}
