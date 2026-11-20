//! The groups `evolve.open` configures. A group is two tracks that crossfade
//! into each other; slot `2 * group + i` plays the group's track `i`.
//!
//! A `Chord` group gets its notes from evolve's chord clock and is measured
//! for its loudness trim. A `Timeline` group plays the events the track
//! already receives and is neither measured nor trimmed.

pub const GROUP_TRACKS: usize = 2;
/// Slots are numbered with `u8` towards the host.
const MAX_SLOTS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Chord,
    Timeline,
}

impl Input {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "chord" => Some(Self::Chord),
            "timeline" => Some(Self::Timeline),
            _ => None,
        }
    }
}

pub struct Layout {
    /// The track of each slot.
    tracks: Box<[i32]>,
    inputs: Box<[Input]>,
}

impl Layout {
    /// `None` without groups, with a negative or repeated track, or with
    /// more slots than the host can number.
    pub fn new(groups: &[([i32; GROUP_TRACKS], Input)]) -> Option<Self> {
        let tracks: Box<[i32]> = groups.iter().flat_map(|(tracks, _)| *tracks).collect();
        let repeated = tracks
            .iter()
            .enumerate()
            .any(|(i, track)| *track < 0 || tracks[..i].contains(track));
        if groups.is_empty() || repeated || tracks.len() > MAX_SLOTS {
            return None;
        }
        Some(Self {
            tracks,
            inputs: groups.iter().map(|(_, input)| *input).collect(),
        })
    }

    pub fn slot_count(&self) -> usize {
        self.tracks.len()
    }

    pub fn group_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn input(&self, group: usize) -> Input {
        self.inputs[group]
    }

    pub fn group_of(slot: usize) -> usize {
        slot / GROUP_TRACKS
    }

    /// The other slot of the slot's group.
    pub fn partner(slot: usize) -> usize {
        const { assert!(GROUP_TRACKS == 2) };
        slot ^ 1
    }

    /// The slots of `group`.
    pub fn slots(group: usize) -> std::ops::Range<usize> {
        group * GROUP_TRACKS..(group + 1) * GROUP_TRACKS
    }

    pub fn tracks(&self, group: usize) -> &[i32] {
        &self.tracks[Self::slots(group)]
    }

    /// Lock-free; the audio callbacks use it.
    pub fn slot_of(&self, track: i32) -> Option<usize> {
        self.tracks.iter().position(|&t| t == track)
    }

    /// The track the host processes first in a block (it goes in ascending
    /// order), where a block begins.
    pub fn first_track(&self) -> i32 {
        self.tracks.iter().copied().min().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_follow_the_groups_and_bad_layouts_are_rejected() {
        let layout = Layout::new(&[([4, 2], Input::Chord), ([7, 3], Input::Timeline)]).unwrap();
        assert_eq!(layout.slot_count(), 4);
        assert_eq!(layout.slot_of(3), Some(3));
        assert_eq!(layout.slot_of(2), Some(1));
        assert_eq!(layout.slot_of(5), None);
        assert_eq!(layout.first_track(), 2);
        assert_eq!(layout.tracks(1), [7, 3]);
        assert_eq!(Layout::group_of(3), 1);
        assert_eq!((Layout::partner(2), Layout::partner(3)), (3, 2));
        assert!(Layout::new(&[]).is_none());
        assert!(Layout::new(&[([1, 1], Input::Chord)]).is_none());
        assert!(Layout::new(&[([0, 1], Input::Chord), ([2, 1], Input::Chord)]).is_none());
        assert!(Layout::new(&[([-1, 1], Input::Chord)]).is_none());
        assert_eq!(Input::parse("timeline"), Some(Input::Timeline));
        assert_eq!(Input::parse("clip"), None);
    }
}
