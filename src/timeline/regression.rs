//! What the piano rolls show for tracks, clips and a loop; the cells are
//! `tests/e2e/timeline-view.js`.
use crate::{control::Control, e2e_script::Script};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Runs the mutation cell `title` and waits for its job.
fn mutate(app: &mut Control, script: &Script, title: &str) -> Result<(), String> {
    app.run(script.cell(title)?)?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        app.poll()?;
        let job: Value =
            serde_json::from_str(&app.eval(script.cell("job")?, "timeline view mutation")?)
                .map_err(|e| e.to_string())?;
        match job["state"].as_str() {
            Some("completed") => return Ok(()),
            Some("failed") => return Err(format!("Timeline view mutation failed: {job}")),
            _ if Instant::now() >= until => return Err("Timeline view mutation timed out".into()),
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    let initial = app.read_timeline()?;
    let view = super::projection::project(&initial);
    if view.tracks.len() != 1 || view.tracks[0].plugins.is_empty() {
        return Err("Unused bootstrap tracks leaked into the piano rolls".into());
    }
    mutate(app, script, "add a track")?;
    let added = app.read_timeline()?;
    let view = super::projection::project(&added);
    if view.tracks.len() != 2 {
        return Err("Explicit empty track was hidden".into());
    }
    let added_id = view.tracks[1].id.clone();
    mutate(app, script, "remove track 1")?;
    let moved = app.read_timeline()?;
    let view = super::projection::project(&moved);
    if view.tracks.len() != 2 || view.tracks[1].id != added_id || view.tracks[1].index != 2 {
        return Err("Explicit empty track lost its identity after reindexing".into());
    }
    app.run(script.cell("chord at beat one")?)?;
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
    app.run(script.cell("loop on track 0")?)?;
    let until = Instant::now() + Duration::from_millis(1100);
    let mut observed = 0;
    while Instant::now() < until {
        app.poll()?;
        let data = app.read_timeline()?;
        let view = super::projection::project(&data);
        if view.tracks[0].clips.len() != 2
            || view.tracks[0].clips[1].notes.len() != 4
            || view.loop_end.is_none_or(|end| (end - 4.0).abs() > 0.001)
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
    app.run(script.cell("save and reload loop")?)?;
    let restored = super::projection::project(&app.read_timeline()?);
    if restored.tracks[0].clips.len() != 2
        || restored.tracks[0].clips[1].notes.len() != 4
        || restored.loop_end.is_some()
    {
        return Err("Project restoration omitted the loop from the timeline".into());
    }
    app.run(script.cell("play restored loop")?)?;
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let data = app.read_timeline()?;
        let view = super::projection::project(&data);
        if view.playing
            && view.tracks[0].clips.len() == 2
            && view.tracks[0].clips[1].notes.len() == 4
        {
            break;
        }
        if Instant::now() >= until {
            return Err("Restored loop did not enter playback".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    app.run(script.cell("stop restored loop")?)?;
    app.run(script.cell("clear the loop")?)?;
    if super::projection::project(&app.read_timeline()?).tracks[0]
        .clips
        .len()
        != 2
    {
        return Err("Disabling transport loop removed ordinary clips".into());
    }
    app.run(script.cell("loop on track 1")?)?;
    if super::projection::project(&app.read_timeline()?)
        .tracks
        .len()
        != 3
    {
        return Err("Loop-only track was hidden".into());
    }
    app.run(script.cell("clear the loop")?)?;
    if super::projection::project(&app.read_timeline()?)
        .tracks
        .len()
        != 3
    {
        return Err("Previously used loop track vanished when its notes were cleared".into());
    }
    Ok(())
}
