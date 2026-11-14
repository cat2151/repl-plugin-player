//! Cutting c from a c-f-g clip and pasting it after g moves it in what the
//! piano roll shows; the cells are `tests/e2e/piano-roll-cut-paste.js`, which
//! also check the ticks and the plugin input.
use crate::{control::Control, e2e_script::Script};
use serde_json::json;

/// Pitches of track 0's notes in the piano roll, in start order.
fn shown(app: &Control) -> Result<Vec<u8>, String> {
    let view = crate::timeline::projection::project(&app.read_timeline()?);
    let track = view.tracks.first().ok_or("No track 0 in the piano rolls")?;
    let mut notes: Vec<_> = track
        .clips
        .iter()
        .flat_map(|clip| &clip.notes)
        .map(|note| (note.start, note.pitch))
        .collect();
    notes.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(notes.into_iter().map(|(_, pitch)| pitch).collect())
}

fn expect(app: &mut Control, step: &str, expected: &[u8]) -> Result<(), String> {
    let pitches = shown(app)?;
    app.log
        .event("piano_roll_view", json!({"step":step,"pitches":pitches}))?;
    if pitches != expected {
        return Err(format!(
            "After {step} the piano roll shows {pitches:?}, expected {expected:?}"
        ));
    }
    Ok(())
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    app.run(&script.lib)?;
    app.run(script.cell("a clip of c f g")?)?;
    expect(app, "creating the clip", &[60, 65, 67])?;
    app.run(script.cell("cut c")?)?;
    expect(app, "cutting c", &[65, 67])?;
    app.run(script.cell("paste c after g")?)?;
    expect(app, "pasting c", &[65, 67, 60])?;
    app.run(script.cell("plays f g c")?)?;
    app.stop("piano roll playback checked")?;
    crate::e2e::wait_stopped(app)?;
    app.run(script.cell("exact ticks at 97 BPM")?)?;
    for (title, error) in [
        ("paste into a missing clip", "No clip"),
        ("paste from an empty clipboard", "clipboard is empty"),
    ] {
        match app.run(script.cell(title)?) {
            Err(message) if message.contains(error) => app
                .log
                .event("expected_error", json!({"stage":title,"error":message}))?,
            result => return Err(format!("'{title}' did not fail with {error}: {result:?}")),
        }
    }
    app.run(script.cell("a cell after the errors")?)
}
