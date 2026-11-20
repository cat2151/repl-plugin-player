//! `LiveInstances` (slot → physical instance) and `SparePool` (spares per kind).
//!
//! # Bound on physical instances
//! Builds are ordered only while `spares + outstanding < target`, and moving
//! an instance between a slot and the pool does not change its kind's count.
//! So per kind the count never exceeds the slot count plus `SPARE_TARGET`, without
//! eviction (same argument as clap-mml-play-server's `instances.rs`).
use std::collections::HashMap;

use crate::{
    builder::{BuildOutcome, BuilderWorker},
    host::{InstanceHandle, PluginKind},
};

/// Spares kept per kind that has ever been requested.
pub const SPARE_TARGET: usize = 1;

pub struct LiveInstances {
    slots: Vec<Option<(PluginKind, InstanceHandle)>>,
}

impl LiveInstances {
    pub fn new(slots: usize) -> Self {
        Self {
            slots: vec![None; slots],
        }
    }

    pub fn get(&self, slot: usize) -> Option<&(PluginKind, InstanceHandle)> {
        self.slots[slot].as_ref()
    }

    pub fn replace(
        &mut self,
        slot: usize,
        kind: PluginKind,
        handle: InstanceHandle,
    ) -> Option<(PluginKind, InstanceHandle)> {
        self.slots[slot].replace((kind, handle))
    }

    pub fn take(&mut self, slot: usize) -> Option<(PluginKind, InstanceHandle)> {
        self.slots[slot].take()
    }

    pub fn snapshot(&self) -> Vec<Option<(PluginKind, InstanceHandle)>> {
        self.slots.clone()
    }
}

#[derive(Default)]
pub struct SparePool {
    bags: HashMap<PluginKind, Vec<InstanceHandle>>,
    outstanding: HashMap<PluginKind, usize>,
    /// Last build failure per kind, kept until a waiting load reads it.
    failures: HashMap<PluginKind, String>,
}

impl SparePool {
    /// Orders builds until the kind has its target in hand or on order.
    pub fn refill(&mut self, kind: &PluginKind, builder: &BuilderWorker) {
        let have = self.bags.entry(kind.clone()).or_default().len();
        let outstanding = self.outstanding.entry(kind.clone()).or_default();
        if have + *outstanding < SPARE_TARGET {
            self.failures.remove(kind);
        }
        while have + *outstanding < SPARE_TARGET {
            if !builder.order(kind.clone()) {
                return;
            }
            *outstanding += 1;
        }
    }

    pub fn take(&mut self, kind: &PluginKind) -> Option<InstanceHandle> {
        self.bags.get_mut(kind)?.pop()
    }

    pub fn put(&mut self, kind: PluginKind, handle: InstanceHandle) {
        self.bags.entry(kind).or_default().push(handle);
    }

    pub fn accept(&mut self, outcome: BuildOutcome) {
        let outstanding = self.outstanding.entry(outcome.kind.clone()).or_default();
        *outstanding = outstanding.saturating_sub(1);
        match outcome.result {
            Ok(handle) => self.put(outcome.kind, handle),
            Err(error) => {
                self.failures.insert(outcome.kind, error);
            }
        }
    }

    pub fn outstanding(&self, kind: &PluginKind) -> usize {
        self.outstanding.get(kind).copied().unwrap_or(0)
    }

    pub fn take_failure(&mut self, kind: &PluginKind) -> Option<String> {
        self.failures.remove(kind)
    }

    pub fn counts(&self) -> Vec<(PluginKind, usize)> {
        self.bags
            .iter()
            .map(|(kind, bag)| (kind.clone(), bag.len()))
            .collect()
    }

    pub fn drain(&mut self) -> Vec<InstanceHandle> {
        self.bags.drain().flat_map(|(_, bag)| bag).collect()
    }
}
