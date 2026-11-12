//! `uapmd.instance(i).loadState(file)` called from a cell returns, and the
//! instance takes the file as its whole state; an unreadable file fails as a
//! `{success:false}` result. Test-owned cells on the prepared instance:
//! `state-load.js` beside this file.
use crate::{control::Control, e2e_script::Script};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// The hang it guards against does not return for minutes; a working load takes well under one second.
const LIMIT: Duration = Duration::from_secs(10);

fn eval_json(app: &mut Control, code: &str, reason: &str) -> Result<Value, String> {
    serde_json::from_str(&app.eval(code, reason)?).map_err(|e| format!("{reason}: {e}"))
}

fn value(app: &mut Control, script: &Script, instance: i64, parameter: i64) -> Result<f64, String> {
    let args = json!({"instance":instance, "parameter":parameter});
    eval_json(
        app,
        &script.call("parameter value", &args)?,
        "state-load parameter",
    )?
    .as_f64()
    .ok_or_else(|| "Missing parameter value".into())
}

/// Waits for the parameter to reach `target`; some plugins apply state on their audio thread.
fn settles(
    app: &mut Control,
    script: &Script,
    instance: i64,
    parameter: i64,
    target: f64,
) -> Result<f64, String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let current = value(app, script, instance, parameter)?;
        if (current - target).abs() < 1e-4 || Instant::now() >= deadline {
            return Ok(current);
        }
        app.poll()?;
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn load(
    app: &mut Control,
    script: &Script,
    instance: i64,
    file: &str,
) -> Result<(Value, u128), String> {
    let code = script.call("load", &json!({"instance":instance, "file":file}))?;
    let start = Instant::now();
    let result = eval_json(app, &code, "state-load loadState")?;
    Ok((result, start.elapsed().as_millis()))
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    let instance = super::instrument(app, script)?;
    app.eval(script.cell("engine on")?, "state-load engine")?;
    let parameter = eval_json(
        app,
        &script.call("continuous parameter", &json!({"instance":instance}))?,
        "state-load parameter id",
    )?
    .as_f64()
    .ok_or("No continuous parameter")? as i64;
    let before = value(app, script, instance, parameter)?;
    let bytes = app.read_plugin_state(instance, Instant::now() + LIMIT)?;
    let dir = std::env::temp_dir().join("repl-plugin-player");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("state-load-regression.bin");
    std::fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    let file = file.to_string_lossy().into_owned();

    let moved = if before < 0.5 { 0.9 } else { 0.1 };
    let set = json!({"instance":instance, "parameter":parameter, "value":moved});
    app.eval(
        &script.call("set parameter", &set)?,
        "state-load move parameter",
    )?;
    let changed = settles(app, script, instance, parameter, moved)?;
    if (changed - before).abs() < 1e-4 {
        return Err(format!("Parameter {parameter} did not move from {before}"));
    }

    let (loaded, ms) = load(app, script, instance, &file)?;
    if loaded["success"] != true || ms > LIMIT.as_millis() {
        return Err(format!("loadState failed or was slow ({ms} ms): {loaded}"));
    }
    let restored = settles(app, script, instance, parameter, before)?;
    if (restored - before).abs() >= 1e-4 {
        return Err(format!(
            "loadState did not restore parameter {parameter}: {before} -> {changed} -> {restored}"
        ));
    }

    let missing = dir.join("state-load-regression-missing.bin");
    let _ = std::fs::remove_file(&missing);
    let (failed, failed_ms) = load(app, script, instance, &missing.to_string_lossy())?;
    if failed["success"] != false || failed["error"].as_str().is_none_or(str::is_empty) {
        return Err(format!("Missing state file did not fail: {failed}"));
    }
    app.log.event(
        "state_load_verified",
        json!({"parameter":parameter,"before":before,"changed":changed,"restored":restored,
            "loadMs":ms,"stateBytes":bytes.len(),"missing":failed,"missingMs":failed_ms}),
    )?;
    Ok(())
}
