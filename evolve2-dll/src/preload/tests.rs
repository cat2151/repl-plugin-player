//! The fake bumps the generation inside `poll` at the instant it reports a
//! completion, which reproduces the narrowest race without extra threads.
use super::*;

struct FakeBackend {
    generation: Generation,
    /// Loads report completion only when this is set.
    complete: bool,
    /// Bump the generation at the instant a completion is reported.
    fold_on_poll: bool,
    fail_begin: bool,
    begun: Vec<u8>,
    abandoned: Vec<u8>,
}

impl FakeBackend {
    fn new(generation: &Generation) -> Self {
        Self {
            generation: Arc::clone(generation),
            complete: false,
            fold_on_poll: false,
            fail_begin: false,
            begun: Vec::new(),
            abandoned: Vec::new(),
        }
    }
}

impl PreloadBackend for FakeBackend {
    type Request = &'static str;
    type Job = u8;

    fn begin(&mut self, slot: u8, _request: &'static str) -> Result<u8, String> {
        if self.fail_begin {
            return Err("begin failed".into());
        }
        self.begun.push(slot);
        Ok(slot)
    }

    fn poll(&mut self, _job: &mut u8) -> Option<Result<(), String>> {
        if !self.complete {
            return None;
        }
        if self.fold_on_poll {
            self.fold_on_poll = false;
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        Some(Ok(()))
    }

    fn abandon(&mut self, job: u8) {
        self.abandoned.push(job);
    }
}

fn setup() -> (Generation, FakeBackend, PreloadTracker<FakeBackend>) {
    let generation: Generation = Arc::new(AtomicU64::new(1));
    let backend = FakeBackend::new(&generation);
    (generation, backend, PreloadTracker::default())
}

#[test]
fn a_completion_observed_after_the_bump_is_stale_and_not_progress() {
    let (generation, mut backend, mut tracker) = setup();
    assert!(tracker
        .submit(&mut backend, &generation, 7, 1, "a", 1)
        .is_empty());

    backend.complete = true;
    backend.fold_on_poll = true;
    let outcomes = tracker.advance(&mut backend, &generation);

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].request_id, 7);
    assert!(outcomes[0].stale);
    assert!(outcomes[0].error.is_none());
    assert!(tracker.is_idle());
}

#[test]
fn a_completion_in_the_same_generation_is_live() {
    let (generation, mut backend, mut tracker) = setup();
    tracker.submit(&mut backend, &generation, 1, 0, "a", 1);
    backend.complete = true;
    let outcomes = tracker.advance(&mut backend, &generation);
    assert_eq!(outcomes.len(), 1);
    assert!(!outcomes[0].stale);
}

#[test]
fn a_second_request_waits_in_the_queue_while_one_is_in_flight() {
    let (generation, mut backend, mut tracker) = setup();
    tracker.submit(&mut backend, &generation, 1, 0, "a", 1);
    let outcomes = tracker.submit(&mut backend, &generation, 2, 1, "b", 1);

    assert!(outcomes.is_empty());
    assert_eq!(backend.begun, vec![0], "only one load is begun at a time");

    backend.complete = true;
    let first = tracker.advance(&mut backend, &generation);
    assert_eq!(backend.begun, vec![0, 1]);
    let second = tracker.advance(&mut backend, &generation);
    let settled: Vec<_> = first.iter().chain(&second).collect();
    assert_eq!(
        settled.iter().map(|o| o.request_id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(settled.iter().all(|o| !o.stale));
    assert!(tracker.is_idle());
}

#[test]
fn a_waiting_request_is_not_begun_when_the_generation_changed_before_it_was_popped() {
    let (generation, mut backend, mut tracker) = setup();
    tracker.submit(&mut backend, &generation, 1, 0, "a", 1);
    tracker.submit(&mut backend, &generation, 2, 1, "b", 1);

    backend.complete = true;
    backend.fold_on_poll = true;
    let outcomes = tracker.advance(&mut backend, &generation);

    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.stale));
    assert_eq!(
        backend.begun,
        vec![0],
        "the waiting one never reaches begin"
    );
    assert_eq!(outcomes[1].elapsed, Duration::ZERO);
}

#[test]
fn a_request_from_an_older_generation_is_dropped_without_begin() {
    let (generation, mut backend, mut tracker) = setup();
    generation.fetch_add(1, Ordering::SeqCst);
    let outcomes = tracker.submit(&mut backend, &generation, 3, 0, "a", 1);
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].stale);
    assert!(backend.begun.is_empty());
}

#[test]
fn a_failed_begin_is_an_error_outcome_and_the_next_one_proceeds() {
    let (generation, mut backend, mut tracker) = setup();
    backend.fail_begin = true;
    let outcomes = tracker.submit(&mut backend, &generation, 1, 0, "a", 1);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].error.as_deref(), Some("begin failed"));
    assert!(!outcomes[0].stale);

    backend.fail_begin = false;
    tracker.submit(&mut backend, &generation, 2, 1, "b", 1);
    assert_eq!(backend.begun, vec![1]);
}

#[test]
fn cancel_turns_everything_stale_and_abandons_the_in_flight_load() {
    let (generation, mut backend, mut tracker) = setup();
    tracker.submit(&mut backend, &generation, 1, 0, "a", 1);
    tracker.submit(&mut backend, &generation, 2, 1, "b", 1);

    let outcomes = tracker.cancel(&mut backend);
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.stale));
    assert_eq!(backend.abandoned, vec![0]);
    assert!(tracker.is_idle());
}
