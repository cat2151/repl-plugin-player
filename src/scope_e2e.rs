//! The scope and the keyboard display follow the instrument on track 0; the
//! cells are `tests/e2e/scope.js`.
use crate::{control::Control, e2e::instrument, e2e_script::Script, scope_trigger};
use std::time::{Duration, Instant};

fn wait_note(app: &mut Control, note: u8) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.poll()?;
        if let Some(frame) = app.scope().and_then(|s| s.display()) {
            if frame.note == note
                && frame.samples.len() == scope_trigger::width(frame.sample_rate, note)
                && frame.samples.iter().flatten().all(|v| v.is_finite())
                && frame.samples.iter().flatten().any(|v| v.abs() > 0.001)
            {
                app.log.event(
                    "scope_verified",
                    serde_json::json!({"note":note,
                    "samples":frame.samples.len(), "sampleRate":frame.sample_rate}),
                )?;
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            let frame = app.scope().and_then(|s| s.display());
            return Err(format!(
                "No scope waveform for note {note}; latest {:?}",
                frame.map(|f| (
                    f.note,
                    f.samples
                        .iter()
                        .flatten()
                        .fold(0.0_f32, |p, v| p.max(v.abs()))
                ))
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The keyboard display shows `note` held at the instruments' input.
fn wait_held(app: &mut Control, note: u8) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !app.read_notes()?.contains(&note) {
        if Instant::now() >= deadline {
            return Err(format!("Note {note} never reached the keyboard display"));
        }
        app.poll()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

/// The scope follows the instrument on track 0.
fn scope_on_instrument(app: &mut Control, script: &Script) -> Result<i64, String> {
    let id = instrument(app, script)?;
    let target = app.eval(script.cell("scope target")?, "E2E: scope target")?;
    if target.parse::<f64>().ok() != Some(id as f64) {
        return Err(format!("Scope shows instance {target}, track 0 holds {id}"));
    }
    Ok(id)
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    let first = scope_on_instrument(app, script)?;
    let plugins = app.eval(
        script.cell("plugins on track 0")?,
        "E2E: plugins on track 0 at startup",
    )?;
    if plugins.parse::<f64>().ok() != Some(1.0) {
        return Err(format!(
            "Track 0 starts with {plugins} plugins, not one instrument"
        ));
    }
    app.run(script.cell("note 60")?)?;
    wait_held(app, 60)?;
    wait_note(app, 60)?;
    app.run(script.cell("note 72 after 60")?)?;
    wait_note(app, 72)?;
    stop(app)?;
    app.run(script.cell("note 67 with the engine on")?)?;
    wait_note(app, 67)?;
    app.run(script.cell("release 67")?)?;
    app.run(script.cell("replace the instrument")?)?;
    let replaced = scope_on_instrument(app, script)?;
    if replaced == first {
        return Err("The instrument on track 0 was not replaced".into());
    }
    app.run(script.cell("note 64")?)?;
    wait_held(app, 64)?;
    wait_note(app, 64)?;
    app.run(script.cell("easy reset and play")?)?;
    scope_on_instrument(app, script)?;
    wait_note(app, 60)?;
    app.log.event(
        "signal_check",
        serde_json::json!({"startupInstrument":first, "replacedInstrument":replaced}),
    )?;
    Ok(())
}

pub fn run_master(app: &mut Control, script: &Script) -> Result<(), String> {
    app.run(script.cell("play evolve master")?)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.poll()?;
        if app.scope().and_then(|s| s.display()).is_some_and(|frame| {
            frame.note == 129
                && frame.samples.len() == frame.sample_rate as usize / 50
                && frame.samples.iter().flatten().all(|v| v.is_finite())
                && frame.samples.iter().flatten().any(|v| v.abs() > 0.001)
        }) {
            app.log
                .event("master_scope_verified", serde_json::json!({}))?;
            break;
        }
        if Instant::now() >= deadline {
            return Err("No final master scope waveform".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    app.run(script.cell("restore scope")?)?;
    Ok(())
}

pub fn run_load(app: &mut Control, script: &Script) -> Result<(), String> {
    scope_on_instrument(app, script)?;
    let archive = std::env::temp_dir().join(format!("repl-scope-{}.uapmdz", std::process::id()));
    app.eval(
        &format!(
            "globalThis.scopeProjectPath = {}",
            serde_json::json!(archive)
        ),
        "E2E: scope archive path",
    )?;
    let loaded = app.run(script.cell("reload project")?);
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_file(format!("{}.empty.uapmdz", archive.display()));
    loaded?;
    scope_on_instrument(app, script)?;
    app.run(script.cell("play loaded project")?)?;
    wait_held(app, 67)?;
    wait_note(app, 67)?;
    stop(app)?;
    app.run(script.cell("freeze loaded project")?)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while app.eval(script.cell("frozen ready")?, "E2E: freeze ready")? != "true" {
        app.poll()?;
        if Instant::now() >= deadline {
            return Err("Scope track freeze timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    app.run(script.cell("play loaded project")?)?;
    wait_held(app, 69)?;
    wait_note(app, 69)?;
    Ok(())
}

fn stop(app: &mut Control) -> Result<(), String> {
    app.stop("scope restart check")?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.stopping {
        app.poll()?;
        if Instant::now() >= deadline {
            return Err("Scope stop timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
