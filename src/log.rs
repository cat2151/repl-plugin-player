use serde_json::{json, Value};
use std::{fs::File, io::Write, path::PathBuf, time::Instant};

pub struct Log {
    pub path: PathBuf,
    file: File,
    start: Instant,
    session: String,
    pub execution: u64,
}
impl Log {
    pub fn new() -> Result<Self, String> {
        let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(crate::i18n::t("m064"))?)
            .join("repl-plugin-player/logs");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let session = format!("{}_{}", local_timestamp(), std::process::id());
        let path = root.join(format!("{session}.jsonl"));
        let file = File::create(&path).map_err(|e| e.to_string())?;
        eprintln!(
            "{}",
            crate::i18n::render("log_path", &[("path", path.display().to_string())])
        );
        Ok(Self {
            path,
            file,
            start: Instant::now(),
            session,
            execution: 0,
        })
    }
    pub fn event(&mut self, kind: &str, details: Value) -> Result<(), String> {
        let line = json!({"session":self.session,"run":self.session,"execution":self.execution,"monotonic_ms":self.start.elapsed().as_secs_f64()*1000.0,"event":kind,"details":details});
        writeln!(self.file, "{line}")
            .and_then(|_| self.file.flush())
            .map_err(|e| e.to_string())
    }
}

/// Local wall-clock time as `YYYY-MM-DD_HH-MM-SS`, which sorts chronologically.
fn local_timestamp() -> String {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut t = SystemTime::default();
    // GetLocalTime only writes into the caller-owned SYSTEMTIME and cannot fail.
    unsafe { GetLocalTime(&mut t) };
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}
