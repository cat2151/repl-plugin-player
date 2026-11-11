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
    pub looping: bool,
    pub notes: Vec<Note>,
}

pub struct Track {
    pub id: String,
    pub index: usize,
    pub plugins: Vec<String>,
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

pub fn project(snapshot: &Value) -> View {
    let metadata = &snapshot["view"];
    let state = &snapshot["state"];
    let sr = number(&snapshot["sampleRate"], 48000.0).max(1.0);
    let loop_data = &metadata["loop"];
    let length = number(&loop_data["lengthSamples"], 0.0);
    let loop_bpm = number(&loop_data["bpm"], 120.0).max(1.0);
    let looping = length > 0.0;
    let clock = Clock::new(
        if looping {
            &[]
        } else {
            array(&metadata["tempos"])
        },
        if looping {
            loop_bpm
        } else {
            number(&state["tempo"], 120.0)
        },
    );
    let loop_end = looping.then(|| clock.beat(length / sr));
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
                looping: false,
                notes: project_notes(events, start, end, |sample| {
                    clock.beat((position + sample as f64 - offset) / sr)
                }),
            });
        }
        if let Some(part) = array(&loop_data["parts"])
            .iter()
            .find(|p| p["trackId"] == id)
        {
            let end = loop_end.unwrap_or(0.0);
            clips.push(Clip {
                label: "↻".into(),
                start: 0.0,
                end,
                muted: false,
                looping: true,
                // Note Offs at the loop end are not stored; playback flushes held notes at the wrap.
                notes: project_notes(&part["events"], 0.0, end, |sample| {
                    clock.beat(sample as f64 / sr)
                })
                .into_iter()
                .map(|note| Note {
                    open: false,
                    ..note
                })
                .collect(),
            });
        }
        let plugins: Vec<String> = array(&track["nodes"])
            .iter()
            .filter(|n| n["isPlugin"] == true)
            .filter_map(|n| n["displayName"].as_str().map(String::from))
            .collect();
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
    if !looping {
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
    }
    View {
        tracks,
        end: end.max(4.0),
        playing: state["isPlaying"] == true,
        loop_end,
        playhead: clock.beat(if looping {
            number(&loop_data["positionSamples"], 0.0) / sr
        } else {
            number(&state["playheadSamples"], 0.0) / sr
        }),
        signatures,
    }
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
