use super::notes::notes_at;
use serde_json::Value;

pub struct Note {
    pub pitch: u8,
    pub group: u8,
    pub channel: u8,
    pub start: f64,
    pub end: f64,
    pub open: bool,
}

pub struct Clip {
    pub label: String,
    pub start: f64,
    pub end: f64,
    pub muted: bool,
    pub notes: Vec<Note>,
}

/// The sound an evolve slot holds on this track; `gain` is linear, 0 when silent.
pub struct Sounding {
    pub label: String,
    pub gain: f32,
    /// evolve's RETIRING slot state: the gain only falls.
    pub fading_out: bool,
}

pub struct Track {
    pub id: String,
    pub index: usize,
    pub plugins: Vec<String>,
    pub plugins_serial: bool,
    pub sounding: Option<Sounding>,
    pub clips: Vec<Clip>,
    pub low: u8,
    pub high: u8,
}

pub struct Signature {
    pub start: f64,
    pub numerator: u32,
    pub denominator: u32,
}

pub struct View {
    pub tracks: Vec<Track>,
    pub end: f64,
    pub playhead: f64,
    pub playing: bool,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
    pub signatures: Vec<Signature>,
}

impl View {
    /// Lowest and highest pitch over every note in every clip.
    pub fn pitch_range(&self) -> Option<(u8, u8)> {
        let mut pitches = self
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .flat_map(|c| &c.notes)
            .map(|n| n.pitch);
        let first = pitches.next()?;
        Some(pitches.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }
}

pub fn number(value: &Value, default: f64) -> f64 {
    value.as_f64().filter(|n| n.is_finite()).unwrap_or(default)
}

pub fn array(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

// UAPMD master tempo points are seconds and BPM, not independent clip tempos.
struct Clock {
    segments: Vec<(f64, f64, f64)>, // seconds, accumulated quarter notes, BPM
}

impl Clock {
    fn new(points: &[Value], default: f64) -> Self {
        let mut bpm = points
            .first()
            .map_or(default, |p| number(&p["bpm"], default))
            .max(1.0);
        let mut segments = vec![(0.0, 0.0, bpm)];
        for point in points {
            let seconds = number(&point["seconds"], 0.0).max(0.0);
            let beat = Self::convert(&segments, seconds);
            bpm = number(&point["bpm"], bpm).max(1.0);
            segments.push((seconds, beat, bpm));
        }
        Self { segments }
    }

    fn convert(segments: &[(f64, f64, f64)], seconds: f64) -> f64 {
        let &(start, beat, bpm) = segments
            .iter()
            .rev()
            .find(|s| s.0 <= seconds)
            .unwrap_or(&segments[0]);
        beat + (seconds - start) * bpm / 60.0
    }

    fn beat(&self, seconds: f64) -> f64 {
        Self::convert(&self.segments, seconds)
    }
}

fn project_notes(
    events: &Value,
    start: f64,
    end: f64,
    sample_to_beat: impl Fn(u64) -> f64,
) -> Vec<Note> {
    notes_at(events, "sample")
        .into_iter()
        .filter_map(|note| {
            let from = sample_to_beat(note.start).max(start);
            let to = note.end.map(&sample_to_beat).unwrap_or(end).min(end);
            (from < end && to > start && to > from).then_some(Note {
                pitch: note.pitch,
                group: note.group,
                channel: note.channel,
                start: from,
                end: to,
                open: note.end.is_none(),
            })
        })
        .collect()
}

/// "plugin · patch file stem" of the evolve slot on `track`.
fn sounding(slots: &Value, track: &Value) -> Option<Sounding> {
    let slot = array(slots).iter().find(|slot| {
        slot["track"]
            .as_f64()
            .is_some_and(|t| Some(t) == track.as_f64())
    })?;
    let plugin = slot["plugin"].as_str()?;
    let label = match slot["patch"].as_str().map(std::path::Path::new) {
        Some(patch) => format!(
            "{plugin} · {}",
            patch
                .file_stem()
                .unwrap_or(patch.as_os_str())
                .to_string_lossy()
        ),
        None => plugin.to_owned(),
    };
    Some(Sounding {
        label,
        gain: number(&slot["gain"], 0.0) as f32,
        // `repl_extension_api::evolve::slot_state::RETIRING`.
        fading_out: slot["state"].as_f64() == Some(4.0),
    })
}

pub fn project(snapshot: &Value) -> View {
    let metadata = &snapshot["view"];
    let state = &snapshot["state"];
    let sr = number(&snapshot["sampleRate"], 48000.0).max(1.0);
    let clock = Clock::new(array(&metadata["tempos"]), number(&state["tempo"], 120.0));
    let looping = state["loopEnabled"] == true
        && number(&state["loopEndSamples"], 0.0) > number(&state["loopStartSamples"], 0.0);
    let loop_start = looping.then(|| clock.beat(number(&state["loopStartSamples"], 0.0) / sr));
    let loop_end = looping.then(|| clock.beat(number(&state["loopEndSamples"], 0.0) / sr));
    let mut tracks = Vec::new();
    let mut end: f64 = loop_end.unwrap_or(4.0);
    for (index, track) in array(&snapshot["tracks"]).iter().enumerate() {
        let info = &metadata["tracks"][index];
        let id = info["trackId"].as_str().unwrap_or("");
        let mut clips = Vec::new();
        for clip in array(&track["clips"]) {
            let position = number(&clip["positionSamples"], 0.0);
            let offset = number(&clip["sourceOffsetSamples"], 0.0);
            let start = clock.beat(position / sr);
            let end = clock.beat((position + number(&clip["durationSamples"], 0.0)) / sr);
            let events = array(&info["clips"])
                .iter()
                .find(|c| c["clipId"] == clip["clipId"])
                .map_or(&Value::Null, |c| &c["events"]);
            clips.push(Clip {
                label: clip["name"]
                    .as_str()
                    .filter(|n| !n.is_empty())
                    .map(String::from)
                    .unwrap_or_else(|| {
                        format!(
                            "{} {}",
                            clip["clipType"].as_str().unwrap_or(""),
                            clip["clipId"]
                        )
                    }),
                start,
                end,
                muted: clip["muted"] == true,
                notes: project_notes(events, start, end, |sample| {
                    clock.beat((position + sample as f64 - offset) / sr)
                }),
            });
        }
        let sequence = &snapshot["sequence"];
        // JavaScript numbers arrive as JSON floats, including integral indices.
        if sequence["track"]
            .as_f64()
            .is_some_and(|target| Some(target) == track["trackIndex"].as_f64())
            || array(&sequence["tracks"]).iter().any(|target| {
                target
                    .as_f64()
                    .is_some_and(|target| Some(target) == track["trackIndex"].as_f64())
            })
        {
            let beats = number(&sequence["beats"], 1.0);
            let steps = array(&sequence["steps"]);
            clips.push(Clip {
                label: String::new(),
                start: 0.0,
                end: beats * steps.len() as f64,
                muted: false,
                notes: steps
                    .iter()
                    .enumerate()
                    .flat_map(|(i, step)| {
                        array(step).iter().filter_map(move |pitch| {
                            let pitch = pitch
                                .as_f64()
                                .filter(|p| (0.0..128.0).contains(p) && p.fract() == 0.0)?
                                as u8;
                            Some(Note {
                                pitch,
                                group: 0,
                                channel: 0,
                                start: i as f64 * beats,
                                end: (i as f64 + number(&sequence["gate"], 1.0)) * beats,
                                open: false,
                            })
                        })
                    })
                    .collect(),
            });
        }
        let (plugins, plugins_serial) = super::plugin_names::project(track);
        if info["implicit"] == true && clips.is_empty() && plugins.is_empty() {
            continue;
        }
        end = clips.iter().map(|c| c.end).fold(end, f64::max);
        let low = clips
            .iter()
            .flat_map(|c| &c.notes)
            .map(|n| n.pitch)
            .min()
            .unwrap_or(60)
            .saturating_sub(2);
        let high = clips
            .iter()
            .flat_map(|c| &c.notes)
            .map(|n| n.pitch)
            .max()
            .unwrap_or(71)
            .saturating_add(2)
            .min(127);
        // At least an octave, with the same physical semitone height in every track.
        let high = high.max(low.saturating_add(11)).min(127);
        let low = low.min(high.saturating_sub(11));
        tracks.push(Track {
            id: id.into(),
            index: number(&track["trackIndex"], index as f64) as usize,
            plugins,
            plugins_serial,
            sounding: sounding(&snapshot["evolve"], &track["trackIndex"]),
            clips,
            low,
            high,
        });
    }
    let mut signatures = vec![Signature {
        start: 0.0,
        numerator: 4,
        denominator: 4,
    }];
    for point in array(&metadata["signatures"]) {
        let numerator = number(&point["numerator"], 4.0).max(1.0) as u32;
        let denominator = number(&point["denominator"], 4.0).max(1.0) as u32;
        if signatures
            .last()
            .is_some_and(|s| s.numerator == numerator && s.denominator == denominator)
        {
            continue;
        }
        signatures.push(Signature {
            start: clock.beat(number(&point["seconds"], 0.0)),
            numerator,
            denominator,
        });
    }
    View {
        tracks,
        end: end.max(4.0),
        playing: state["isPlaying"] == true,
        loop_end,
        loop_start,
        playhead: clock.beat(number(&state["playheadSamples"], 0.0) / sr),
        signatures,
    }
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
