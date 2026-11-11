//! Test-owned phrases/settings: editable snippet contents are never expectations.
use crate::control::Control;
use serde_json::json;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

const INIT: &str = "var testDemo = include('builtin/lib/tutorial.js');
testDemo.init({instanceId:globalThis.surgeId, preset:'Init Saw', bpm:120});";

fn check_play(app: &mut Control, source: &str, initialize: bool) -> Result<(), String> {
    let pitches: Vec<u8> = serde_json::from_str(&app.eval(
        &format!("mmlabc_to_smf.pass3_events.ast_to_events(mmlabc_to_smf.pass2_ast.tokens_to_ast(mmlabc_to_smf.pass1_parser.parse_mml(cmrt_chord.resolve_chord_or_mml({source:?}).mml)), true).filter(e => e.type === 'note_on').map(e => e.note)"),
        "demo reference pitches",
    )?).map_err(|e| e.to_string())?;
    let first = *pitches.first().ok_or("No reference notes")?;
    app.run(&format!(
        "{}testDemo.play({source:?})",
        if initialize { INIT } else { "" }
    ))?;
    let state = app.read_timeline()?;
    let clips = state["tracks"][0]["clips"]
        .as_array()
        .ok_or("Missing demo clips")?;
    if clips.len() != 1 {
        return Err(format!("Demo did not replace its clip: {state}"));
    }
    let notes = crate::timeline::notes(&clips[0]["midi"]["events"]);
    if notes.first().is_none_or(|n| n.start != 0) {
        return Err("Demo inserted a leading rest".into());
    }
    let tpqn = clips[0]["midi"]["tickResolution"]
        .as_f64()
        .ok_or("Missing TPQN")?;
    let end = notes
        .iter()
        .filter_map(|n| n.end)
        .max()
        .ok_or("No note ends")?;
    let deadline = Instant::now() + Duration::from_secs_f64(end as f64 / tpqn * 0.5 + 0.2);
    let mut delivered = BTreeSet::new();
    let mut first_peak = 0.0_f64;
    while Instant::now() < deadline {
        app.poll()?;
        let active = app.read_notes()?;
        let signal = app.observe("demo play: actual plugin input and master output")?;
        if signal["finite"] != true {
            return Err("Non-finite demo output".into());
        }
        if active.contains(&first) {
            first_peak = first_peak.max(signal["peak"].as_f64().ok_or("Missing peak")?);
        }
        delivered.extend(active);
        std::thread::sleep(Duration::from_millis(5));
    }
    if delivered != pitches.iter().copied().collect() || first_peak <= 0.001 {
        return Err(format!("Missing demo notes/first-note signal: {source:?}, expected {pitches:?}, delivered {delivered:?}, first peak {first_peak}"));
    }
    if app.eval(
        "uapmd.timeline.getState().loopEnabled || typeof globalThis.loopParts !== 'undefined'",
        "demo clears loop",
    )? != "false"
    {
        return Err("Demo retained loop playback".into());
    }
    app.log.event(
        "demo_play_verified",
        json!({"source":source,"pitches":pitches,"firstPeak":first_peak,"initialize":initialize}),
    )?;
    Ok(())
}

pub fn run(app: &mut Control) -> Result<(), String> {
    // First run and reinitialization in one evaluation, with no leading rest.
    check_play(app, "cfg", true)?;
    check_play(app, "cfg", true)?;
    // Edits made after init (like another cell or the plugin UI) survive play.
    app.run("uapmd.audio.setEngineEnabled(false); var testPresetJob = testDemo.preparePreset(surgeId, 'Attacky');")?;
    let until = Instant::now() + Duration::from_secs(10);
    while app.eval("(() => { var j = uapmd.mutations.getJob(testPresetJob.presetJobId); if (j.state === 'failed') throw new Error(j.error); return j.state === 'completed'; })()", "preset completion before editing")? != "true" {
        if Instant::now() >= until { return Err("Demo test preset timed out".into()); }
        app.poll()?;
        std::thread::sleep(Duration::from_millis(5));
    }
    app.run("var testVolume = uapmd.instance(surgeId).getParameters().find(p=>p.name === 'Global Volume');
        uapmd.instance(surgeId).setParameterValue(testVolume.id, 0.8);
        var testControls = uapmd.instance(surgeId).getParameters().filter(p=>!p.isReadonly);
        var testValues = () => testControls.map(p=>uapmd.instance(surgeId).getParameterValue(p.id));
        var testBefore = JSON.stringify(testValues());")?;
    check_play(app, "F-G", false)?;
    if app.eval(
        "testBefore === JSON.stringify(testValues())",
        "play preserves sound and volume",
    )? != "true"
    {
        let changed = app.eval("testControls.map((p,i)=>({id:p.id,name:p.name,before:JSON.parse(testBefore)[i],after:testValues()[i]})).filter(p=>p.before!==p.after)", "changed parameters")?;
        return Err(format!("play changed instrument parameters: {changed}"));
    }
    // Init explicitly loads its chosen patch again; play alone must not do that.
    check_play(app, "cfg", true)?;
    // Replacing an actively playing clip, then entering from an active loop.
    app.run("testDemo.play('c1')")?;
    check_play(app, "cfg", false)?;
    app.run(
        "var testLoop = include('builtin/lib/loop.js');
        var testTrack = include('builtin/lib/plugins.js').trackFor(surgeId).trackIndex;
        testLoop.publishLoopParts({test:testLoop.prepareLoopPart(testTrack, 'c1')}, 120);
        uapmd.audio.setEngineEnabled(true); uapmd.sequencer.startPlayback();",
    )?;
    check_play(app, "cfg", false)?;
    app.log.event("signal_check", json!({"demoPlay":true,"firstNotesVerified":true,"soundAndVolumePreserved":true,"deviceOutputMeasured":false}))?;
    Ok(())
}
