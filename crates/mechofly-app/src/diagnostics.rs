use std::{
    backtrace::Backtrace,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

static STARTUP_JOURNAL: OnceLock<PathBuf> = OnceLock::new();

pub fn initialize() {
    let directory = log_directory();
    let _ = fs::create_dir_all(&directory);
    let path = directory.join(format!(
        "startup-{}-pid{}.log",
        unix_millis(),
        std::process::id()
    ));
    let _ = STARTUP_JOURNAL.set(path);
    mark("process entered Rust main");

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        mark("fatal Rust panic");
        append(&format!(
            "panic={panic_info}\nbacktrace=\n{}\n",
            Backtrace::force_capture()
        ));
        previous_hook(panic_info);
    }));
}

pub fn mark(message: &str) {
    append(&format!("utc_unix_ms={} {message}\n", unix_millis()));
}

pub fn record_fatal_error(message: &str) {
    mark("main returned a fatal error");
    append(&format!("error={message}\n"));
}

fn append(message: &str) {
    let Some(path) = STARTUP_JOURNAL.get() else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(message.as_bytes());
    }
}

fn log_directory() -> PathBuf {
    crate::storage::override_directory()
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("MechoFly")
        })
        .join("logs")
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
