use crate::control::Control;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn mutate(app: &mut Control, expression: &str) -> Result<(), String> {
    app.run(&format!("globalThis.timelineViewJob = {expression};"))?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        app.poll()?;
        let job: Value = serde_json::from_str(&app.eval(
            "uapmd.mutations.getJob(timelineViewJob.jobId)",
            "timeline view mutation",
        )?)
        .map_err(|e| e.to_string())?;
        match job["state"].as_str() {
            Some("completed") => return Ok(()),
            Some("failed") => return Err(format!("Timeline view mutation failed: {job}")),
            _ if Instant::now() >= until => return Err("Timeline view mutation timed out".into()),
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

pub fn run(app: &mut Control) -> Result<(), String> {
    app.loop_checks()?;
    let initial = app.read_timeline()?;
    let view = super::projection::project(&initial);
    if view.tracks.len() != 1 || view.tracks[0].plugins.is_empty() {
        return Err("Unused bootstrap tracks leaked into the piano rolls".into());
    }
    mutate(app, "uapmd.sequencer.addTrack()")?;
    let added = app.read_timeline()?;
    let view = super::projection::project(&added);
    if view.tracks.len() != 2 {
        return Err("Explicit empty track was hidden".into());
    }
    let added_id = view.tracks[1].id.clone();
    mutate(app, "uapmd.sequencer.removeTrack(1)")?;
    let moved = app.read_timeline()?;
    let view = super::projection::project(&moved);
    if view.tracks.len() != 2 || view.tracks[1].id != added_id || view.tracks[1].index != 2 {
        return Err("Explicit empty track lost its identity after reindexing".into());
    }
    app.run("var tvClip = uapmd.timeline.createEmptyMidiClip(0, uapmd.sequencer.getSampleRate()/2, 480, 120);
        uapmd.timeline.addUmpEvent(0,tvClip,0,[0x20903c64]);
        uapmd.timeline.addUmpEvent(0,tvClip,0,[0x20904064]);
        uapmd.timeline.addUmpEvent(0,tvClip,960,[0x20803c00]);
        uapmd.timeline.addUmpEvent(0,tvClip,960,[0x20804000]);")?;
    let clips = app.read_timeline()?;
    let view = super::projection::project(&clips);
    let clip = &view.tracks[0].clips[0];
    if (clip.start - 1.0).abs() > 0.001
        || clip.notes.len() != 2
        || clip
            .notes
            .iter()
            .any(|n| (n.start - 1.0).abs() > 0.001 || (n.end - 3.0).abs() > 0.001)
    {
        return Err("Native clip timing did not produce an aligned chord at beat one".into());
    }
    // A test-owned phrase, independent of the user's editable built-in snippets.
    app.run(
        "var tvPath = getGeneratedMidiPath();
        mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('l4 >cg<cg'),tvPath);
        repl.timeline.publishLoopClips([{trackIndex:0,filepath:tvPath}],4,120);
        uapmd.audio.setEngineEnabled(true); uapmd.sequencer.startPlayback();",
    )?;
    let until = Instant::now() + Duration::from_millis(1100);
    let mut observed = 0;
    while Instant::now() < until {
        app.poll()?;
        let data = app.read_timeline()?;
        if data["view"]["loop"]["ready"] == false {
            continue;
        }
        let view = super::projection::project(&data);
        if view.tracks[0].clips.len() != 2
            || view.tracks[0].clips[1].notes.len() != 4
            || view.loop_end.is_none_or(|end| (end - 4.0).abs() > 0.001)
            || data["tracks"] != clips["tracks"]
        {
            return Err(
                "Playing loop visualization omitted notes or changed ordinary clips".into(),
            );
        }
        observed += 1;
        app.log.event("timeline_view", json!({"snapshot":data}))?;
        std::thread::sleep(Duration::from_millis(25));
    }
    if observed < 10 {
        return Err("Loop display could not acquire a stable playback snapshot".into());
    }
    app.stop("timeline view regression complete")?;
    let until = Instant::now() + Duration::from_secs(10);
    while app.stopping {
        app.poll()?;
        if Instant::now() >= until {
            return Err("Timeline view Stop timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let stopped_data = app.read_timeline()?;
    app.log
        .event("timeline_view_stopped", json!({"snapshot":stopped_data}))?;
    let stopped = super::projection::project(&stopped_data);
    if stopped.tracks[0].clips.len() != 2 || stopped.playing {
        return Err("Stopped loop disappeared from the display".into());
    }
    app.run("repl.timeline.clearLoop()")?;
    if super::projection::project(&app.read_timeline()?).tracks[0]
        .clips
        .len()
        != 1
    {
        return Err("Cleared loop remained visible".into());
    }
    // Using an initially empty track for a loop also makes it a user-used track.
    app.run("repl.timeline.publishLoopClips([{trackIndex:1,filepath:tvPath}],4,120)")?;
    if super::projection::project(&app.read_timeline()?)
        .tracks
        .len()
        != 3
    {
        return Err("Loop-only track was hidden".into());
    }
    app.run("repl.timeline.clearLoop()")?;
    if super::projection::project(&app.read_timeline()?)
        .tracks
        .len()
        != 3
    {
        return Err("Previously used loop track vanished when its notes were cleared".into());
    }
    Ok(())
}
