//! Background catalog-state preparation; plugin calls stay on the host thread.
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    thread::JoinHandle,
};

#[derive(Default)]
struct Jobs {
    next: u64,
    entries: HashMap<u64, Entry>,
}
enum Entry {
    Preparing(JoinHandle<Result<PathBuf, String>>),
    Prepared(PathBuf),
    Loading,
    Failed(String),
}
fn jobs() -> &'static Mutex<Jobs> {
    static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();
    JOBS.get_or_init(Mutex::default)
}
fn id(input: &Value) -> Result<u64, String> {
    input["jobId"]
        .as_u64()
        .or_else(|| {
            input["jobId"]
                .as_f64()
                .filter(|n| n.fract() == 0.0 && *n >= 0.0)
                .map(|n| n as u64)
        })
        .ok_or("Expected jobId".into())
}
pub fn dispatch(api: &str, input: &Value) -> Result<Value, String> {
    let mut jobs = jobs().lock().map_err(|e| e.to_string())?;
    match api {
        "patchJob.start" => {
            let text = |key| {
                input[key]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("Expected {key}"))
            };
            let format = text("format")?;
            let plugin = text("pluginId")?;
            let patch = text("patch")?;
            jobs.next += 1;
            let id = jobs.next;
            let worker = std::thread::spawn(move || {
                crate::patch_state::background_file(&format, &plugin, &patch)
            });
            jobs.entries.insert(id, Entry::Preparing(worker));
            Ok(json!(id))
        }
        "patchJob.get" => {
            let entry = jobs
                .entries
                .get_mut(&id(input)?)
                .ok_or("Unknown patch job")?;
            if matches!(entry, Entry::Preparing(worker) if worker.is_finished()) {
                let Entry::Preparing(worker) = std::mem::replace(entry, Entry::Loading) else {
                    unreachable!()
                };
                *entry = match worker
                    .join()
                    .unwrap_or_else(|_| Err("Patch worker panicked".into()))
                {
                    Ok(file) => Entry::Prepared(file),
                    Err(error) => Entry::Failed(error),
                };
            }
            Ok(match entry {
                Entry::Preparing(_) | Entry::Loading => json!({"state":"pending"}),
                Entry::Prepared(file) => json!({"state":"completed","file":file}),
                Entry::Failed(error) => json!({"state":"failed","error":error}),
            })
        }
        "patchJob.release" => {
            if let Some(entry) = jobs.entries.remove(&id(input)?) {
                dispose(entry);
            }
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown patch job API: {api}")),
    }
}
fn dispose(entry: Entry) {
    let file = match entry {
        Entry::Prepared(file) => Some(file),
        Entry::Preparing(worker) => worker.join().ok().and_then(Result::ok),
        _ => None,
    };
    if let Some(file) = file {
        let _ = std::fs::remove_file(file);
    }
}
pub fn cleanup() {
    if let Ok(mut jobs) = jobs().lock() {
        for (_, entry) in jobs.entries.drain() {
            dispose(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_preparation_is_never_published_and_release_removes_its_file() {
        let file =
            std::env::temp_dir().join(format!("repl-patch-job-test-{}.bin", std::process::id()));
        let (send, receive) = std::sync::mpsc::channel();
        let worker_file = file.clone();
        let worker = std::thread::spawn(move || {
            receive.recv().unwrap();
            std::fs::write(&worker_file, b"prepared").map_err(|e| e.to_string())?;
            Ok(worker_file)
        });
        let id = {
            let mut jobs = jobs().lock().unwrap();
            jobs.next += 1;
            let id = jobs.next;
            jobs.entries.insert(id, Entry::Preparing(worker));
            id
        };
        assert_eq!(
            dispatch("patchJob.get", &json!({"jobId":id})).unwrap(),
            json!({"state":"pending"})
        );
        send.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let value = dispatch("patchJob.get", &json!({"jobId":id})).unwrap();
            if value["state"] == "completed" {
                assert_eq!(value["file"], json!(file));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(std::fs::read(&file).unwrap(), b"prepared");
        dispatch("patchJob.release", &json!({"jobId":id})).unwrap();
        assert!(!file.exists());
        assert!(dispatch("patchJob.get", &json!({"jobId":id})).is_err());
    }
}
