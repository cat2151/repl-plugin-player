use crate::{control::Control, scope_trigger};
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
            return Err(format!("No scope waveform for note {note}"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn run(app: &mut Control) -> Result<(), String> {
    app.run("uapmd.audio.setEngineEnabled(true); uapmd.sequencer.sendNoteOn(surgeId, 60);")?;
    wait_note(app, 60)?;
    app.run("uapmd.sequencer.sendNoteOff(surgeId, 60); uapmd.sequencer.sendNoteOn(surgeId, 72);")?;
    wait_note(app, 72)?;
    app.stop("scope restart check")?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.stopping {
        app.poll()?;
        if Instant::now() >= deadline {
            return Err("Scope stop timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    app.run("uapmd.audio.setEngineEnabled(true); uapmd.sequencer.sendNoteOn(surgeId, 67);")?;
    wait_note(app, 67)
}
