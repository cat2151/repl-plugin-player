#[path = "../patches/uapmd/loop-playback/regression.rs"]
mod loop_playback;
#[path = "../patches/uapmd/note-monitor/regression.rs"]
mod note_monitor;
#[path = "../patches/uapmd/script-api/regression.rs"]
mod script_api;

use crate::{
    control::{Control, SURGE},
    log,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

// Retain the immediate-note regression suite independently of the timeline sample.
const SAMPLE: &str =
    "uapmd.audio.setEngineEnabled(true);\nuapmd.sequencer.sendNoteOn(globalThis.surgeId, 60);";
/// Test-owned cells: track lookup with a marked reset, one clip, then playback.
pub(crate) const TIMELINE: &str = include_str!("e2e-timeline.js");

fn pump(app: &mut Control, duration: Duration, window: &str) -> Result<Vec<Value>, String> {
    let until = Instant::now() + duration;
    let mut samples = Vec::new();
    while Instant::now() < until {
        app.poll()?;
        samples.push(app.observe(window)?);
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(samples)
}
pub fn run(case: &str) -> Result<(), String> {
    if ![
        "sample",
        "cold",
        "rerun",
        "stop",
        "close",
        "syntax",
        "missing",
        "close-preparing",
        "close-creating",
        "mutation-close",
        "syntax-sustain",
        "runtime-error",
        "stop-run",
        "timeline",
        "timeline-view",
        "note-monitor",
        "scope",
        "partial-run",
        "snippets",
        "tutorials",
        "demo-play",
        "mml-clip",
        "chord-clip",
        "script-ui",
        "loop",
        "loop-update",
    ]
    .contains(&case)
    {
        return Err(format!("Unknown internal E2E case: {case}"));
    }
    let log = log::Log::new()?;
    let mut app = Control::open(
        log,
        if case == "missing" {
            "org.repl-plugin-player.deliberately-missing"
        } else {
            SURGE
        },
    )?;
    let result = (|| {
        if case == "close-preparing" {
            return Ok(());
        }
        while !app.ready {
            if let Err(error) = app.poll() {
                if case == "missing" && error.contains("not found in official catalog") {
                    app.log
                        .event("expected_error", json!({"stage":"catalog","error":error}))?;
                    return Ok(());
                }
                return Err(error);
            }
            if case == "close-creating" && app.creation_requested() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if case == "missing" {
            return Err("Missing target unexpectedly became Ready".into());
        }
        if case == "syntax" {
            let error = app
                .run("uapmd.audio.setEngineEnabled(true); const = ;")
                .expect_err("Invalid script must fail");
            app.log
                .event("expected_error", json!({"stage":"script","error":error}))?;
            let observed = app.observe("syntax error leaves engine state unchanged")?;
            if observed["engineEnabled"] != false {
                return Err("Syntax error executed the preceding engine enable".into());
            }
            return Ok(());
        }
        if case == "mutation-close" {
            app.run("uapmd.sequencer.addTrack()")?;
            return Ok(());
        }
        if case == "timeline" {
            return timeline_case(&mut app);
        }
        if case == "timeline-view" {
            return crate::timeline::regression::run(&mut app);
        }
        if case == "loop" || case == "loop-update" {
            return loop_playback::run(&mut app, case == "loop-update");
        }
        if case == "note-monitor" {
            return note_monitor::run(&mut app);
        }
        if case == "scope" {
            return crate::scope_e2e::run(&mut app);
        }
        if case == "partial-run" {
            return crate::execution_e2e::run(&mut app);
        }
        if case == "snippets" {
            return crate::snippets_e2e::run(&mut app);
        }
        if case == "tutorials" {
            return crate::tutorials_e2e::run(&mut app);
        }
        if case == "demo-play" {
            return crate::demo_play_e2e::run(&mut app);
        }
        if case == "script-ui" {
            return crate::script_ui::regression::run(&mut app);
        }
        if case == "mml-clip" || case == "chord-clip" {
            return script_api::run(&mut app, case == "chord-clip");
        }
        let mut baseline_peak = 0.0;
        if case != "cold" {
            app.observe("initial history snapshot; not baseline")?;
            app.eval("uapmd.audio.setEngineEnabled(true)", "silent baseline")?;
            let baseline = pump(&mut app, Duration::from_millis(500), "baseline")?;
            baseline_peak = max_peak(&baseline)?;
            if baseline_peak > 0.000001 {
                return Err(format!("Baseline not silent: {baseline_peak}"));
            }
        }
        // Cold first Run has no machine-only engine prelude.
        app.run(SAMPLE)?;
        if case == "stop" || case == "close" {
            let held = pump(
                &mut app,
                Duration::from_millis(250),
                "before explicit Stop / close",
            )?;
            if max_peak(&held)? <= 0.001 {
                return Err("No note before Stop/close".into());
            }
            if case == "close" {
                return Ok(());
            }
            app.stop("E2E Stop")?;
            wait_stopped(&mut app)?;
            let first = app.observe("stopped; stale analyser values ignored")?;
            std::thread::sleep(Duration::from_millis(200));
            let last = app.observe("stopped; counter must remain unchanged")?;
            if last["engineEnabled"] != false
                || last["dispatcherPlaying"] != false
                || first["blocks"] != last["blocks"]
            {
                return Err("Engine/device did not remain stopped".into());
            }
            return Ok(());
        }
        let first = pump(&mut app, Duration::from_millis(300), "first Run")?;
        if max_peak(&first)? <= 0.001 {
            return Err("First Run has no note signal".into());
        }
        if case == "rerun" {
            // A second evaluation changes JS state without retriggering the note.
            app.run("globalThis.rerunMarker = 42")?;
        }
        if case == "syntax-sustain" || case == "runtime-error" {
            let code = if case == "syntax-sustain" {
                "const = ;"
            } else {
                "globalThis.errorSideEffect = 42; throw new Error('expected E2E error')"
            };
            let error = app.run(code).expect_err("Invalid script must fail");
            app.log
                .event("expected_error", json!({"stage":"script","error":error}))?;
            if case == "runtime-error"
                && app.eval("globalThis.errorSideEffect === 42", "E2E side-effect check")? != "true"
            {
                return Err("Runtime error lost preceding JS side effect".into());
            }
            app.run("globalThis.afterError = true")?;
        }
        if case == "stop-run" {
            app.stop("E2E Stop before new Run")?;
            wait_stopped(&mut app)?;
            app.run(SAMPLE)?;
        }
        // Observe beyond the removed one-second deadline; use a later window
        // with advancing blocks so analyser history cannot masquerade as sustain.
        pump(
            &mut app,
            Duration::from_millis(1200),
            "sustain past one second",
        )?;
        let held = pump(
            &mut app,
            Duration::from_millis(250),
            "late sustained signal",
        )?;
        let held_peak = max_peak(&held)?;
        if held_peak <= 0.001 {
            return Err(format!(
                "Note did not sustain across Run/error/time: {held_peak}"
            ));
        }
        if held.last().unwrap()["blocks"].as_u64() <= first.last().unwrap()["blocks"].as_u64() {
            return Err("Realtime block counter did not advance".into());
        }
        // The test, rather than Control, explicitly releases its note through Run.
        app.run("uapmd.sequencer.sendNoteOff(globalThis.surgeId, 60)")?;
        pump(
            &mut app,
            Duration::from_millis(500),
            "explicit NoteOff release settling",
        )?;
        let released = pump(&mut app, Duration::from_millis(500), "release window")?;
        let released_peak = max_peak(&released)?;
        if released_peak > 0.00001 {
            return Err(format!("Explicit NoteOff did not decay: {released_peak}"));
        }
        app.stop("E2E complete")?;
        wait_stopped(&mut app)?;
        app.log.event("signal_check", json!({"baselinePeak":baseline_peak,"heldPeak":held_peak,"releasedPeak":released_peak,"deviceOutputMeasured":false}))?;
        Ok(())
    })();
    if let Err(error) = &result {
        app.log
            .event("error", json!({"stage":"E2E","error":error}))?;
    }
    app.close()?;
    if result.is_ok() {
        app.log
            .event("e2e_pass", json!({"case":case,"cleanup":"complete"}))?;
    }
    result
}

fn timeline_case(app: &mut Control) -> Result<(), String> {
    app.run("globalThis.retainedValue = 42")?;
    app.run(TIMELINE)?;
    let first = app.read_timeline()?;
    let clips = first["tracks"][0]["clips"]
        .as_array()
        .ok_or("No clip array")?;
    if clips.len() != 1 {
        return Err(format!("Expected one clip: {first}"));
    }
    let midi = &clips[0]["midi"];
    let tpqn = midi["tickResolution"].as_u64().ok_or("No TPQN")?;
    let notes = crate::timeline::notes(&midi["events"]);
    if notes.len() != 1
        || notes[0].pitch != 60
        || notes[0].start != tpqn
        || notes[0].end != Some(3 * tpqn)
    {
        return Err(format!("Wrong model note: {midi}"));
    }
    let before = pump(app, Duration::from_millis(300), "timeline before Note On")?;
    let held = pump(app, Duration::from_millis(900), "timeline Note On window")?;
    pump(
        app,
        Duration::from_millis(900),
        "timeline Note Off and release",
    )?;
    let release = pump(app, Duration::from_millis(400), "timeline released")?;
    if max_peak(&before)? > 1e-6 || max_peak(&held)? <= 1e-3 || max_peak(&release)? > 1e-5 {
        return Err(format!(
            "Timeline signal failed: before={} held={} release={}",
            max_peak(&before)?,
            max_peak(&held)?,
            max_peak(&release)?
        ));
    }
    app.run(TIMELINE)?;
    let repeated = app.read_timeline()?;
    if repeated["tracks"][0]["clips"].as_array().map(Vec::len) != Some(1) {
        return Err("Explicit reset did not keep one clip".into());
    }
    let code = TIMELINE;
    let start = code.find("// BEGIN RESET").ok_or("No reset marker")?;
    let end = code.find("// END RESET").ok_or("No reset end marker")?;
    let accumulating = format!("{}{}", &code[..start], &code[end..]);
    app.run(&accumulating)?;
    let accumulated = app.read_timeline()?;
    if accumulated["tracks"][0]["clips"].as_array().map(Vec::len) != Some(2) {
        return Err("Without reset the clips did not accumulate".into());
    }
    for _ in 0..3 {
        let read = app.read_timeline()?;
        if read["tracks"] != accumulated["tracks"] {
            return Err("Display read changed model content".into());
        }
    }
    if app.eval("globalThis.retainedValue", "verify retained REPL global")? != "42" {
        return Err("Run unexpectedly reset globals".into());
    }
    let failed =
        app.run("uapmd.timeline.createEmptyMidiClip(0, 0); throw new Error('after model edit');");
    if failed.is_ok()
        || app.read_timeline()?["tracks"][0]["clips"]
            .as_array()
            .map(Vec::len)
            != Some(3)
    {
        return Err("Partial model edits were hidden or rolled back after an error".into());
    }
    app.stop("E2E Stop")?;
    wait_stopped(app)?;
    let stopped = app.read_timeline()?;
    if stopped["state"]["isPlaying"] != false
        || stopped["tracks"][0]["clips"].as_array().map(Vec::len) != Some(3)
    {
        return Err("Stop discarded clips or left transport playing".into());
    }
    app.log.event("signal_check", json!({"beforePeak":max_peak(&before)?, "heldPeak":max_peak(&held)?, "releasePeak":max_peak(&release)?, "resetClipCount":1,"accumulatedClipCount":2,"afterErrorClipCount":3}))?;
    Ok(())
}
fn wait_stopped(app: &mut Control) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(10);
    while app.stopping {
        app.poll()?;
        if Instant::now() > until {
            return Err("Engine stop timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn max_peak(values: &[Value]) -> Result<f64, String> {
    let mut peak: f64 = 0.0;
    for value in values {
        if value["finite"] != true
            || value["dispatcherPlaying"] != true
            || value["engineEnabled"] != true
        {
            return Err(format!("Invalid observation: {value}"));
        }
        peak = peak.max(value["peak"].as_f64().ok_or("Missing peak")?);
    }
    Ok(peak)
}
