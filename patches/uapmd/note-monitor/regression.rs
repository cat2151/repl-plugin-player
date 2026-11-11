use super::{pump, wait_stopped};
use crate::control::Control;
use serde_json::json;
use std::time::{Duration, Instant};

fn wait_notes(app: &mut Control, expected: &[u8]) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        app.poll()?;
        let notes = match app.read_notes() {
            Ok(notes) => notes,
            Err(error) if error.contains("Note monitor busy") && Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(error) => return Err(error),
        };
        if notes == expected {
            app.log
                .event("note_monitor_check", json!({"notes":notes}))?;
            return Ok(());
        }
        if Instant::now() >= until {
            return Err(format!("Expected live notes {expected:?}, got {notes:?}"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn run(app: &mut Control) -> Result<(), String> {
    wait_notes(app, &[])?;
    // Queuing a script command while the engine is off must not light a key.
    app.run("uapmd.sequencer.sendNoteOn(surgeId, 60)")?;
    wait_notes(app, &[])?;
    app.run("uapmd.audio.setEngineEnabled(true)")?;
    wait_notes(app, &[60])?;
    app.run("uapmd.sequencer.sendNoteOn(surgeId, 64); uapmd.sequencer.sendNoteOn(surgeId, 72)")?;
    wait_notes(app, &[60, 64, 72])?;
    app.run("uapmd.sequencer.sendNoteOff(surgeId, 60)")?;
    wait_notes(app, &[64, 72])?;
    if app
        .run("uapmd.sequencer.sendNoteOn(surgeId, 61); throw new Error('after note')")
        .is_ok()
    {
        return Err("Expected script exception".into());
    }
    wait_notes(app, &[61, 64, 72])?;
    app.stop("E2E Stop held chord")?;
    wait_stopped(app)?;
    wait_notes(app, &[])?;
    app.run("uapmd.audio.setEngineEnabled(true); uapmd.sequencer.sendNoteOn(surgeId, 60)")?;
    wait_notes(app, &[60])?;
    app.run("uapmd.sequencer.sendNoteOff(surgeId, 60)")?;
    wait_notes(app, &[])?;
    // Check scheduled delivery of a timeline clip rather than model prediction.
    app.run(super::TIMELINE)?;
    wait_notes(app, &[60])?;
    wait_notes(app, &[])?;
    app.stop("E2E complete")?;
    wait_stopped(app)?;
    // Same pitch on different channels, MIDI 1/2, velocity-zero and channel CCs.
    app.run(
        r#"
        uapmd.timeline.removeClip(0, uapmd.timeline.getClips(0)[0].clipId);
        uapmd.sequencer.jumpPlayback(0);
        var monitorClip = uapmd.timeline.createEmptyMidiClip(0, 0, 480, 120);
        var monitorTpqn = uapmd.timeline.getClipUmpEvents(0, monitorClip).tickResolution;
        [[1,[0x20903c7f]], [1,[0x20913c7f]], [1,[0x20903d7f]],
         [1,[0x40904800,0x7fff0000]], [2,[0x20803c00]],
         [3,[0x20b17b00]], [4,[0x20903d00]], [4,[0x40b07800,0]]]
          .forEach(e => uapmd.timeline.addUmpEvent(0, monitorClip, e[0]*monitorTpqn, e[1]));
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.startPlayback();
    "#,
    )?;
    wait_notes(app, &[60, 61, 72])?;
    pump(
        app,
        Duration::from_millis(650),
        "same pitch on second channel",
    )?;
    if app.read_notes()? != [60, 61, 72] {
        return Err("Note Off on one channel cleared another channel's note".into());
    }
    wait_notes(app, &[61, 72])?;
    wait_notes(app, &[])?;
    app.stop("E2E complete")?;
    wait_stopped(app)?;
    Ok(())
}
