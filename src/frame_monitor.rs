//! UI frame stalls, watched from outside the UI/control thread.
//!
//! The UI/control loop calls [`beat`] once per pass (each GUI frame, each CLI pump, each
//! E2E wait).
//! A separate thread reads the latest beat and writes every gap longer than four times
//! the median of the recent gaps to `<session log>.frames.jsonl` as
//! `{"startMs","endMs","ms","thresholdMs"}` (Unix epoch ms). The file is created by
//! the first stall. A gap still open when the monitor stops is written with
//! `"open": true` and the stop time as its end.
//!
//! While a gap is past the threshold, the monitor samples the call stack of the thread
//! that called [`start`] every period. The stall line then carries `"samples"` (how many),
//! `"functions"`: the frames found in the most samples, each with that count, and
//! `"stacks"`: the most frequent whole stacks, innermost frame first, each with its count.
//! A frame is `module!function` where the module has debug info, otherwise `module+0x..`.

mod stack;

use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, OnceLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Microseconds since [`EPOCH`] plus one; 0 means no beat yet.
static BEAT: AtomicU64 = AtomicU64::new(0);
static EPOCH: OnceLock<(Instant, SystemTime)> = OnceLock::new();
const WINDOW: usize = 64;
const MIN_INTERVALS: usize = 16;
const FACTOR: u64 = 4;
const PERIOD: Duration = Duration::from_millis(5);
/// Ten seconds of samples; a longer stall keeps its first ten seconds.
const MAX_SAMPLES: usize = 2000;
const STACKS: usize = 3;
const FUNCTIONS: usize = 30;

/// Marks one pass of the UI/control loop. Lock-free; does nothing before [`start`].
pub fn beat() {
    if let Some((epoch, _)) = EPOCH.get() {
        BEAT.store(epoch.elapsed().as_micros() as u64 + 1, Ordering::Relaxed);
    }
}

/// Stops and joins the watching thread when dropped.
pub struct Monitor {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Watches the beats of this process; `log` is the session log beside which the
/// stalls are written. Must be called on the thread that beats.
pub fn start(log: &std::path::Path) -> Monitor {
    let (epoch, wall) = *EPOCH.get_or_init(|| (Instant::now(), SystemTime::now()));
    let path = log.with_extension("frames.jsonl");
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let stacks = Sampler::open();
    let thread = std::thread::Builder::new()
        .name("frame-monitor".into())
        .spawn(move || watch(&stopping, epoch, wall, path, stacks))
        .ok();
    Monitor { stop, thread }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn watch(stop: &AtomicBool, epoch: Instant, wall: SystemTime, path: PathBuf, mut stacks: Sampler) {
    let base = wall
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as f64
        / 1000.0;
    let mut file = None;
    let mut write = |stall: Stall, samples: &[Vec<u64>], stacks: &mut Sampler, open: bool| {
        let mut line = json!({"startMs":base + stall.start as f64 / 1000.0,
            "endMs":base + stall.end as f64 / 1000.0,
            "ms":(stall.end - stall.start) as f64 / 1000.0,
            "thresholdMs":stall.threshold as f64 / 1000.0});
        if open {
            line["open"] = json!(true);
        }
        if !samples.is_empty() {
            line["samples"] = json!(samples.len());
            stacks.summarize(samples, &mut line);
        }
        if file.is_none() {
            file = std::fs::File::create(&path).ok();
        }
        if let Some(file) = &mut file {
            let _ = writeln!(file, "{line}").and_then(|_| file.flush());
        }
    };
    let mut detector = Detector::default();
    let mut samples = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(PERIOD);
        let beat = BEAT.load(Ordering::Relaxed);
        if beat != detector.last {
            if let Some(stall) = detector.observe(beat) {
                write(stall, &samples, &mut stacks, false);
            }
            samples.clear();
        }
        let now = epoch.elapsed().as_micros() as u64 + 1;
        if samples.len() < MAX_SAMPLES && detector.open(now).is_some() {
            if samples.is_empty() {
                stacks.refresh();
            }
            samples.push(stacks.sample());
        }
    }
    if let Some(stall) = detector.open(epoch.elapsed().as_micros() as u64 + 1) {
        write(stall, &samples, &mut stacks, true);
    }
}

/// Stacks of the beating thread; `None` (no access to it) writes no stacks.
struct Sampler(Option<stack::Stacks>);

impl Sampler {
    fn open() -> Self {
        let thread = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        Self(stack::Stacks::open(thread))
    }
    fn refresh(&mut self) {
        if let Some(stacks) = &mut self.0 {
            stacks.refresh();
        }
    }
    fn sample(&self) -> Vec<u64> {
        self.0
            .as_ref()
            .map(stack::Stacks::sample)
            .unwrap_or_default()
    }
    /// Writes `"functions"` and `"stacks"` of the samples into the stall `line`.
    fn summarize(&mut self, samples: &[Vec<u64>], line: &mut serde_json::Value) {
        let Some(stacks) = &mut self.0 else {
            return;
        };
        stacks.refresh_symbols();
        let mut names = HashMap::new();
        let mut whole: HashMap<Vec<String>, usize> = HashMap::new();
        let mut functions: HashMap<String, usize> = HashMap::new();
        for sample in samples.iter().filter(|s| !s.is_empty()) {
            let frames: Vec<String> = sample
                .iter()
                .map(|pc| {
                    names
                        .entry(*pc)
                        .or_insert_with(|| stacks.describe(*pc))
                        .clone()
                })
                .collect();
            let mut seen = HashSet::new();
            for frame in frames.iter().filter(|f| seen.insert(*f)) {
                *functions.entry(frame.clone()).or_default() += 1;
            }
            *whole.entry(frames).or_default() += 1;
        }
        line["functions"] = json!(most(functions, FUNCTIONS, |name, count| {
            json!({"count":count,"name":name})
        }));
        line["stacks"] = json!(most(whole, STACKS, |frames, count| {
            json!({"count":count,"frames":frames})
        }));
    }
}

fn most<K>(
    counts: HashMap<K, usize>,
    take: usize,
    line: impl Fn(K, usize) -> serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counts
        .into_iter()
        .take(take)
        .map(|(k, c)| line(k, c))
        .collect()
}

/// A gap between two beats, in the beat clock.
#[derive(Debug, PartialEq)]
struct Stall {
    start: u64,
    end: u64,
    threshold: u64,
}

#[derive(Default)]
struct Detector {
    last: u64,
    intervals: VecDeque<u64>,
    threshold: Option<u64>,
}

impl Detector {
    /// Feeds the latest beat; returns the gap it closed when that gap was a stall.
    fn observe(&mut self, beat: u64) -> Option<Stall> {
        if beat == self.last {
            return None;
        }
        let last = std::mem::replace(&mut self.last, beat);
        if last == 0 {
            return None;
        }
        let interval = beat - last;
        if let Some(threshold) = self.threshold.filter(|t| interval > *t) {
            return Some(Stall {
                start: last,
                end: beat,
                threshold,
            });
        }
        // Stalls stay out of the window so a long one does not raise the threshold.
        if self.intervals.len() == WINDOW {
            self.intervals.pop_front();
        }
        self.intervals.push_back(interval);
        if self.intervals.len() >= MIN_INTERVALS {
            let mut sorted: Vec<u64> = self.intervals.iter().copied().collect();
            sorted.sort_unstable();
            self.threshold = Some(sorted[sorted.len() / 2] * FACTOR);
        }
        None
    }
    /// The gap since the last beat, if it is already a stall at `now`.
    fn open(&self, now: u64) -> Option<Stall> {
        let threshold = self.threshold?;
        (self.last != 0 && now - self.last > threshold).then_some(Stall {
            start: self.last,
            end: now,
            threshold,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steady(detector: &mut Detector, from: u64, count: u64, every: u64) -> u64 {
        for index in 0..count {
            assert_eq!(detector.observe(from + index * every), None);
        }
        from + (count - 1) * every
    }

    #[test]
    fn a_gap_over_four_medians_is_one_stall_with_its_exact_bounds() {
        let mut detector = Detector::default();
        let last = steady(&mut detector, 1, 40, 10_000);
        assert_eq!(detector.observe(last), None);
        assert_eq!(
            detector.observe(last + 200_000),
            Some(Stall {
                start: last,
                end: last + 200_000,
                threshold: 40_000
            })
        );
        assert_eq!(detector.observe(last + 210_000), None);
    }

    #[test]
    fn gaps_within_the_threshold_and_before_the_window_fills_are_not_stalls() {
        let mut detector = Detector::default();
        let last = steady(&mut detector, 1, 10, 10_000);
        assert_eq!(detector.observe(last + 500_000), None, "no threshold yet");
        let last = steady(&mut detector, last + 510_000, 30, 10_000);
        assert_eq!(detector.observe(last + 39_000), None);
    }

    #[test]
    fn the_threshold_follows_the_median_pass_not_the_slowest() {
        let mut detector = Detector::default();
        let mut last = steady(&mut detector, 1, 30, 10_000);
        for _ in 0..5 {
            last += 35_000;
            assert_eq!(detector.observe(last), None);
        }
        assert!(detector.observe(last + 100_000).is_some());
    }

    #[test]
    fn a_stall_does_not_raise_the_threshold() {
        let mut detector = Detector::default();
        let last = steady(&mut detector, 1, 40, 10_000);
        for index in 1..=40 {
            assert!(detector.observe(last + index * 100_000).is_some());
        }
        assert_eq!(detector.threshold, Some(40_000));
    }

    #[test]
    fn an_unfinished_gap_is_reported_open_only_past_the_threshold() {
        let mut detector = Detector::default();
        let last = steady(&mut detector, 1, 40, 10_000);
        assert_eq!(detector.open(last + 30_000), None);
        assert_eq!(
            detector.open(last + 90_000),
            Some(Stall {
                start: last,
                end: last + 90_000,
                threshold: 40_000
            })
        );
    }
}
