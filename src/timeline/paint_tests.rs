use super::*;
use crate::timeline::projection::{self, Note};

fn fixture() -> View {
    View {
        tracks: vec![Track {
            id: "fixture".into(),
            index: 0,
            plugins: vec!["Instrument".into()],
            plugins_serial: true,
            sounding: None,
            low: 58,
            high: 73,
            clips: vec![Clip {
                label: "Chord".into(),
                start: 1.0,
                end: 4.0,
                muted: false,
                audio: None,
                notes: [60, 64, 67]
                    .into_iter()
                    .map(|pitch| Note {
                        pitch,
                        group: 0,
                        channel: 0,
                        start: 1.0,
                        end: 3.0,
                        open: false,
                    })
                    .collect(),
            }],
        }],
        end: 8.0,
        playhead: 2.0,
        playing: true,
        recording: false,
        latest_recording: None,
        loop_start: None,
        loop_end: None,
        signatures: vec![projection::Signature {
            start: 0.0,
            numerator: 4,
            denominator: 4,
        }],
    }
}

fn rectangles(shape: &egui::Shape, fill: Color32, output: &mut Vec<Rect>) {
    match shape {
        egui::Shape::Rect(rect) if rect.fill == fill => output.push(rect.rect),
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| rectangles(s, fill, output)),
        _ => {}
    }
}

#[test]
fn finished_recording_reveals_its_audio_lane_below_the_visible_tracks() {
    let ctx = egui::Context::default();
    let mut view = fixture();
    for index in 1..4 {
        let mut track = fixture().tracks.pop().unwrap();
        track.index = index;
        track.id = index.to_string();
        view.tracks.push(track);
    }
    let track = view.tracks.last_mut().unwrap();
    track.clips[0] = audio_clip(std::path::Path::new(""));
    let recording = (track.id.clone(), "take.wav".into());
    let mut visible = false;
    for frame in 0..5 {
        if frame == 2 {
            view.latest_recording = Some(recording.clone());
        }
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 240.0),
            )),
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| show(ui, &view));
        });
        visible = output.shapes.iter().any(|clipped| {
            let mut clips = Vec::new();
            rectangles(&clipped.shape, AUDIO_COLOR.gamma_multiply(0.25), &mut clips);
            clips
                .iter()
                .any(|clip| clipped.clip_rect.contains(clip.center()))
        });
        output.drop_without_applying_deltas();
        if frame < 2 {
            assert!(!visible);
        }
    }
    assert!(
        visible,
        "the audio clip was still outside the visible timeline"
    );
}

fn audio_clip(path: &std::path::Path) -> Clip {
    let data = serde_json::json!({
        "sampleRate": 48000,
        "tracks": [{
            "clips": [{
                "clipType": "audio",
                "filepath": path,
                "positionSamples": 24000,
                "durationSamples": 72000
            }]
        }]
    });
    projection::project(&data).tracks.remove(0).clips.remove(0)
}

fn render_timeline(ctx: &egui::Context, view: &View, zoom: f32) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 300.0),
        )),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let id = ui.make_persistent_id("timeline_zoom");
            ui.data_mut(|data| data.insert_temp(id, zoom));
            show(ui, view);
        });
    })
}

#[test]
fn timeline_draws_wav_peaks_and_adds_columns_when_zoomed() {
    let path = std::env::temp_dir().join(format!("repl-waveform-{}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for (frames, amplitude) in [(24_000, 0.0_f32), (24_000, 0.01), (24_000, 0.05)] {
        for frame in 0..frames {
            let sample = if frame % 2 == 0 {
                amplitude
            } else {
                -amplitude
            };
            writer.write_sample(sample).unwrap();
        }
    }
    writer.finalize().unwrap();
    let mut view = fixture();
    view.tracks[0].clips[0] = audio_clip(&path);
    let ctx = egui::Context::default();
    let frame = |zoom| {
        let output = render_timeline(&ctx, &view, zoom);
        let columns = output.shapes.iter().find_map(|clipped| {
            let egui::Shape::Mesh(mesh) = &clipped.shape else {
                return None;
            };
            mesh.vertices
                .first()
                .filter(|vertex| vertex.color == AUDIO_COLOR)?;
            Some(
                mesh.vertices
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|rect| rect[2].pos.y - rect[0].pos.y)
                    .collect::<Vec<_>>(),
            )
        });
        output.drop_without_applying_deltas();
        columns
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let mut overview = None;
    while overview.is_none() && std::time::Instant::now() < deadline {
        overview = frame(80.0_f32);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let zoomed = frame(160.0_f32);
    std::fs::remove_file(&path).unwrap();
    let overview = overview.expect("the worker should decode and display the WAV");
    let zoomed = zoomed.expect("the same waveform should be available after zooming");
    assert!(zoomed.len() >= overview.len() * 2 - 1);
    for columns in [overview, zoomed] {
        let quarter = columns.len() / 4;
        assert!(columns[quarter] <= 1.1);
        assert!(columns[2 * quarter] > 4.0);
        assert!(columns[3 * quarter] > 2.0 * columns[2 * quarter]);
    }
}

#[test]
fn timeline_overlays_wav_pitch_at_the_trimmed_clip_tempo() {
    let path = std::env::temp_dir().join(format!("repl-pitch-{}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for (frames, hz) in [
        (9_600, 0.0),
        (19_200, 440.0),
        (19_200, 0.0),
        (19_200, 660.0),
    ] {
        for frame in 0..frames {
            let phase = std::f64::consts::TAU * hz * f64::from(frame) / 48_000.0;
            writer.write_sample((phase.sin() * 8000.0) as i16).unwrap();
            writer.write_sample(0_i16).unwrap();
        }
    }
    writer.finalize().unwrap();
    let view = projection::project(&serde_json::json!({
        "sampleRate": 48000,
        "view": {"tempos": [{"seconds": 0, "bpm": 120}, {"seconds": 1, "bpm": 60}]},
        "tracks": [{"clips": [{"clipType": "audio", "filepath": path,
            "positionSamples": 24000, "durationSamples": 57600, "sourceOffsetSamples": 9600}]}]
    }));
    let ctx = egui::Context::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let overlay = loop {
        let output = render_timeline(&ctx, &view, 160.0);
        let mut clip_x = None;
        let mut lines = Vec::new();
        for clipped in &output.shapes {
            match &clipped.shape {
                egui::Shape::Mesh(mesh) => {
                    if let Some(vertex) = mesh.vertices.first().filter(|v| v.color == AUDIO_COLOR) {
                        clip_x = Some(vertex.pos.x);
                    }
                }
                egui::Shape::Path(path)
                    if path.stroke.color
                        == egui::epaint::ColorMode::Solid(Color32::from_rgb(255, 218, 122)) =>
                {
                    lines.push(path.points.clone());
                }
                _ => {}
            }
        }
        output.drop_without_applying_deltas();
        if !lines.is_empty() || std::time::Instant::now() >= deadline {
            break (clip_x, lines);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    std::fs::remove_file(&path).unwrap();
    let (clip_x, lines) = overlay;
    let clip_x = clip_x.expect("WAV peaks should be visible");
    let first = clip_x + 0.4 * 160.0; // Source 0.4 s, timeline 0.7 s at 120 BPM.
    let gap = clip_x + 1.1 * 160.0; // Source 0.8 s, timeline 1.1 s at 60 BPM.
    let second = clip_x + 1.5 * 160.0; // Source 1.2 s, timeline 1.5 s at 60 BPM.
    let y_at = |x: f32| {
        lines
            .iter()
            .flatten()
            .find(|point| (point.x - x).abs() < 1.0)
            .map(|point| point.y)
    };
    assert!(lines
        .iter()
        .all(|line| !(line[0].x < gap && line.last().unwrap().x > gap)));
    assert!(y_at(gap).is_none());
    let low = y_at(first).expect("440 Hz section should have a pitch line");
    let high = y_at(second).expect("660 Hz section should have a pitch line");
    assert!(low - high > 5.0);
}

#[test]
fn header_uses_arrows_only_for_serial_connections() {
    fn texts(shape: &egui::Shape, output: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => output.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, output)),
            _ => {}
        }
    }
    for (serial, expected) in [
        (true, "Track 0 · Instrument → Effect"),
        (false, "Track 0 · Instrument, Effect"),
    ] {
        let ctx = egui::Context::default();
        let mut view = fixture();
        view.tracks[0].plugins = vec!["Instrument".into(), "Effect".into()];
        view.tracks[0].plugins_serial = serial;
        let mut labels = Vec::new();
        for _ in 0..2 {
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| show(ui, &view));
            });
            labels.clear();
            for shape in &output.shapes {
                texts(&shape.shape, &mut labels);
            }
            output.drop_without_applying_deltas();
        }
        assert!(labels.iter().any(|label| label == expected), "{labels:?}");
    }
}

#[test]
fn fading_out_label_darkens_early_and_fading_in_label_brightens_early() {
    let (background, text) = (Color32::from_gray(10), Color32::from_gray(220));
    let half = (10 + 220) / 2;
    for fading_out in [true, false] {
        assert_eq!(label_color(background, text, 0.0, fading_out), background);
        assert_eq!(label_color(background, text, 1.0, fading_out), text);
        let level = |gain| label_color(background, text, gain, fading_out).r();
        assert!(level(0.01) < level(0.1) && level(0.1) < level(0.5) && level(0.5) < level(1.0));
        // Halfway through a fade the outgoing label is dim and the incoming one bright.
        assert_eq!(level(0.5) < half, fading_out);
    }
}

#[test]
fn silent_evolve_label_is_drawn_in_the_header_background() {
    fn texts(shape: &egui::Shape, output: &mut Vec<(String, Color32)>) {
        match shape {
            egui::Shape::Text(text) => output.push((
                text.galley.text().to_owned(),
                text.galley.job.sections[0].format.color,
            )),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, output)),
            _ => {}
        }
    }
    let ctx = egui::Context::default();
    let mut view = fixture();
    view.tracks[0].sounding = Some(projection::Sounding {
        label: "Synth · Pad".into(),
        gain: 0.0,
        fading_out: false,
    });
    let mut labels = Vec::new();
    for _ in 0..2 {
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| show(ui, &view));
        });
        labels.clear();
        for shape in &output.shapes {
            texts(&shape.shape, &mut labels);
        }
        output.drop_without_applying_deltas();
    }
    let background = ctx.global_style().visuals.extreme_bg_color;
    let text = ctx.global_style().visuals.text_color();
    assert!(labels.contains(&("Track 0".into(), text)), "{labels:?}");
    assert!(
        labels.contains(&(" · Synth · Pad".into(), background)),
        "{labels:?}"
    );
}

#[test]
fn chord_notes_use_pitch_rows_and_one_time_axis_at_different_widths() {
    for width in [260.0, 560.0, 1000.0] {
        let ctx = egui::Context::default();
        let view = fixture();
        let mut bars = Vec::new();
        for _ in 0..2 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 500.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| show(ui, &view));
                },
            );
            bars.clear();
            for shape in &output.shapes {
                rectangles(&shape.shape, color(&view.tracks[0].clips[0]), &mut bars);
            }
            output.drop_without_applying_deltas();
        }
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].x_range(), bars[1].x_range());
        assert_eq!(bars[1].x_range(), bars[2].x_range());
        assert!((bars[0].top() - bars[1].top() - 4.0 * SEMITONE).abs() < 0.01);
        assert!((bars[1].top() - bars[2].top() - 3.0 * SEMITONE).abs() < 0.01);
        assert!(bars.iter().all(|r| r.is_finite() && r.width() > 2.0));
    }
}

#[path = "paint_live_tests.rs"]
mod live;
