use crate::capture;
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
};

pub(crate) struct Monitor {
    commands: mpsc::Sender<bool>,
    level: Arc<AtomicU32>,
    worker: JoinHandle<()>,
}

impl Monitor {
    pub(crate) fn new() -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        let level = Arc::new(AtomicU32::new(capture::NO_SAMPLES));
        let input_level = level.clone();
        let worker = thread::Builder::new()
            .name("microphone-monitor".into())
            .spawn(move || {
                let mut stream = None;
                while let Ok(open) = receiver.recv() {
                    if open {
                        if stream.is_none() {
                            stream = capture::monitor(input_level.clone()).ok();
                        }
                    } else {
                        drop(stream.take());
                        input_level.store(capture::NO_SAMPLES, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            commands,
            level,
            worker,
        })
    }

    pub(crate) fn set_open(&self, open: bool) -> Result<(), String> {
        self.commands.send(open).map_err(|error| error.to_string())
    }

    pub(crate) fn input_level(&self) -> Option<f32> {
        let bits = self.level.load(Ordering::Relaxed);
        (bits != capture::NO_SAMPLES).then(|| f32::from_bits(bits))
    }

    pub(crate) fn shutdown(self) {
        drop(self.commands);
        let _ = self.worker.join();
    }
}
