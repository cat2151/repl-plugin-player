//! `BuilderWorker`: creates spare instances in the background, one at a time.
//!
//! Serial on purpose: parallel creation is not faster and some plugins crash
//! when instantiated concurrently (clap-mml-play-server ADR 0013).
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::host::{InstanceHandle, PluginKind, SharedHost};

pub struct BuildOutcome {
    pub kind: PluginKind,
    pub result: Result<InstanceHandle, String>,
    pub elapsed: Duration,
}

pub type Deliver = Box<dyn Fn(BuildOutcome) + Send>;

pub struct BuilderWorker {
    orders: Option<Sender<PluginKind>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl BuilderWorker {
    pub fn spawn(host: SharedHost, deliver: Deliver) -> Self {
        let (orders, received) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("evolve-builder".into())
            .spawn(move || run(&host, &received, &stopped, &deliver))
            .ok();
        Self {
            orders: thread.as_ref().map(|_| orders),
            stop,
            thread,
        }
    }

    /// `false` when the thread is not running.
    pub fn order(&self, kind: PluginKind) -> bool {
        self.orders
            .as_ref()
            .is_some_and(|orders| orders.send(kind).is_ok())
    }

    /// Orders still queued are delivered as errors without being created;
    /// waits for the creation in progress.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.orders = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for BuilderWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(host: &SharedHost, orders: &Receiver<PluginKind>, stop: &AtomicBool, deliver: &Deliver) {
    for kind in orders {
        let started = Instant::now();
        let result = if stop.load(Ordering::SeqCst) {
            Err("cancelled".to_string())
        } else {
            let (done, completed) = channel();
            host.create_instance(
                &kind,
                Box::new(move |result| {
                    let _ = done.send(result);
                }),
            );
            completed
                .recv()
                .unwrap_or_else(|_| Err("the host dropped the create callback".to_string()))
        };
        deliver(BuildOutcome {
            kind,
            result,
            elapsed: started.elapsed(),
        });
    }
}
