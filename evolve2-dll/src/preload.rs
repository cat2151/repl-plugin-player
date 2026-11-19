//! One in-flight load plus a queue of loads waiting to be begun.
//!
//! The tracker never blocks: the loader loop calls [`PreloadTracker::advance`]
//! every turn instead of waiting for a completion.
//!
//! ## Generation
//!
//! A load already handed to the host cannot be cancelled, so completions of a
//! folded cycle still arrive. Each request carries the generation it was
//! submitted under; a settlement whose generation differs is `stale` and must
//! not count as progress (it is still drained).
//!
//! **Read the current generation right before each decision**: just after a
//! completion is observed, and just after a waiting entry is popped. Reusing a
//! value read at the top of the loop lets a bump that lands in between pass a
//! folded completion as live. Bumpers `fetch_add` before anything that could
//! release a completion, so a load after observing it sees the new value.
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

/// Shared generation counter; bumped by whoever folds a cycle.
pub type Generation = Arc<AtomicU64>;

/// The host side that actually begins and completes a load.
pub trait PreloadBackend {
    /// What the submitter asked to load.
    type Request;
    /// Handle of a load that has been begun.
    type Job;
    fn begin(&mut self, slot: u8, request: Self::Request) -> Result<Self::Job, String>;
    /// `None` while the load is still running. Must not block.
    fn poll(&mut self, job: &mut Self::Job) -> Option<Result<(), String>>;
    /// Gives up ownership of a begun load that is no longer wanted.
    fn abandon(&mut self, job: Self::Job);
}

/// How one request was settled. `stale` outcomes count as neither progress
/// nor failure.
#[derive(Debug, Clone, PartialEq)]
pub struct PreloadOutcome {
    pub request_id: u32,
    pub slot: u8,
    /// From begin to settlement; zero when the request never began.
    pub elapsed: Duration,
    pub error: Option<String>,
    pub stale: bool,
}

struct InFlight<J> {
    job: J,
    request_id: u32,
    slot: u8,
    generation: u64,
    started: Instant,
}

struct Waiting<R> {
    request: R,
    request_id: u32,
    slot: u8,
    generation: u64,
}

pub struct PreloadTracker<B: PreloadBackend> {
    in_flight: Option<InFlight<B::Job>>,
    waiting: VecDeque<Waiting<B::Request>>,
}

impl<B: PreloadBackend> Default for PreloadTracker<B> {
    fn default() -> Self {
        Self {
            in_flight: None,
            waiting: VecDeque::new(),
        }
    }
}

impl<B: PreloadBackend> PreloadTracker<B> {
    /// Queues one request and advances. Does not wait for the load.
    ///
    /// `generation` is the submitter's generation; the current one is read
    /// from `current` at each decision.
    pub fn submit(
        &mut self,
        backend: &mut B,
        current: &AtomicU64,
        request_id: u32,
        slot: u8,
        request: B::Request,
        generation: u64,
    ) -> Vec<PreloadOutcome> {
        self.waiting.push_back(Waiting {
            request,
            request_id,
            slot,
            generation,
        });
        self.advance(backend, current)
    }

    /// Polls the in-flight load once and, when the seat is free, begins the
    /// next waiting request whose generation is still current.
    pub fn advance(&mut self, backend: &mut B, current: &AtomicU64) -> Vec<PreloadOutcome> {
        let mut outcomes = Vec::new();
        loop {
            if let Some(outcome) = self.poll_in_flight(backend, current) {
                outcomes.push(outcome);
            }
            if self.in_flight.is_some() {
                break;
            }
            let Some(next) = self.waiting.pop_front() else {
                break;
            };
            if next.generation != current.load(Ordering::SeqCst) {
                outcomes.push(PreloadOutcome {
                    request_id: next.request_id,
                    slot: next.slot,
                    elapsed: Duration::ZERO,
                    error: None,
                    stale: true,
                });
                continue;
            }
            let started = Instant::now();
            match backend.begin(next.slot, next.request) {
                Ok(job) => {
                    self.in_flight = Some(InFlight {
                        job,
                        request_id: next.request_id,
                        slot: next.slot,
                        generation: next.generation,
                        started,
                    });
                    break;
                }
                Err(error) => outcomes.push(PreloadOutcome {
                    request_id: next.request_id,
                    slot: next.slot,
                    elapsed: started.elapsed(),
                    error: Some(error),
                    stale: false,
                }),
            }
        }
        outcomes
    }

    /// Folds everything: waiting requests become stale and the in-flight load
    /// is abandoned to the backend.
    pub fn cancel(&mut self, backend: &mut B) -> Vec<PreloadOutcome> {
        let mut outcomes: Vec<_> = self
            .waiting
            .drain(..)
            .map(|waiting| PreloadOutcome {
                request_id: waiting.request_id,
                slot: waiting.slot,
                elapsed: Duration::ZERO,
                error: None,
                stale: true,
            })
            .collect();
        if let Some(entry) = self.in_flight.take() {
            outcomes.push(PreloadOutcome {
                request_id: entry.request_id,
                slot: entry.slot,
                elapsed: entry.started.elapsed(),
                error: None,
                stale: true,
            });
            backend.abandon(entry.job);
        }
        outcomes
    }

    pub fn is_idle(&self) -> bool {
        self.in_flight.is_none() && self.waiting.is_empty()
    }

    fn poll_in_flight(&mut self, backend: &mut B, current: &AtomicU64) -> Option<PreloadOutcome> {
        let entry = self.in_flight.as_mut()?;
        let error = backend.poll(&mut entry.job)?.err();
        // Read only after the completion was observed (see the module doc).
        let current = current.load(Ordering::SeqCst);
        let entry = self.in_flight.take()?;
        Some(PreloadOutcome {
            request_id: entry.request_id,
            slot: entry.slot,
            elapsed: entry.started.elapsed(),
            error,
            stale: entry.generation != current,
        })
    }
}
