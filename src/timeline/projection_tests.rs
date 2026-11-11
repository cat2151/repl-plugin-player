use super::*;
use serde_json::json;

fn snapshot() -> Value {
    json!({"sampleRate":48000, "state":{"tempo":120, "isPlaying":true,"playheadSamples":24000},
        "tracks":[{"trackIndex":0,"nodes":[],"clips":[]},
            {"trackIndex":3,"nodes":[],"clips":[]}],
        "view":{"tracks":[{"trackId":"bootstrap","implicit":true,"clips":[]},
            {"trackId":"user-empty","implicit":false,"clips":[]}],
            "loop":{"ready":true,"lengthSamples":0},"tempos":[],"signatures":[]}})
}

#[test]
fn only_unused_bootstrap_tracks_are_hidden_and_identity_survives_reindexing() {
    let mut data = snapshot();
    let first = project(&data);
    assert_eq!(first.tracks.len(), 1);
    assert_eq!(first.tracks[0].id, "user-empty");
    data["tracks"][1]["trackIndex"] = json!(1);
    assert_eq!(project(&data).tracks[0].id, "user-empty");
    data["tracks"][0]["nodes"] = json!([{"isPlugin":true,"displayName":"Instrument"}]);
    assert_eq!(project(&data).tracks.len(), 2);
}

#[test]
fn clip_positions_durations_and_source_trims_share_the_same_beat_axis() {
    let mut data = snapshot();
    data["tracks"][1]["clips"] = json!([{"clipId":7,"clipType":"midi","name":"fixture",
        "positionSamples":48000,"sourceOffsetSamples":24000,"durationSamples":48000}]);
    data["view"]["tracks"][1]["clips"] = json!([{"clipId":7,"events":[
        {"sample":0,"words":[0x20903c7f_u32]}, {"sample":48000,"words":[0x20803c00_u32]},
        {"sample":60000,"words":[0x2090407f_u32]}, {"sample":120000,"words":[0x20804000_u32]}
    ]}]);
    let view = project(&data);
    let clip = &view.tracks[0].clips[0];
    assert_eq!((clip.start, clip.end), (2.0, 4.0));
    assert_eq!((clip.notes[0].start, clip.notes[0].end), (2.0, 3.0));
    assert_eq!((clip.notes[1].start, clip.notes[1].end), (3.5, 4.0));
    assert_eq!(view.playhead, 1.0);
}

#[test]
fn loop_parts_and_clips_coexist_and_unterminated_loop_notes_end_at_the_wrap() {
    let mut data = snapshot();
    data["view"]["loop"] = json!({"ready":true,"bpm":120,"positionSamples":12000,
    "lengthSamples":144000,"parts":[{"trackId":"bootstrap","events":[
        {"sample":0,"words":[0x20903c7f_u32]}, {"sample":24000,"words":[0x20803c00_u32]},
        {"sample":48000,"words":[0x2090407f_u32]}
    ]}]});
    data["tracks"][0]["clips"] =
        json!([{"clipId":8,"clipType":"audio","positionSamples":24000,"durationSamples":48000}]);
    let view = project(&data);
    assert_eq!(view.tracks.len(), 2);
    assert_eq!(view.tracks[0].clips.len(), 2);
    assert_eq!(view.loop_end, Some(6.0));
    assert_eq!(view.playhead, 0.5);
    let notes = &view.tracks[0].clips[1].notes;
    assert_eq!((notes[0].start, notes[0].end), (0.0, 1.0));
    assert_eq!(
        (notes[1].start, notes[1].end, notes[1].open),
        (2.0, 6.0, false)
    );
}

#[test]
fn project_tempo_map_and_meter_changes_are_used_instead_of_clip_tempo() {
    let mut data = snapshot();
    data["view"]["tempos"] = json!([{"seconds":0,"bpm":120},{"seconds":1,"bpm":60}]);
    data["view"]["signatures"] = json!([{"seconds":1,"numerator":3,"denominator":4}]);
    data["tracks"][1]["clips"] = json!([{"clipId":2,"tempo":300,"clipType":"midi",
        "positionSamples":24000,"durationSamples":72000}]);
    data["view"]["tracks"][1]["clips"] = json!([{"clipId":2,"events":[
        {"sample":0,"words":[0x20903c7f_u32]}, {"sample":72000,"words":[0x20803c00_u32]}
    ]}]);
    let view = project(&data);
    let clip = &view.tracks[0].clips[0];
    assert_eq!((clip.start, clip.end), (1.0, 3.0));
    assert_eq!((clip.notes[0].start, clip.notes[0].end), (1.0, 3.0));
    assert_eq!(view.signatures[1].start, 2.0);
    assert_eq!(view.signatures[1].numerator, 3);
}

#[test]
fn pitch_range_spans_every_note_across_tracks_clips_and_loop_parts() {
    let mut data = snapshot();
    assert_eq!(project(&data).pitch_range(), None);
    data["tracks"][1]["clips"] = json!([{"clipId":7,"clipType":"midi",
        "positionSamples":0,"durationSamples":48000}]);
    data["view"]["tracks"][1]["clips"] = json!([{"clipId":7,"events":[
        {"sample":0,"words":[0x2090487f_u32]}, {"sample":100,"words":[0x20804800_u32]}
    ]}]);
    data["view"]["loop"] = json!({"ready":true,"bpm":120,"lengthSamples":48000,
        "parts":[{"trackId":"bootstrap","events":[{"sample":0,"words":[0x2090237f_u32]}]}]});
    assert_eq!(project(&data).pitch_range(), Some((0x23, 0x48)));
}
