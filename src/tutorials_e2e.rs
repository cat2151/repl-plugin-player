//! Every try-* snippet runs twice through normal snippet execution without error, even
//! when another plugin has replaced track 0's instrument first.
//! Snippet content (notes, plugins, presets, order) is the user's to change, so it is not asserted.
//! A snippet that browses patches meets a stand-in player whose browser closes at once.
//! The test-owned cells are `tests/e2e/tutorials.js`.
use crate::{
    control::Control,
    e2e_script::Script,
    patch_browser::Link,
    resident_ipc::{self, ToEditor, ToPlayer},
    snippets::Library,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    process::Child,
    time::{Duration, Instant},
};

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    app.enable_gui();
    crate::patch_browse::start_with(|| Link::start(Path::new("stand-in"), launch));
    let entries = Library::default().load()?;
    let tutorials: Vec<_> = entries
        .iter()
        .filter(|s| s.group == "Built-in" && s.name.starts_with("try-"))
        .collect();
    if tutorials.is_empty() {
        return Err("No try-* snippets".into());
    }
    for snippet in tutorials {
        // The user may have chosen another plugin's patch before running a snippet.
        app.eval(
            script.cell("other instrument")?,
            "replace track 0's instrument",
        )
        .map_err(|e| format!("before {}: {e}", snippet.name))?;
        for repeat in 0..2 {
            let request = crate::snippet_insert::insert(&mut String::new(), 0, snippet);
            let output = app.run_request_result(&request)?;
            let result: Value = if output == "undefined" {
                Value::Null
            } else {
                serde_json::from_str(&output).map_err(|e| format!("{}: {e}", snippet.name))?
            };
            if let Some(job) = result.get("presetJobId").and_then(Value::as_u64) {
                wait_job(app, script, job).map_err(|e| format!("{}: {e}", snippet.name))?;
            }
            let until = Instant::now() + Duration::from_millis(300);
            while Instant::now() < until {
                app.pump_cli_messages()?;
                app.poll()?;
                std::thread::sleep(Duration::from_millis(25));
            }
            app.log.event(
                "tutorial_verified",
                json!({"name":snippet.name,"repeat":repeat}),
            )?;
        }
    }
    Ok(())
}

fn wait_job(app: &mut Control, script: &Script, job: u64) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let code = script.call("job", &json!({ "job": job }))?;
    loop {
        let status = app.eval(&code, "preset job")?;
        let status: Value = serde_json::from_str(&status).map_err(|e| e.to_string())?;
        match status["state"].as_str() {
            Some("completed") => return Ok(()),
            Some("failed") => return Err(format!("preset job failed: {status}")),
            _ if Instant::now() > deadline => {
                return Err(format!("preset job timed out: {status}"))
            }
            _ => {
                app.pump_cli_messages()?;
                app.poll()?;
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn launch(_: &Path, name: &str) -> std::io::Result<Option<Child>> {
    let name = name.to_owned();
    std::thread::spawn(move || closes_at_once(&name));
    Ok(None)
}

/// Answers every `Open` with the browser closed, until the app drops the link.
fn closes_at_once(name: &str) {
    let Ok((mut out, mut input)) = resident_ipc::connect(name) else {
        return;
    };
    while let Ok(Some(message)) = input.receive::<ToPlayer>() {
        if let ToPlayer::Open(target) = message {
            if out.send(&ToEditor::BrowserClosed { target }).is_err() {
                return;
            }
        }
    }
}
