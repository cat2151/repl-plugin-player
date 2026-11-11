//! Generate an SMF clip from MML or a chord progression and play it, from Ready.
use crate::control::Control;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Test-owned entry; `{source}` is the chord progression or MML under test.
const ENTRY: &str =
    "var generatedTrack = include('builtin/lib/plugins.js').trackFor(globalThis.surgeId);
var midi = include('builtin/lib/midi.js');
var generatedPath = midi.writeMml(cmrt_chord.resolve_chord_or_mml({source}).mml);
midi.addFile(generatedTrack.trackIndex, generatedPath);
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();";

/// Note-on pitches in the converter's order; MML meaning is not restated here.
fn converted_pitches(app: &mut Control, source: &str) -> Result<Vec<u8>, String> {
    let code = format!(
        "mmlabc_to_smf.pass3_events.ast_to_events(mmlabc_to_smf.pass2_ast.tokens_to_ast(mmlabc_to_smf.pass1_parser.parse_mml(cmrt_chord.resolve_chord_or_mml({source:?}).mml)), true).filter(e => e.type === 'note_on').map(e => e.note)"
    );
    let value: Value =
        serde_json::from_str(&app.eval(&code, "reference pitches")?).map_err(|e| e.to_string())?;
    value
        .as_array()
        .ok_or("Missing reference pitches")?
        .iter()
        .map(|p| {
            p.as_u64()
                .and_then(|p| u8::try_from(p).ok())
                .ok_or_else(|| "Bad reference pitch".to_owned())
        })
        .collect()
}

pub fn run(app: &mut Control, chord: bool) -> Result<(), String> {
    // Two chords of three notes, or three successive notes.
    let source = if chord { "F-G" } else { "cde" };
    let expected = converted_pitches(app, source)?;
    // Paths are unique even within one evaluation and do not create SMF files.
    if app.eval(
        "var pathA = getGeneratedMidiPath(); var pathB = getGeneratedMidiPath(); pathA !== pathB",
        "generated path uniqueness",
    )? != "true"
    {
        return Err("Generated paths collide".into());
    }
    for key in ["pathA", "pathB"] {
        let path: String =
            serde_json::from_str(&app.eval(key, "generated path")?).map_err(|e| e.to_string())?;
        let expected = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("repl-plugin-player/generated-midi");
        if !std::path::Path::new(&path).starts_with(expected)
            || std::path::Path::new(&path).exists()
        {
            return Err(format!(
                "Path helper created a file or returned wrong location: {path}"
            ));
        }
    }
    app.run(&ENTRY.replace("{source}", &format!("{source:?}")))?;
    let snapshot = app.read_timeline()?;
    let clips = snapshot["tracks"][0]["clips"]
        .as_array()
        .ok_or("Missing clips")?;
    if clips.len() != 1 {
        return Err(format!("Expected one musical clip: {snapshot}"));
    }
    let notes = crate::timeline::notes(&clips[0]["midi"]["events"]);
    let pitches: Vec<_> = notes.iter().map(|note| note.pitch).collect();
    if pitches != expected || notes.iter().any(|n| n.end.is_none()) {
        return Err(format!("Unexpected generated notes: {pitches:?}"));
    }
    if chord {
        if notes[..3].iter().any(|n| n.start != notes[0].start)
            || notes[3..].iter().any(|n| n.start != notes[3].start)
            || notes[3].start <= notes[0].start
        {
            return Err("Chord progression was not two successive chords".into());
        }
    } else if !(notes[0].start < notes[1].start && notes[1].start < notes[2].start) {
        return Err("MML notes were not successive".into());
    }
    let path: String = serde_json::from_str(&app.eval("generatedPath", "SMF retained")?)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if !bytes.starts_with(b"MThd") {
        return Err("Generated file is not SMF".into());
    }
    let mut peak = 0.0_f64;
    let tpqn = clips[0]["midi"]["tickResolution"]
        .as_f64()
        .ok_or("Missing TPQN")?;
    let last_end = notes
        .iter()
        .filter_map(|n| n.end)
        .max()
        .ok_or("Missing note ends")?;
    let mut delivered = std::collections::BTreeSet::new();
    let until = Instant::now() + Duration::from_secs_f64(last_end as f64 / tpqn * 0.5 + 0.3);
    while Instant::now() < until {
        app.poll()?;
        let signal = app.observe("generated MIDI playback")?;
        if signal["finite"] != true {
            return Err("Non-finite generated playback".into());
        }
        peak = peak.max(signal["peak"].as_f64().ok_or("Missing peak")?);
        delivered.extend(app.read_notes()?);
        std::thread::sleep(Duration::from_millis(25));
    }
    if peak <= 0.001 {
        return Err(format!("Generated clip did not sound: {peak}"));
    }
    if delivered != expected.iter().copied().collect() {
        return Err(format!(
            "Not all generated pitches reached the plugin: {delivered:?}"
        ));
    }
    // The native callback must surface a catchable JS error and stay usable.
    if app
        .run("mmlabc_to_smf.pass4_midi.save_midi_file([256], getGeneratedMidiPath())")
        .is_ok()
    {
        return Err("Invalid MIDI byte accepted".into());
    }
    if app.eval(
        "cmrt_chord.resolve_chord_or_mml('cde').from_chord",
        "callback recovery",
    )? != "false"
    {
        return Err("Callback failed to recover".into());
    }
    app.log.event("signal_check", json!({"source":source,"peak":peak,"retainedSmf":path,"pitches":pitches,"deviceOutputMeasured":false}))?;
    Ok(())
}
