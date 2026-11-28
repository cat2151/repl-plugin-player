mod capture;
mod monitor;

use serde_json::{json, Value};
use std::{
    cell::RefCell,
    ffi::{c_char, CStr, CString},
    fs, io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, AtomicU8, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

const RECORD: u8 = 0;
const FINISH: u8 = 1;
const DISCARD: u8 = 2;

struct Take {
    path: PathBuf,
    command: Arc<AtomicU8>,
    level: Arc<AtomicU32>,
    progress: Arc<Progress>,
    worker: JoinHandle<Result<bool, String>>,
}

#[derive(Default)]
struct Progress {
    rate: AtomicU32,
    frames: AtomicU32,
}

impl Progress {
    fn snapshot(&self, path: &std::path::Path, running: bool) -> Value {
        let frames = self.frames.load(Ordering::Acquire);
        json!({
            "filepath": path,
            "frames": frames,
            "sampleRate": self.rate.load(Ordering::Relaxed),
            "running": running
        })
    }
}

impl Take {
    fn start(directory: PathBuf) -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = directory.join(format!("Recording-{timestamp}-{}.wav", std::process::id()));
        let command = Arc::new(AtomicU8::new(RECORD));
        let level = Arc::new(AtomicU32::new(capture::NO_SAMPLES));
        let progress = Arc::new(Progress::default());
        let state = command.clone();
        let input_level = level.clone();
        let output = path.clone();
        let written = progress.clone();
        let worker = thread::Builder::new()
            .name("microphone-recording".into())
            .spawn(move || run(directory, output, &state, input_level, &written))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            path,
            command,
            level,
            progress,
            worker,
        })
    }

    fn preview(&self) -> Value {
        self.progress.snapshot(&self.path, self.is_recording())
    }

    fn input_level(&self) -> Option<f32> {
        let bits = self.level.load(Ordering::Relaxed);
        (self.is_recording() && bits != capture::NO_SAMPLES).then(|| f32::from_bits(bits))
    }

    fn is_recording(&self) -> bool {
        self.command.load(Ordering::Acquire) == RECORD && !self.worker.is_finished()
    }

    fn complete(self) -> Result<Value, String> {
        let result = self
            .worker
            .join()
            .map_err(|_| "Recording worker panicked")?;
        if self.command.load(Ordering::Acquire) == DISCARD {
            if result == Ok(true) {
                remove(&self.path)?;
            }
            return Ok(json!({}));
        }
        if result? {
            Ok(self.progress.snapshot(&self.path, false))
        } else {
            Ok(json!({}))
        }
    }
}

fn remove(path: &std::path::Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn run(
    directory: PathBuf,
    path: PathBuf,
    command: &AtomicU8,
    level: Arc<AtomicU32>,
    progress: &Progress,
) -> Result<bool, String> {
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let file =
        fs::File::create_new(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let result = capture::record(file, command, level, progress);
    if result == Ok(true) && command.load(Ordering::Acquire) != DISCARD {
        return Ok(true);
    }
    remove(&path)?;
    result.map(|_| false)
}

#[derive(Default)]
struct Session {
    take: Option<Take>,
    monitor: Option<monitor::Monitor>,
}

fn dispatch(session: &mut Session, request: &Value) -> Result<Value, String> {
    let slot = &mut session.take;
    let take = slot.as_ref();
    match request["api"].as_str() {
        Some("recording.start") => {
            if let Some(take) = take {
                if take.command.load(Ordering::Acquire) != DISCARD || !take.worker.is_finished() {
                    return Err("A microphone recording is already in progress".into());
                }
                slot.take().unwrap().complete()?;
            }
            let directory = request["directory"]
                .as_str()
                .ok_or("Missing recording directory")?;
            let take = Take::start(directory.into())?;
            let preview = take.preview();
            *slot = Some(take);
            return Ok(preview);
        }
        Some("recording.stop") => {
            if let Some(take) = take {
                let _ = take.command.compare_exchange(
                    RECORD,
                    FINISH,
                    Ordering::Release,
                    Ordering::Relaxed,
                );
            }
        }
        Some("recording.poll") => {
            if take.is_some_and(|take| take.worker.is_finished()) {
                return slot.take().unwrap().complete();
            }
        }
        Some("recording.preview") => {
            return Ok(take
                .filter(|take| take.command.load(Ordering::Acquire) != DISCARD)
                .map_or(Value::Null, Take::preview));
        }
        Some("recording.level") => return Ok(json!(take.and_then(Take::input_level))),
        Some("recording.monitor.open") => {
            if session.monitor.is_none() {
                session.monitor = Some(monitor::Monitor::new()?);
            }
            session.monitor.as_ref().unwrap().set_open(true)?;
        }
        Some("recording.monitor.close") => {
            if let Some(monitor) = &session.monitor {
                monitor.set_open(false)?;
            }
        }
        Some("recording.monitor.level") => {
            let recording = take.is_some_and(Take::is_recording);
            let level = take.and_then(Take::input_level).or_else(|| {
                session
                    .monitor
                    .as_ref()
                    .and_then(monitor::Monitor::input_level)
            });
            return Ok(json!({"recording": recording, "level": level}));
        }
        Some("recording.cancel" | "recording.shutdown") => {
            if let Some(take) = take {
                take.command.store(DISCARD, Ordering::Release);
            }
            if request["api"] == "recording.shutdown" {
                if let Some(monitor) = session.monitor.take() {
                    monitor.shutdown();
                }
                if let Some(take) = slot.take() {
                    take.complete()?;
                }
            }
        }
        _ => return Err("Unknown recording API".into()),
    }
    Ok(Value::Null)
}

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::default());
    static REPLY: RefCell<CString> = RefCell::new(CString::default());
}

/// # Safety
/// `request` must be a valid NUL-terminated UTF-8 string from the control thread.
/// Copy the reply before calling again. Call `recording.shutdown` before unloading.
#[no_mangle]
pub unsafe extern "C" fn repl_recording_request(request: *const c_char) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        let input = unsafe { CStr::from_ptr(request) }.to_bytes();
        let request: Value = serde_json::from_slice(input).map_err(|error| error.to_string())?;
        SESSION.with(|session| dispatch(&mut session.borrow_mut(), &request))
    });
    let reply = match result {
        Ok(Ok(value)) => json!({"ok":true,"value":value}),
        Ok(Err(error)) => json!({"ok":false,"error":error}),
        Err(_) => json!({"ok":false,"error":"Recording dispatch panicked"}),
    };
    REPLY.with(|slot| {
        *slot.borrow_mut() = CString::new(reply.to_string()).expect("JSON escapes NUL");
        slot.borrow().as_ptr()
    })
}
