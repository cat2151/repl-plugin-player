//! The loader's [`PreloadBackend`]: decides per request between reloading the
//! slot's own instance and swapping in a spare.
//!
//! A spare receives its state *before* it is attached, so a failed load
//! leaves the slot exactly as it was.
use std::collections::HashMap;

use crate::{
    builder::BuilderWorker,
    host::{InstanceHandle, PluginKind, SharedHost},
    ownership::{Ownership, SLOT_COUNT},
    pool::{LiveInstances, SparePool},
    preload::PreloadBackend,
};

use super::Event;

pub struct LoadRequest {
    pub kind: PluginKind,
    pub state: Vec<u8>,
}

pub struct Job {
    slot: usize,
    request: LoadRequest,
    phase: Phase,
}

enum Phase {
    WaitSlot,
    WaitSpare,
    Loading { ticket: u64, target: Target },
}

#[derive(Clone, Copy)]
enum Target {
    Attached,
    Spare(InstanceHandle),
}

pub struct LoaderBackend {
    pub host: SharedHost,
    pub ownership: std::sync::Arc<Ownership>,
    pub live: LiveInstances,
    pub spares: SparePool,
    pub builder: BuilderWorker,
    events: std::sync::mpsc::Sender<Event>,
    loaded: HashMap<u64, Result<(), String>>,
    next_ticket: u64,
    /// Abandoned loads whose callback has not arrived yet; they keep their
    /// claim (and their spare) until it does.
    orphans: Vec<Job>,
    /// The in-flight job of this slot ended while still holding its claim.
    unsettled: [bool; SLOT_COUNT],
}

impl LoaderBackend {
    pub fn new(
        host: SharedHost,
        ownership: std::sync::Arc<Ownership>,
        builder: BuilderWorker,
        events: std::sync::mpsc::Sender<Event>,
    ) -> Self {
        Self {
            host,
            ownership,
            live: LiveInstances::default(),
            spares: SparePool::default(),
            builder,
            events,
            loaded: HashMap::new(),
            next_ticket: 1,
            orphans: Vec::new(),
            unsettled: [false; SLOT_COUNT],
        }
    }

    pub fn state_loaded(&mut self, ticket: u64, result: Result<(), String>) {
        self.loaded.insert(ticket, result);
    }

    /// Releases the claim of a finished in-flight job; `live` hands the slot
    /// to playback. No-op for requests that never claimed.
    pub fn finish(&mut self, slot: u8, live: bool) {
        let slot = usize::from(slot);
        if std::mem::take(&mut self.unsettled[slot]) {
            self.ownership.settle(slot, live);
        }
    }

    pub fn has_orphans(&self) -> bool {
        !self.orphans.is_empty()
    }

    /// Finishes abandoned loads whose callback has arrived.
    pub fn reap(&mut self) {
        let orphans = std::mem::take(&mut self.orphans);
        for job in orphans {
            let Phase::Loading { ticket, target } = job.phase else {
                continue;
            };
            if self.loaded.remove(&ticket).is_none() {
                self.orphans.push(job);
                continue;
            }
            if let Target::Spare(handle) = target {
                self.spares.put(job.request.kind, handle);
            }
            self.ownership.settle(job.slot, false);
        }
    }

    fn start_load(&mut self, job: &mut Job, target: Target, handle: InstanceHandle) {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let events = self.events.clone();
        self.host.load_state(
            handle,
            &job.request.state,
            Box::new(move |result| {
                let _ = events.send(Event::StateLoaded { ticket, result });
            }),
        );
        job.phase = Phase::Loading { ticket, target };
    }

    /// Puts a loaded spare on the slot; the evicted instance goes to its bag.
    fn swap_in(
        &mut self,
        slot: usize,
        kind: PluginKind,
        spare: InstanceHandle,
    ) -> Result<(), String> {
        let previous = self.live.get(slot).map(|(_, handle)| *handle);
        let detached = self.host.detach_from_slot(slot as u8);
        debug_assert_eq!(detached, previous);
        if !self.host.attach_to_slot(slot as u8, spare) {
            if let Some(previous) = previous {
                self.host.attach_to_slot(slot as u8, previous);
            }
            self.spares.put(kind, spare);
            return Err("attach_to_slot failed".to_string());
        }
        if let Some((old_kind, old)) = self.live.replace(slot, kind, spare) {
            self.spares.put(old_kind, old);
        }
        Ok(())
    }
}

impl PreloadBackend for LoaderBackend {
    type Request = LoadRequest;
    type Job = Job;

    fn begin(&mut self, slot: u8, request: LoadRequest) -> Result<Job, String> {
        let slot = usize::from(slot);
        if slot >= SLOT_COUNT {
            return Err(format!("slot {slot} does not exist"));
        }
        Ok(Job {
            slot,
            request,
            phase: Phase::WaitSlot,
        })
    }

    fn poll(&mut self, job: &mut Job) -> Option<Result<(), String>> {
        loop {
            match job.phase {
                Phase::WaitSlot => {
                    // Playback still owns it: touch nothing until it is retired.
                    if !self.ownership.claim(job.slot) {
                        return None;
                    }
                    self.unsettled[job.slot] = true;
                    match self.live.get(job.slot) {
                        Some((kind, handle)) if *kind == job.request.kind => {
                            let handle = *handle;
                            self.start_load(job, Target::Attached, handle);
                        }
                        _ => {
                            self.spares.refill(&job.request.kind, &self.builder);
                            job.phase = Phase::WaitSpare;
                        }
                    }
                }
                Phase::WaitSpare => {
                    let kind = &job.request.kind;
                    if let Some(spare) = self.spares.take(kind) {
                        self.spares.refill(kind, &self.builder);
                        self.start_load(job, Target::Spare(spare), spare);
                        continue;
                    }
                    if let Some(error) = self.spares.take_failure(kind) {
                        return Some(Err(error));
                    }
                    if self.spares.outstanding(kind) == 0 {
                        self.spares.refill(kind, &self.builder);
                        if self.spares.outstanding(kind) == 0 {
                            return Some(Err("the builder is not running".to_string()));
                        }
                    }
                    return None;
                }
                Phase::Loading { ticket, target } => {
                    let result = self.loaded.remove(&ticket)?;
                    return Some(match (result, target) {
                        (Ok(()), Target::Attached) => Ok(()),
                        (Ok(()), Target::Spare(spare)) => {
                            self.swap_in(job.slot, job.request.kind.clone(), spare)
                        }
                        (Err(error), Target::Spare(spare)) => {
                            self.spares.put(job.request.kind.clone(), spare);
                            Err(error)
                        }
                        (Err(error), Target::Attached) => Err(error),
                    });
                }
            }
        }
    }

    fn abandon(&mut self, job: Job) {
        match job.phase {
            Phase::WaitSlot => {}
            Phase::WaitSpare => {
                self.unsettled[job.slot] = false;
                self.ownership.settle(job.slot, false);
            }
            Phase::Loading { .. } => {
                self.unsettled[job.slot] = false;
                self.orphans.push(job);
            }
        }
    }
}
