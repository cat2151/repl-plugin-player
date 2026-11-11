//! Every try-* snippet runs twice through normal snippet execution without error.
//! Snippet content (notes, plugins, presets, order) is the user's to change, so it is not asserted.
use crate::{control::Control, snippets::Library};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub fn run(app: &mut Control) -> Result<(), String> {
    app.enable_gui();
    let entries = Library::default().load()?;
    let tutorials: Vec<_> = entries
        .iter()
        .filter(|s| s.group == "Built-in" && s.name.starts_with("try-"))
        .collect();
    if tutorials.is_empty() {
        return Err("No try-* snippets".into());
    }
    for snippet in tutorials {
        for repeat in 0..2 {
            let request = crate::snippet_insert::insert(&mut String::new(), 0, snippet);
            let output = app.run_request_result(&request)?;
            let result: Value = if output == "undefined" {
                Value::Null
            } else {
                serde_json::from_str(&output).map_err(|e| format!("{}: {e}", snippet.name))?
            };
            if let Some(job) = result.get("presetJobId").and_then(Value::as_u64) {
                wait_job(app, job).map_err(|e| format!("{}: {e}", snippet.name))?;
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

fn wait_job(app: &mut Control, job: u64) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = app.eval(&format!("uapmd.mutations.getJob({job})"), "preset job")?;
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
