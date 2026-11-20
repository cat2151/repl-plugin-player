//! Deadline misses and the slots a miss retires.
//!
//! One block is one miss however many groups it plays, so a miss blames one
//! slot across every group: the latest engaged slot still measuring, else the
//! latest engaged slot fading in. Without either, the current slot of each
//! group that sounds alone is marked instead. A group whose current slot is
//! marked is left out of all of this until its next slot is engaged.
use std::time::{Duration, Instant};

use crate::layout::Layout;

pub(super) struct Misses {
    /// `begin` came and `finish` did not yet.
    open: bool,
    started: Option<Instant>,
    finished: Option<Instant>,
    period: Duration,
    block: u64,
    engaged_at: Box<[u64]>,
}

impl Misses {
    pub(super) fn new(slots: usize) -> Self {
        Self {
            open: false,
            started: None,
            finished: None,
            period: Duration::ZERO,
            block: 0,
            engaged_at: vec![0; slots].into_boxed_slice(),
        }
    }

    /// Starts timing a block; returns whether the previous one missed.
    pub(super) fn start(&mut self, frames: usize, rate: f64) -> bool {
        let now = Instant::now();
        self.block += 1;
        let dropped = std::mem::replace(&mut self.open, true);
        let late = match (self.started.replace(now), self.finished.take()) {
            (Some(started), Some(finished)) => finished - started > self.period,
            _ => false,
        };
        self.period = Duration::from_secs_f64(frames as f64 / rate);
        dropped || late
    }

    pub(super) fn finish(&mut self) {
        self.open = false;
        self.finished = Some(Instant::now());
    }

    pub(super) fn engaged(&mut self, slot: usize) {
        self.engaged_at[slot] = self.block;
    }

    /// Marks in `out` the picked slots engaged last; false when none is picked.
    fn latest(&self, pick: impl Fn(usize) -> bool, out: &mut [bool]) -> bool {
        let slots = 0..self.engaged_at.len();
        let Some(latest) = slots
            .clone()
            .filter(|&s| pick(s))
            .map(|s| self.engaged_at[s])
            .max()
        else {
            return false;
        };
        for s in slots {
            out[s] = pick(s) && self.engaged_at[s] == latest;
        }
        true
    }
}

/// What a group's crossfade is doing.
#[derive(Default)]
pub(super) struct Group {
    pub(super) current: Option<usize>,
    pub(super) elapsed: u64,
    pub(super) fade: Option<u64>,
    /// The current slot missed while it sounded alone.
    pub(super) doomed: Option<usize>,
}

impl Group {
    fn marked(&self) -> bool {
        self.current.is_some() && self.doomed == self.current
    }
}

/// Fills `skip` with the slots a miss retires and marks lone current slots;
/// see the module doc.
pub(super) fn blame(
    misses: &Misses,
    groups: &mut [Group],
    enabled: &[bool],
    heard: &[bool],
    skip: &mut [bool],
) {
    skip.fill(false);
    let open = |s: usize| !groups[Layout::group_of(s)].marked();
    if misses.latest(|s| open(s) && enabled[s] && !heard[s], skip) {
        return;
    }
    let fading_in = |s: usize| {
        let group = &groups[Layout::group_of(s)];
        open(s) && group.fade.is_some() && group.current == Some(Layout::partner(s))
    };
    if misses.latest(fading_in, skip) {
        return;
    }
    for group in groups.iter_mut().filter(|g| !g.marked()) {
        if let Some(current) = group.current.filter(|&c| heard[c]) {
            group.doomed = Some(current);
        }
    }
}
