use super::{max_peak, pump, wait_stopped};
use crate::control::Control;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

// Test-owned entry: built-in snippets may change without breaking this regression.
const SNIPPET: &str =
    "var {loopPhraseBeats, prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');
var loopTrack = include('builtin/lib/plugins.js').trackFor(globalThis.surgeId, 'snippet_00');
var loopNext = Object.assign({}, globalThis.loopParts || {});
loopNext.main = prepareLoopPart(loopTrack.trackIndex, \"cegb\");
publishLoopParts(loopNext, 120);
uapmd.audio.setEngineEnabled(true);
if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();";
const MAIN: &str = "cegb";

#[path = "dynamic.rs"]
mod dynamic;

/// Note-ons of one phrase as the MML converter produces them; MML meaning is not restated here.
pub(super) struct Phrase {
    pub(super) pitches: Vec<u64>,
    onsets: Vec<u64>,
    pub(super) period: u64,
}

impl Phrase {
    pub(super) fn pitch(&self, i: usize) -> u64 {
        self.pitches[i % self.pitches.len()]
    }
    /// Frame of the i-th note-on when the phrase repeats every `period` from frame zero.
    pub(super) fn at(&self, i: usize, period: u64) -> u64 {
        (i / self.pitches.len()) as u64 * period + self.onsets[i % self.pitches.len()]
    }
}

pub(super) fn phrase(app: &mut Control, sr: f64, mml: &str) -> Result<Phrase, String> {
    let code = format!(
        "(() => {{ var m = {mml:?}; var e = mmlabc_to_smf.pass3_events.ast_to_events(mmlabc_to_smf.pass2_ast.tokens_to_ast(mmlabc_to_smf.pass1_parser.parse_mml(cmrt_chord.resolve_chord_or_mml(m).mml)), true).filter(e => e.type === 'note_on'); return {{notes:e.map(e => [e.time, e.note]), beats:include('builtin/lib/loop.js').loopPhraseBeats(m)}}; }})()"
    );
    let value: Value =
        serde_json::from_str(&app.eval(&code, "reference phrase")?).map_err(|e| e.to_string())?;
    // The loop publishes at 120 BPM.
    let frames = |beats: f64| (beats * sr / 2.0).round() as u64;
    let notes = value["notes"].as_array().ok_or("Missing phrase notes")?;
    let field = |n: &Value, i: usize| n[i].as_f64().ok_or("Bad phrase note");
    Ok(Phrase {
        pitches: notes
            .iter()
            .map(|n| field(n, 1).map(|p| p as u64))
            .collect::<Result<_, _>>()?,
        onsets: notes
            .iter()
            .map(|n| field(n, 0).map(|t| frames(t / 480.0)))
            .collect::<Result<_, _>>()?,
        period: frames(value["beats"].as_f64().ok_or("Missing phrase beats")?),
    })
}

fn events(app: &Control) -> Result<Vec<Value>, String> {
    let trace = app.loop_trace(false)?;
    if trace["overflow"] != 0 || trace["programChanges"] != 0 {
        return Err("Plugin-input trace overflow or timing marker delivery".into());
    }
    Ok(trace["events"].as_array().ok_or("Missing trace")?.clone())
}

fn ons(events: &[Value]) -> Vec<(u64, u64)> {
    events
        .iter()
        .filter(|e| e["on"] == true)
        .map(|e| (e["frame"].as_u64().unwrap(), e["note"].as_u64().unwrap()))
        .collect()
}

fn stop(app: &mut Control) -> Result<(), String> {
    app.stop("E2E Stop loop")?;
    wait_stopped(app)?;
    let observed = app.observe("loop stopped")?;
    let later = pump(app, Duration::from_millis(250), "loop signal stopped")?;
    let last = later.last().ok_or("No stopped observations")?;
    if app.read_timeline()?["state"]["isPlaying"] != false
        || !app.read_notes()?.is_empty()
        || observed["engineEnabled"] != false
        || last["dispatcherPlaying"] != false
        || last["blocks"] != observed["blocks"]
    {
        return Err("Loop survived Stop".into());
    }
    Ok(())
}

pub fn run(app: &mut Control, update: bool) -> Result<(), String> {
    app.loop_checks()?;
    let sr = app.read_timeline()?["sampleRate"]
        .as_f64()
        .ok_or("Missing sample rate")?;
    let before = app.read_timeline()?["tracks"].clone();
    app.loop_trace(true)?;
    // First publication must also work when ordinary transport is already running.
    app.run("uapmd.audio.setEngineEnabled(true); uapmd.sequencer.startPlayback();")?;
    app.run(SNIPPET)?;
    let signal = pump(app, Duration::from_millis(6500), "fixed four-beat loop")?;
    if max_peak(&signal)? <= 0.001 {
        return Err("Loop produced no output signal".into());
    }
    let fixed = events(app)?;
    let notes = ons(&fixed);
    if notes.len() < 12 {
        return Err(format!("Too few loop notes: {notes:?}"));
    }
    let origin = notes[0].0;
    let main = phrase(app, sr, MAIN)?;
    for (i, (frame, note)) in notes.iter().enumerate() {
        if *note != main.pitch(i) || (*frame - origin).abs_diff(main.at(i, main.period)) > 1 {
            return Err(format!("Loop timing/sequence mismatch at {i}: {notes:?}"));
        }
    }
    if app.read_timeline()?["tracks"] != before {
        return Err("Loop changed existing clips or plugin chain".into());
    }
    app.log.event(
        "loop_delivery",
        json!({"sampleRate":sr,"events":fixed,"origin":origin}),
    )?;
    if update {
        updates(app, sr, origin, &main)?;
    }
    stop(app)?;
    // A rounded sample period with trailing rests, deliberately not a block multiple.
    app.loop_trace(true)?;
    let single = phrase(app, sr, "l16 crrr")?.pitch(0);
    let fractional = SNIPPET
        .replace(MAIN, "l16 crrr")
        .replace("publishLoopParts(loopNext, 120);", "repl.timeline.publishLoopClips(Object.keys(loopNext).map(k => loopNext[k]),1.234567,120); globalThis.loopParts=loopNext;");
    app.run(&fractional)?;
    pump(
        app,
        Duration::from_millis(3000),
        "fractional period with rest",
    )?;
    let fractional_events = events(app)?;
    let notes = ons(&fractional_events);
    let period = (1.234567 * sr / 2.0).round() as u64;
    if notes.len() < 4
        || notes.iter().any(|(_, n)| *n != single)
        || notes.windows(2).any(|pair| pair[1].0 - pair[0].0 != period)
    {
        return Err(format!("Fractional loop drift: period {period}, {notes:?}"));
    }
    app.log.event(
        "loop_fractional_delivery",
        json!({"period":period,"events":fractional_events}),
    )?;
    stop(app)?;
    if update {
        dynamic::automatic(app, sr)?;
        multiple_parts(app, sr)?;
    }
    tutorial_reset(app, sr)?;
    app.log.event("signal_check", json!({"loopPeak":max_peak(&signal)?,"fixedNotes":fixed.len(),"fractionalPeriod":period,"updates":update}))?;
    Ok(())
}

fn updates(app: &mut Control, sr: f64, origin: u64, main: &Phrase) -> Result<(), String> {
    let old = ons(&events(app)?).len();
    let next = phrase(app, sr, "dfac")?;
    app.run(&SNIPPET.replace(MAIN, "dfac"))?;
    // Invalid preparation must leave the submitted plan untouched.
    if app
        .run("repl.timeline.publishLoopClips([{trackIndex:0,filepath:'X:/missing-loop.mid'}])")
        .is_ok()
    {
        return Err("Invalid loop MIDI was accepted".into());
    }
    pump(app, Duration::from_millis(4400), "boundary update")?;
    let changed = events(app)?;
    let notes = ons(&changed);
    let first = notes
        .iter()
        .position(|(_, n)| *n == next.pitch(0))
        .ok_or("New loop never reached plugin")?;
    let cycle = main.pitches.len();
    if first != old.div_ceil(cycle) * cycle {
        return Err(format!("Mid-cycle loop update: {notes:?}"));
    }
    let expected_boundary = (notes[first].0 - origin) as f64 / main.period as f64;
    if (expected_boundary - expected_boundary.round()).abs() > 1.0 / sr {
        return Err("Update was not at common boundary".into());
    }
    for (i, (_, note)) in notes.iter().enumerate().skip(first) {
        if *note != next.pitch(i - first) {
            return Err(format!("Old/new cycle mixed: {notes:?}"));
        }
    }
    // Latest fully prepared request before a boundary wins.
    let settled = phrase(app, sr, "gggg")?.pitch(0);
    app.run(&SNIPPET.replace(MAIN, "eeee"))?;
    app.run(&SNIPPET.replace(MAIN, "gggg"))?;
    pump(app, Duration::from_millis(4300), "consecutive updates")?;
    let latest = ons(&events(app)?);
    if latest.iter().rev().take(4).any(|(_, n)| *n != settled) {
        return Err(format!("Consecutive updates did not settle: {latest:?}"));
    }
    app.run(&SNIPPET.replace(MAIN, "aaaa"))?;
    stop(app)?;
    pump(app, Duration::from_millis(250), "stopped reservation")?;
    if app.read_timeline()?["state"]["isPlaying"] != false {
        return Err("Stale update restarted transport".into());
    }
    app.log.event(
        "loop_update_delivery",
        json!({"events":changed,"boundary":expected_boundary,"latest":latest}),
    )?;
    Ok(())
}

fn multiple_parts(app: &mut Control, sr: f64) -> Result<(), String> {
    let second: i64 = serde_json::from_str(&app.eval(
        "globalThis.loopSecond = uapmd.instancing.create('CLAP','org.surge-synth-team.surge-xt',-1)",
        "create second loop instrument",
    )?).map_err(|e| e.to_string())?;
    let effect: i64 = serde_json::from_str(&app.eval(
        "globalThis.loopEffect = uapmd.instancing.create('CLAP','org.surge-synth-team.surge-xt-fx',0)",
        "create effect to preserve",
    )?).map_err(|e| e.to_string())?;
    if second < 0 || second == app.instance || effect < 0 {
        return Err("Multi-part fixture instances were not created".into());
    }
    app.run("uapmd.timeline.createEmptyMidiClip(0,12345,480,120)")?;
    let instrument_before = app.read_plugin_state(None, Instant::now() + Duration::from_secs(5))?;
    let effect_before =
        app.read_plugin_state(Some(effect), Instant::now() + Duration::from_secs(5))?;
    let topology = app.read_timeline()?["tracks"].clone();
    let base = phrase(app, sr, MAIN)?;
    let (updated_a, updated_b) = (
        phrase(app, sr, "ffff")?.pitch(0),
        phrase(app, sr, "aaaa")?.pitch(0),
    );
    app.loop_trace(true)?;
    let prepare = SNIPPET.replace(
        "uapmd.audio.setEngineEnabled(true);\nif (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();",
        "",
    );
    app.run(&prepare)?;
    app.run("var secondTrack = uapmd.sequencer.getTrackInfos().find(t => t.nodes.some(n => n.instanceId === loopSecond));\nvar secondNext = Object.assign({},loopParts); secondNext.second = prepareLoopPart(secondTrack.trackIndex,'dddd'); publishLoopParts(secondNext, 120);\nuapmd.audio.setEngineEnabled(true); uapmd.sequencer.startPlayback();")?;
    let initial_signal = pump(app, Duration::from_millis(2700), "two common-phase parts")?;
    // Both parts are prepared before the one publication operation.
    app.run("var mainNext = getGeneratedMidiPath(); var secondNext = getGeneratedMidiPath();\nmmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('ffff'),mainNext);\nmmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('aaaa'),secondNext);\nrepl.timeline.publishLoopClips([{trackIndex:loopParts.main.trackIndex,filepath:mainNext},{trackIndex:secondTrack.trackIndex,filepath:secondNext}]);")?;
    pump(app, Duration::from_millis(4200), "atomic multi-part update")?;
    let delivered = events(app)?;
    let timing = app.loop_trace(false)?["timing"].clone();
    let lead = |instance: i64| -> Result<u64, String> {
        timing
            .as_array()
            .ok_or("Missing render timing")?
            .iter()
            .find(|t| {
                t["instances"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| *id == instance))
            })
            .and_then(|t| t["lead"].as_u64())
            .ok_or_else(|| "Missing track render lead".into())
    };
    let a_lead = lead(app.instance)?;
    let b_lead = lead(second)?;
    let a = ons(&delivered
        .iter()
        .filter(|e| e["instance"] == app.instance)
        .cloned()
        .collect::<Vec<_>>());
    let b = ons(&delivered
        .iter()
        .filter(|e| e["instance"] == second)
        .cloned()
        .collect::<Vec<_>>());
    let ia = a
        .iter()
        .position(|(_, n)| *n == updated_a)
        .ok_or("Main part update missing")?;
    let ib = b
        .iter()
        .position(|(_, n)| *n == updated_b)
        .ok_or("Second part update missing")?;
    if a.len() < 12
        || b.len() < 12
        || ia == 0
        || ia % base.pitches.len() != 0
        || ib != ia
        || a_lead == b_lead
        || a[ia].0 + a_lead != b[ib].0 + b_lead
        || a.iter().zip(&b).any(|(a, b)| a.0 + a_lead != b.0 + b_lead)
        || a.iter().skip(ia).any(|(_, n)| *n != updated_a)
        || b.iter().skip(ib).any(|(_, n)| *n != updated_b)
        || (a[ia].0 - a[0].0).abs_diff((ia / base.pitches.len()) as u64 * base.period) > 1
    {
        return Err(format!("Multi-part phase/update mismatch: {a:?}, {b:?}"));
    }
    let main = app.instance;
    dynamic::multiple(app, sr, main, second, a_lead, b_lead)?;
    stop(app)?;
    let instrument_after = app.read_plugin_state(None, Instant::now() + Duration::from_secs(5))?;
    let effect_after =
        app.read_plugin_state(Some(effect), Instant::now() + Duration::from_secs(5))?;
    if app.read_timeline()?["tracks"] != topology
        || instrument_before != instrument_after
        || effect_before != effect_after
    {
        return Err("Loop update changed existing clips, instrument or effect state".into());
    }
    let status: Value =
        serde_json::from_str(&app.eval("repl.timeline.loopStatus()", "loop delivery overflow")?)
            .map_err(|e| e.to_string())?;
    if status["dropped"].as_f64() != Some(0.0) || max_peak(&initial_signal)? <= 0.001 {
        return Err(format!(
            "Multi-part delivery overflow or missing signal: {status}"
        ));
    }
    app.log.event(
        "loop_multiple_parts",
        json!({"main":a,"second":b,"trackTiming":timing,"statePreserved":true,"status":status}),
    )?;
    Ok(())
}

/// After the tutorial reset, a later phrase longer than the old loop plays through without folding.
fn tutorial_reset(app: &mut Control, sr: f64) -> Result<(), String> {
    const LATER: &str = "l2 cde";
    let later = phrase(app, sr, LATER)?;
    app.loop_trace(true)?;
    app.run(SNIPPET)?;
    pump(app, Duration::from_millis(1200), "loop before reset")?;
    if ons(&events(app)?).is_empty() {
        return Err("Loop did not play before reset".into());
    }
    app.run("include('builtin/lib/tutorial.js').reset();")?;
    // The input trace restarts only with audio stopped; a stale loop plan would survive this too.
    stop(app)?;
    app.loop_trace(true)?;
    app.run(&format!(
        "for (var t of uapmd.sequencer.getTrackInfos()) for (var n of t.nodes.filter(n => n.instanceId >= 0)) uapmd.sequencer.setPluginBypassed(n.instanceId, false);
var laterPath = getGeneratedMidiPath(); mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes({LATER:?}), laterPath);
var laterTrack = include('builtin/lib/plugins.js').trackFor(globalThis.surgeId, 'snippet_00');
var laterClip = uapmd.timeline.addMidiClip(laterTrack.trackIndex, 0, laterPath); if (!laterClip.success) throw new Error(laterClip.error);
uapmd.audio.setEngineEnabled(true); uapmd.sequencer.startPlayback();"
    ))?;
    let span = later.at(later.pitches.len() - 1, later.period) as f64 / sr;
    pump(
        app,
        Duration::from_secs_f64(span + 0.8),
        "later phrase after reset",
    )?;
    let state = app.read_timeline()?["state"].clone();
    let after = events(app)?;
    let mut notes = ons(&after
        .iter()
        .filter(|e| e["instance"] == app.instance)
        .cloned()
        .collect::<Vec<_>>());
    // Folding never reaches the later pitches; repeats of one pitch before the next are ignored.
    notes.dedup_by_key(|(_, n)| *n);
    let parts = app.eval("globalThis.loopParts === undefined", "loop map after reset")?;
    if state["loopEnabled"] != false
        || state["loopEndSamples"].as_f64() != Some(0.0)
        || parts != "true"
        || notes.iter().map(|(_, n)| *n).collect::<Vec<_>>() != later.pitches
    {
        return Err(format!("Loop survived reset: {state}, {notes:?}"));
    }
    app.log
        .event("loop_reset", json!({"state":state,"events":after}))?;
    stop(app)
}
