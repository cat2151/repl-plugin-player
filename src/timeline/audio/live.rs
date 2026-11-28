use super::{
    peaks::{LiveAnalysis, Waveform},
    pitch::Detector,
};
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    time::Duration,
};

const FINALIZED: u32 = u32::MAX;
const BATCH_FRAMES: u32 = 2048;

pub(super) struct Preview {
    target: Arc<AtomicU32>,
    wake: SyncSender<()>,
    pub(super) waveform: Arc<Mutex<Option<Waveform>>>,
}

impl Preview {
    pub(super) fn start(path: PathBuf, ctx: egui::Context) -> Option<Self> {
        let (wake, requests) = mpsc::sync_channel(1);
        let target = Arc::new(AtomicU32::new(0));
        let waveform = Arc::new(Mutex::new(None));
        let requested_frames = target.clone();
        let output = waveform.clone();
        std::thread::Builder::new()
            .name("timeline-recording-preview".into())
            .spawn(move || analyze(&path, &ctx, &requested_frames, &output, requests))
            .ok()?;
        Some(Self {
            target,
            wake,
            waveform,
        })
    }

    pub(super) fn update(&self, frames: Option<u32>) {
        let frames = frames.unwrap_or(FINALIZED);
        if self.target.fetch_max(frames, Ordering::Relaxed) < frames {
            let _ = self.wake.try_send(());
        }
    }
}

fn analyze(
    path: &Path,
    ctx: &egui::Context,
    target: &AtomicU32,
    output: &Mutex<Option<Waveform>>,
    requests: Receiver<()>,
) {
    let detector = Detector::load();
    let mut analysis = None;
    let mut processed = 0;
    loop {
        let limit = target.load(Ordering::Relaxed);
        if limit == processed {
            if requests.recv().is_err() {
                return;
            }
            continue;
        }
        // Also check for cancellation between batches when catching up on a long take.
        if requests.try_recv() == Err(mpsc::TryRecvError::Disconnected) {
            return;
        }
        match read_batch(path, processed, limit) {
            Ok((spec, frames, samples)) => {
                if !samples.is_empty() {
                    analysis
                        .get_or_insert_with(|| LiveAnalysis::new(spec, detector.as_ref()))
                        .append(output, &samples);
                    processed += frames;
                    ctx.request_repaint();
                }
                if limit == FINALIZED && frames < BATCH_FRAMES {
                    return;
                }
            }
            Err(_) if limit == FINALIZED => return,
            Err(_) => {
                // The writer may have been updating the header when it was read.
                if requests.recv_timeout(Duration::from_millis(20))
                    == Err(mpsc::RecvTimeoutError::Disconnected)
                {
                    return;
                }
            }
        }
    }
}

fn read_batch(
    path: &Path,
    start: u32,
    limit: u32,
) -> hound::Result<(hound::WavSpec, u32, Vec<f32>)> {
    let mut wav = hound::WavReader::open(path)?;
    let spec = wav.spec();
    if spec.channels == 0
        || (limit != FINALIZED && wav.duration() < limit)
        || wav.duration() < start
    {
        return Err(hound::Error::FormatError("Incomplete recording WAV header"));
    }
    let frames = (wav.duration().min(limit) - start).min(BATCH_FRAMES);
    wav.seek(start)?;
    let count = frames as usize * usize::from(spec.channels);
    let samples: Vec<_> = wav.samples::<f32>().take(count).collect::<Result<_, _>>()?;
    if samples.len() != count {
        return Err(hound::Error::FormatError("Incomplete recording WAV data"));
    }
    Ok((spec, frames, samples))
}
