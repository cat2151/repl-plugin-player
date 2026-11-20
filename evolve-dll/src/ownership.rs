//! Who may touch each slot's instance: the loader or the playback side.
//!
//! The loader operates on a slot only after `claim` succeeds, so an instance
//! that is sounding (or ready to sound) is never detached or reloaded until
//! playback hands it back with `retire`. Lock-free so the audio side can use it.
use std::sync::atomic::{AtomicU32, Ordering};

pub const FREE: u32 = 0;
pub const LOADING: u32 = 1;
pub const READY: u32 = 2;
pub const ENGAGED: u32 = 3;

pub struct Ownership {
    states: Box<[AtomicU32]>,
}

impl Ownership {
    /// Every slot starts `FREE`.
    pub fn new(slots: usize) -> Self {
        Self {
            states: (0..slots).map(|_| AtomicU32::new(FREE)).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn get(&self, slot: usize) -> u32 {
        self.states[slot].load(Ordering::Acquire)
    }

    pub(crate) fn claim(&self, slot: usize) -> bool {
        self.swap(slot, FREE, LOADING)
    }

    /// Ends the loader's claim: a live load is handed to playback, anything
    /// else gives the slot back unchanged.
    pub(crate) fn settle(&self, slot: usize, live: bool) {
        self.swap(slot, LOADING, if live { READY } else { FREE });
    }

    /// Playback takes a ready slot.
    pub fn engage(&self, slot: usize) -> bool {
        self.swap(slot, READY, ENGAGED)
    }

    /// Playback gives a slot back to the loader.
    pub fn retire(&self, slot: usize) -> bool {
        self.swap(slot, ENGAGED, FREE)
    }

    fn swap(&self, slot: usize, from: u32, to: u32) -> bool {
        self.states[slot]
            .compare_exchange(from, to, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}
