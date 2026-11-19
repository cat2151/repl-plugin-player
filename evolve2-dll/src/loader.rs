//! `LoaderWorker`: owns the instances and applies requests one at a time.
//!
//! Callers only enqueue: [`Loader::submit`] returns a request id at once and
//! the settlement is collected with [`Loader::poll`]. The worker thread is the
//! only owner of `LiveInstances` and `SparePool`, so a swap (take a spare,
//! attach, return the evicted one) finishes on one thread without locks.
mod backend;
#[cfg(test)]
mod tests;

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::{
    builder::{BuildOutcome, BuilderWorker},
    host::{InstanceHandle, PluginKind, SharedHost},
    ownership::{Ownership, SLOT_COUNT},
    preload::{Generation, PreloadOutcome, PreloadTracker},
};

use backend::{LoadRequest, LoaderBackend};

/// While something is in flight the worker also wakes on this period, because
/// playback hands slots back through an atomic, not through the channel.
const TICK: Duration = Duration::from_millis(2);
/// How long close waits for host callbacks of abandoned loads.
const CLOSE_WAIT: Duration = Duration::from_secs(10);

pub(crate) enum Event {
    Submit {
        request_id: u32,
        slot: u8,
        request: LoadRequest,
        generation: u64,
    },
    Built(BuildOutcome),
    StateLoaded {
        ticket: u64,
        result: Result<(), String>,
    },
    Shutdown,
}

/// What the loader currently holds, for status and tests.
#[derive(Clone, Default, Debug)]
pub struct View {
    pub slots: [Option<(PluginKind, InstanceHandle)>; SLOT_COUNT],
    pub spares: Vec<(PluginKind, usize)>,
}

impl View {
    /// Instances of `kind` on slots plus in its bag.
    pub fn physical(&self, kind: &PluginKind) -> usize {
        let on_slots = self
            .slots
            .iter()
            .flatten()
            .filter(|(k, _)| k == kind)
            .count();
        let spare = self
            .spares
            .iter()
            .filter(|(k, _)| k == kind)
            .map(|(_, count)| count)
            .sum::<usize>();
        on_slots + spare
    }
}

pub struct Loader {
    events: Sender<Event>,
    outcomes: Arc<Mutex<VecDeque<PreloadOutcome>>>,
    view: Arc<Mutex<View>>,
    generation: Generation,
    next_id: AtomicU32,
    thread: Option<JoinHandle<()>>,
}

impl Loader {
    pub fn spawn(host: SharedHost, ownership: Arc<Ownership>) -> Self {
        let (events, received) = channel();
        let built = events.clone();
        let builder = BuilderWorker::spawn(
            Arc::clone(&host),
            Box::new(move |outcome| {
                let _ = built.send(Event::Built(outcome));
            }),
        );
        let backend = LoaderBackend::new(host, ownership, builder, events.clone());
        let outcomes = Arc::new(Mutex::new(VecDeque::new()));
        let view = Arc::new(Mutex::new(View::default()));
        let generation: Generation = Arc::new(AtomicU64::new(0));
        let worker = Worker {
            backend,
            tracker: PreloadTracker::default(),
            generation: Arc::clone(&generation),
            outcomes: Arc::clone(&outcomes),
            view: Arc::clone(&view),
        };
        let thread = std::thread::Builder::new()
            .name("evolve2-loader".into())
            .spawn(move || worker.run(&received))
            .ok();
        Self {
            events,
            outcomes,
            view,
            generation,
            next_id: AtomicU32::new(1),
            thread,
        }
    }

    /// Returns at once with a request id; 0 means rejected.
    pub fn submit(&self, slot: u8, kind: PluginKind, state: Vec<u8>, generation: u64) -> u32 {
        if usize::from(slot) >= SLOT_COUNT || self.thread.is_none() {
            return 0;
        }
        let mut request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if request_id == 0 {
            request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        }
        let sent = self.events.send(Event::Submit {
            request_id,
            slot,
            request: LoadRequest { kind, state },
            generation,
        });
        if sent.is_ok() {
            request_id
        } else {
            0
        }
    }

    pub fn poll(&self) -> Vec<PreloadOutcome> {
        self.outcomes
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn view(&self) -> View {
        self.view
            .lock()
            .map(|view| view.clone())
            .unwrap_or_default()
    }
}

impl Drop for Loader {
    /// Folds pending requests and destroys every instance the loader owns.
    fn drop(&mut self) {
        let _ = self.events.send(Event::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Worker {
    backend: LoaderBackend,
    tracker: PreloadTracker<LoaderBackend>,
    generation: Generation,
    outcomes: Arc<Mutex<VecDeque<PreloadOutcome>>>,
    view: Arc<Mutex<View>>,
}

impl Worker {
    fn run(mut self, events: &Receiver<Event>) {
        loop {
            let busy = !self.tracker.is_idle() || self.backend.has_orphans();
            let event = if busy {
                match events.recv_timeout(TICK) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => Some(Event::Shutdown),
                }
            } else {
                Some(events.recv().unwrap_or(Event::Shutdown))
            };
            let mut settled = Vec::new();
            match event {
                Some(Event::Shutdown) => break,
                Some(Event::Submit {
                    request_id,
                    slot,
                    request,
                    generation,
                }) => settled.extend(self.tracker.submit(
                    &mut self.backend,
                    &self.generation,
                    request_id,
                    slot,
                    request,
                    generation,
                )),
                Some(other) => self.absorb(other),
                None => {}
            }
            settled.extend(self.tracker.advance(&mut self.backend, &self.generation));
            self.backend.reap();
            self.publish(settled);
        }
        self.close(events);
    }

    fn absorb(&mut self, event: Event) {
        match event {
            Event::Built(outcome) => self.backend.spares.accept(outcome),
            Event::StateLoaded { ticket, result } => self.backend.state_loaded(ticket, result),
            Event::Submit { .. } | Event::Shutdown => {}
        }
    }

    fn publish(&mut self, settled: Vec<PreloadOutcome>) {
        for outcome in &settled {
            let live = !outcome.stale && outcome.error.is_none();
            self.backend.finish(outcome.slot, live);
        }
        if let Ok(mut queue) = self.outcomes.lock() {
            queue.extend(settled);
        }
        if let Ok(mut view) = self.view.lock() {
            *view = View {
                slots: self.backend.live.snapshot(),
                spares: self.backend.spares.counts(),
            };
        }
    }

    fn close(mut self, events: &Receiver<Event>) {
        let folded = self.tracker.cancel(&mut self.backend);
        self.publish(folded);
        self.backend.builder.stop();
        let deadline = Instant::now() + CLOSE_WAIT;
        while self.backend.has_orphans() && Instant::now() < deadline {
            if let Ok(event) = events.recv_timeout(TICK) {
                self.absorb(event);
            }
            self.backend.reap();
        }
        while let Ok(event) = events.try_recv() {
            self.absorb(event);
        }
        let host = Arc::clone(&self.backend.host);
        for slot in 0..SLOT_COUNT {
            if let Some((_, handle)) = self.backend.live.take(slot) {
                host.detach_from_slot(slot as u8);
                host.destroy_instance(handle);
            }
        }
        for handle in self.backend.spares.drain() {
            host.destroy_instance(handle);
        }
        self.publish(Vec::new());
    }
}
