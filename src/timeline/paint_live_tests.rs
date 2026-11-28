use super::*;

#[test]
fn growing_recording_draws_waveform_and_pitch_before_finalizing_its_wav() {
    let path = std::env::temp_dir().join(format!("repl-live-waveform-{}.wav", std::process::id()));
    let _cleanup = RemoveTestWav(&path);
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    let push = |writer: &mut hound::WavWriter<_>, count| {
        for frame in 0..count {
            let phase = std::f32::consts::TAU * 440.0 * frame as f32 / 48_000.0;
            writer.write_sample(0.1 * phase.sin()).unwrap();
        }
        writer.flush().unwrap();
    };
    let ctx = egui::Context::default();
    let assert_preview = |view: &View, color, expected_width: f32| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        loop {
            let output = render_timeline(&ctx, view, 160.0);
            let mut width = 0.0_f32;
            let mut pitch = false;
            for clipped in &output.shapes {
                match &clipped.shape {
                    egui::Shape::Mesh(mesh)
                        if mesh.vertices.first().is_some_and(|v| v.color == color) =>
                    {
                        width = width.max(mesh.calc_bounds().width());
                    }
                    egui::Shape::Path(path)
                        if path.stroke.color
                            == egui::epaint::ColorMode::Solid(Color32::from_rgb(255, 218, 122)) =>
                    {
                        pitch = true;
                    }
                    _ => {}
                }
            }
            output.drop_without_applying_deltas();
            if width >= expected_width && pitch {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "waveform width {width}, pitch shown {pitch}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let mut data = serde_json::json!({
        "pendingRecording": {"startSeconds": 0.5},
        "recordingProgress": {"filepath": path, "frames": 0, "sampleRate": 48_000, "running": true}
    });
    for (more, width) in [(9_600, 63.0), (4_920, 96.0)] {
        push(&mut writer, more);
        data["recordingProgress"]["frames"] = writer.duration().into();
        let view = projection::project(&data);
        assert!(view.recording);
        let clip = &view.tracks[0].clips[0];
        assert_eq!(clip.start, 1.0);
        assert_preview(&view, RECORDING_COLOR, width);
    }
    // The finalized clip can have additional audio that was never projected as live.
    push(&mut writer, 3_480);
    writer.finalize().unwrap();
    data["pendingRecording"] = serde_json::Value::Null;
    data["tracks"] = serde_json::json!([{"clips": [{
        "clipType": "audio", "filepath": path, "positionSamples": 24_000, "durationSamples": 18_000
    }]}]);
    let view = projection::project(&data);
    assert!(!view.recording);
    assert_eq!(view.tracks.len(), 1);
    assert_preview(&view, AUDIO_COLOR, 119.0);
}

struct RemoveTestWav<'a>(&'a std::path::Path);

impl Drop for RemoveTestWav<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
    }
}
