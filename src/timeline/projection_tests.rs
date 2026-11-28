use super::*;
use serde_json::json;

fn snapshot() -> Value {
    json!({"sampleRate":48000, "state":{"tempo":120, "isPlaying":true,"playheadSamples":24000},
        "tracks":[{"trackIndex":0,"nodes":[],"clips":[]},
            {"trackIndex":3,"nodes":[],"clips":[]}],
        "view":{"tracks":[{"trackId":"bootstrap","implicit":true,"clips":[]},
            {"trackId":"user-empty","implicit":false,"clips":[]}],
            "tempos":[],"signatures":[]}})
}

#[test]
fn recorded_audio_uses_clip_path_offset_and_tempo_changes() {
    let mut data = snapshot();
    data["tracks"][1]["clips"] = json!([{
        "clipType": "audio", "filepath": "X:/recordings/take.wav", "positionSamples": 24000,
        "durationSamples": 96000, "sourceOffsetSamples": 12000
    }]);
    data["view"]["tempos"] = json!([{"seconds": 0.0, "bpm": 120}, {"seconds": 1.0, "bpm": 60}]);
    let view = project(&data);
    let clip = &view.tracks[0].clips[0];
    let audio = clip.audio.as_ref().unwrap();
    assert_eq!(audio.path, std::path::Path::new("X:/recordings/take.wav"));
    assert_eq!((clip.start, clip.end), (1.0, 3.5));
    assert_eq!(audio.source_seconds(1.0), 0.25);
    assert_eq!(audio.source_seconds(2.0), 0.75);
    assert_eq!(audio.source_seconds(3.0), 1.75);
    assert_eq!(audio.source_seconds(3.5), 2.25);
}

#[test]
fn live_sequence_projects_one_pass_without_creating_document_clips() {
    let mut data = snapshot();
    data["sequence"] = json!({"track":0.0,"steps":[[48.0,52.0],[50,53]],"beats":4,"gate":1});
    let view = project(&data);
    let clip = &view.tracks[0].clips[0];
    assert_eq!((clip.start, clip.end), (0.0, 8.0));
    assert_eq!(
        clip.notes
            .iter()
            .map(|n| (n.pitch, n.start, n.end))
            .collect::<Vec<_>>(),
        [
            (48, 0.0, 4.0),
            (52, 0.0, 4.0),
            (50, 4.0, 8.0),
            (53, 4.0, 8.0)
        ]
    );
    assert!(data["tracks"][0]["clips"].as_array().unwrap().is_empty());
    data["sequence"] = Value::Null;
    assert_eq!(project(&data).tracks.len(), 1);
}

#[test]
fn paired_sequence_projects_on_each_selected_track() {
    let mut data = snapshot();
    data["sequence"] = json!({"tracks":[0.0,3.0],"steps":[[48,52],[50,53]],"beats":4,"gate":1});
    let view = project(&data);
    assert_eq!(view.tracks.len(), 2);
    for track in &view.tracks {
        assert_eq!(track.clips.len(), 1);
        assert_eq!(track.clips[0].notes.len(), 4);
        assert_eq!(track.clips[0].end, 8.0);
    }
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
fn transport_loop_does_not_change_clips_or_the_tempo_map() {
    let mut data = snapshot();
    data["tracks"][1]["clips"] = json!([{"clipId":8,"clipType":"midi",
        "positionSamples":24000,"durationSamples":144000}]);
    data["view"]["tracks"][1]["clips"] = json!([{"clipId":8,"events":[
        {"sample":0,"words":[0x20903c7f_u32]}, {"sample":120000,"words":[0x20803c00_u32]}
    ]}]);
    data["view"]["tempos"] = json!([{"seconds":0,"bpm":120},{"seconds":1,"bpm":60}]);
    data["view"]["signatures"] = json!([{"seconds":1,"numerator":3,"denominator":4}]);
    let before = project(&data);
    data["state"]["loopEnabled"] = json!(true);
    data["state"]["loopStartSamples"] = json!(24000);
    data["state"]["loopEndSamples"] = json!(144000);
    let view = project(&data);
    assert_eq!(view.tracks[0].clips.len(), before.tracks[0].clips.len());
    assert_eq!(
        view.tracks[0].clips[0].notes[0].end,
        before.tracks[0].clips[0].notes[0].end
    );
    assert_eq!((view.loop_start, view.loop_end), (Some(1.0), Some(4.0)));
    assert_eq!(view.playhead, before.playhead);
    assert_eq!(view.signatures[1].start, 2.0);
    assert_eq!(view.signatures[1].numerator, 3);
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
fn pitch_range_spans_every_note_across_tracks_and_clips() {
    let mut data = snapshot();
    assert_eq!(project(&data).pitch_range(), None);
    data["tracks"][1]["clips"] = json!([{"clipId":7,"clipType":"midi",
        "positionSamples":0,"durationSamples":48000}]);
    data["view"]["tracks"][1]["clips"] = json!([{"clipId":7,"events":[
        {"sample":0,"words":[0x2090487f_u32]}, {"sample":100,"words":[0x20804800_u32]}
    ]}]);
    data["tracks"][0]["clips"] = json!([{"clipId":8,"clipType":"midi",
        "positionSamples":0,"durationSamples":48000}]);
    data["view"]["tracks"][0]["clips"] = json!([{"clipId":8,"events":[
        {"sample":0,"words":[0x2090237f_u32]}
    ]}]);
    assert_eq!(project(&data).pitch_range(), Some((0x23, 0x48)));
}

#[test]
fn evolve_slot_labels_its_track_with_plugin_patch_stem_and_gain() {
    let mut data = snapshot();
    data["evolve"] = json!([
        {"track":0.0,"gain":0.0,"plugin":null,"patch":null},
        {"track":3.0,"gain":0.25,"state":4,"plugin":"Surge XT","patch":"Author/Pads/Paddy Power.fxp"}
    ]);
    let view = project(&data);
    let sounding = view.tracks[0].sounding.as_ref().unwrap();
    assert_eq!(view.tracks[0].index, 3);
    assert_eq!(sounding.label, "Surge XT · Paddy Power");
    assert_eq!(sounding.gain, 0.25);
    assert!(sounding.fading_out);
    data["evolve"] = json!([]);
    assert!(project(&data).tracks[0].sounding.is_none());
}
