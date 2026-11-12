use serde_json::Value;
use std::collections::{HashMap, VecDeque};

// A disposable projection of a snapshot, never an editable playback model.
#[derive(Debug, PartialEq)]
pub struct Note {
    pub pitch: u8,
    pub group: u8,
    pub channel: u8,
    pub start: u64,
    pub end: Option<u64>,
}

// The JS runtime serializes some integral values as JSON floats (notably 0.0).
fn integer(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && *n < u64::MAX as f64 && n.fract() == 0.0)
            .map(|n| n as u64)
    })
}

pub(super) fn notes_at(events: &Value, time_key: &str) -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    let mut held: HashMap<(u8, u8, u8), VecDeque<usize>> = HashMap::new();
    for event in events.as_array().into_iter().flatten() {
        let Some(word) = integer(&event["words"][0]) else {
            continue;
        };
        let Some(tick) = integer(&event[time_key]) else {
            continue;
        };
        let kind = (word >> 28) & 15;
        if kind != 2 && kind != 4 {
            continue;
        }
        let status = (word >> 20) & 15;
        if status != 8 && status != 9 {
            continue;
        }
        let pitch = ((word >> 8) & 127) as u8;
        let group = ((word >> 24) & 15) as u8;
        let channel = ((word >> 16) & 15) as u8;
        let velocity = if kind == 2 {
            word & 127
        } else {
            let Some(second) = integer(&event["words"][1]) else {
                continue;
            };
            (second >> 16) & 65535
        };
        if status == 9 && velocity > 0 {
            held.entry((group, channel, pitch))
                .or_default()
                .push_back(notes.len());
            notes.push(Note {
                pitch,
                group,
                channel,
                start: tick,
                end: None,
            });
        } else if let Some(queue) = held.get_mut(&(group, channel, pitch)) {
            if let Some(index) = queue.front().copied().filter(|i| notes[*i].start <= tick) {
                notes[index].end = Some(tick);
                queue.pop_front();
            }
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pairs_midi1_and_midi2_by_group_channel_and_pitch() {
        let events = json!([
            {"tick": 0.0, "words": [0x40903c00u32, 0x7fff0000u32]},
            {"tick": 10, "words": [0x40903c00u32, 0x7fff0000u32]},
            {"tick": 20, "words": [0x41903c00u32, 0x7fff0000u32]},
            {"tick": 30, "words": [0x40803c00u32, 0.0]},
            {"tick": 40, "words": [0x20913d7fu32]},
            {"tick": 50, "words": [0x20913d00u32]},
            {"tick": 60, "words": [0x40903c00u32, 0]},
            {"tick": 70, "words": [0x40903e00u32]}
        ]);
        let result = notes_at(&events, "tick");
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].end, Some(30));
        assert_eq!(result[1].end, Some(60));
        assert_eq!(result[2].end, None);
        assert_eq!(result[3].end, Some(50));
    }
}
