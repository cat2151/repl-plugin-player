use libloading::Library;
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    ffi::{c_char, CStr, CString},
};

struct Recorder {
    _library: Library,
    request: unsafe extern "C" fn(*const c_char) -> *const c_char,
}

impl Recorder {
    fn load() -> Result<Self, String> {
        let path = crate::resources::extension("recording")?;
        unsafe {
            let library =
                Library::new(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            let request = *library
                .get(b"repl_recording_request\0")
                .map_err(|error| error.to_string())?;
            Ok(Self {
                _library: library,
                request,
            })
        }
    }

    fn call(&self, value: &Value) -> Result<Value, String> {
        let text = CString::new(value.to_string()).expect("JSON escapes NUL");
        let reply = unsafe { CStr::from_ptr((self.request)(text.as_ptr())) };
        let reply: Value =
            serde_json::from_slice(reply.to_bytes()).map_err(|error| error.to_string())?;
        if reply["ok"] != true {
            return Err(reply["error"]
                .as_str()
                .unwrap_or("Recording failed")
                .to_owned());
        }
        Ok(reply["value"].clone())
    }
}

thread_local! {
    static RECORDER: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

pub fn dispatch(api: &str) -> Result<Value, String> {
    RECORDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let request = match api {
            "recording.start" => {
                let directory = crate::resources::user_directory()?.join("recordings");
                if slot.is_none() {
                    *slot = Some(Recorder::load()?);
                }
                json!({"api":api,"directory":directory})
            }
            "recording.stop" | "recording.poll" | "recording.cancel" | "recording.level"
            | "recording.preview" => {
                json!({"api":api})
            }
            _ => return Err("Unknown recording API".into()),
        };
        match slot.as_ref() {
            Some(recorder) => recorder.call(&request),
            None => Ok(Value::Null),
        }
    })
}

pub fn open_monitor() -> Result<(), String> {
    RECORDER.with_borrow_mut(|slot| {
        if slot.is_none() {
            *slot = Some(Recorder::load()?);
        }
        slot.as_ref()
            .unwrap()
            .call(&json!({"api":"recording.monitor.open"}))?;
        Ok(())
    })
}

pub fn close_monitor() {
    let _ = monitor_request("recording.monitor.close");
}

pub fn monitor_level() -> (bool, Option<f32>) {
    let value = monitor_request("recording.monitor.level").unwrap_or(Value::Null);
    (
        value["recording"] == true,
        value["level"].as_f64().map(|level| level as f32),
    )
}

fn monitor_request(api: &str) -> Result<Value, String> {
    RECORDER.with_borrow(|slot| match slot {
        Some(recorder) => recorder.call(&json!({"api": api})),
        None => Ok(Value::Null),
    })
}

pub fn preview(snapshot: &Value) -> Option<Value> {
    let pending = snapshot["pendingRecording"]["takePath"].as_str()?;
    let progress = dispatch("recording.preview").ok()?;
    (progress["filepath"].as_str() == Some(pending)).then_some(progress)
}

pub fn close() {
    RECORDER.with(|slot| {
        if let Some(recorder) = slot.borrow_mut().take() {
            let _ = recorder.call(&json!({"api":"recording.shutdown"}));
        }
    });
}
