use super::pitch::{Analyzer, Detector, Track};
use std::{io::Read, path::Path, sync::Mutex};

const MAX_BUCKETS: usize = 262_144;

#[derive(Clone, Copy)]
struct Peak {
    min: f32,
    max: f32,
}

impl Peak {
    const EMPTY: Self = Self {
        min: f32::INFINITY,
        max: f32::NEG_INFINITY,
    };

    fn merge(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

pub(super) struct Waveform {
    frames: usize,
    rate: f64,
    frames_per_bucket: usize,
    levels: Vec<Vec<Peak>>,
    tail: Peak,
    peak: Peak,
    pitch: Option<Track>,
}

impl Waveform {
    pub(super) fn read(path: &Path, detector: Option<&Detector>) -> hound::Result<Self> {
        Self::from_wav(hound::WavReader::open(path)?, detector)
    }

    fn from_wav(
        reader: hound::WavReader<impl Read>,
        detector: Option<&Detector>,
    ) -> hound::Result<Self> {
        let mut reader = reader;
        let frames = reader.duration() as usize;
        let spec = reader.spec();
        let mut analyzer = detector.and_then(|detector| Analyzer::new(detector, spec));
        let frames_per_bucket = frames.div_ceil(MAX_BUCKETS).next_power_of_two().max(32);
        let bucket_size = frames_per_bucket * usize::from(spec.channels);
        let bucket_count = frames.div_ceil(frames_per_bucket);
        let buckets = match spec.sample_format {
            hound::SampleFormat::Float => collect(
                reader.samples(),
                bucket_size,
                bucket_count,
                |x: f32| x,
                analyzer.as_mut(),
            ),
            hound::SampleFormat::Int => {
                let full_scale = 2.0_f32.powi(i32::from(spec.bits_per_sample) - 1);
                collect(
                    reader.samples(),
                    bucket_size,
                    bucket_count,
                    |x: i32| x as f32 / full_scale,
                    analyzer.as_mut(),
                )
            }
        }?;
        let mut levels = vec![buckets];
        // Dyadic ranges let each column keep short transients even at overview zoom.
        while levels.last().unwrap().len() > 1 {
            let next = levels
                .last()
                .unwrap()
                .chunks(2)
                .map(|pair| pair[0].merge(pair.get(1).copied().unwrap_or(Peak::EMPTY)))
                .collect();
            levels.push(next);
        }
        let peak = levels
            .last()
            .unwrap()
            .first()
            .copied()
            .unwrap_or(Peak::EMPTY);
        Ok(Self {
            frames,
            rate: f64::from(spec.sample_rate),
            frames_per_bucket,
            levels,
            tail: Peak::EMPTY,
            peak,
            pitch: analyzer.and_then(Analyzer::finish),
        })
    }

    pub(super) fn pitch(&self) -> Option<&Track> {
        self.pitch.as_ref()
    }

    pub(super) fn amplitude(&self) -> f32 {
        if self.frames == 0 {
            return 0.0;
        }
        let peak = self.peak.merge(self.tail);
        peak.min.abs().max(peak.max.abs())
    }

    pub(super) fn extrema(&self, start: f64, end: f64) -> Option<(f32, f32)> {
        let start_frame = (start * self.rate).floor() as usize;
        let end_frame = ((end * self.rate).ceil() as usize).min(self.frames);
        if start_frame >= end_frame {
            return None;
        }
        let mut index = start_frame / self.frames_per_bucket;
        let outside = end_frame.div_ceil(self.frames_per_bucket);
        let end = outside.min(self.levels[0].len());
        let mut peak = Peak::EMPTY;
        while index < end {
            let level = index.trailing_zeros().min((end - index).ilog2());
            peak = peak.merge(self.levels[level as usize][index >> level]);
            index += 1usize << level;
        }
        if outside > end {
            peak = peak.merge(self.tail);
        }
        Some((peak.min, peak.max))
    }

    // Only completed pairs enter the pyramid. The changing partial bucket is separate.
    fn push(&mut self, mut peak: Peak) {
        self.peak = self.peak.merge(peak);
        for level in &mut self.levels {
            level.push(peak);
            if level.len() % 2 != 0 {
                return;
            }
            peak = peak.merge(level[level.len() - 2]);
        }
        self.levels.push(vec![peak]);
    }
}

pub(super) struct LiveAnalysis<'a> {
    spec: hound::WavSpec,
    tail: Peak,
    filled: usize,
    analyzer: Option<Analyzer<'a>>,
}

impl<'a> LiveAnalysis<'a> {
    // The recording's final size is unknown. Keep peak storage bounded by the WAV size limit.
    const BUCKET_FRAMES: usize = 128;

    pub(super) fn new(spec: hound::WavSpec, detector: Option<&'a Detector>) -> Self {
        Self {
            spec,
            tail: Peak::EMPTY,
            filled: 0,
            analyzer: detector.and_then(|detector| Analyzer::new(detector, spec)),
        }
    }

    pub(super) fn append(&mut self, output: &Mutex<Option<Waveform>>, samples: &[f32]) {
        let mut peaks = Vec::new();
        let channels = usize::from(self.spec.channels);
        for &sample in samples {
            let value = finite(sample);
            self.tail.min = self.tail.min.min(value);
            self.tail.max = self.tail.max.max(value);
            self.filled += 1;
            if self.filled == Self::BUCKET_FRAMES * channels {
                peaks.push(std::mem::replace(&mut self.tail, Peak::EMPTY));
                self.filled = 0;
            }
            if let Some(analyzer) = &mut self.analyzer {
                analyzer.push(value);
            }
        }
        let mut output = output.lock().unwrap();
        let waveform = output.get_or_insert_with(|| Waveform {
            frames: 0,
            rate: f64::from(self.spec.sample_rate),
            frames_per_bucket: Self::BUCKET_FRAMES,
            levels: vec![Vec::new()],
            tail: Peak::EMPTY,
            peak: Peak::EMPTY,
            pitch: self
                .analyzer
                .as_ref()
                .map(|_| Track::new(self.spec.sample_rate)),
        });
        for peak in peaks {
            waveform.push(peak);
        }
        waveform.frames += samples.len() / channels;
        waveform.tail = self.tail;
        if let (Some(track), Some(analyzer)) = (&mut waveform.pitch, &mut self.analyzer) {
            track.extend(analyzer.drain());
        }
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn collect<T>(
    samples: impl Iterator<Item = hound::Result<T>>,
    bucket_size: usize,
    bucket_count: usize,
    convert: impl Fn(T) -> f32,
    mut analyzer: Option<&mut Analyzer<'_>>,
) -> hound::Result<Vec<Peak>> {
    let mut buckets = Vec::with_capacity(bucket_count);
    let mut bucket = Peak::EMPTY;
    let mut remaining = bucket_size;
    for sample in samples {
        let value = finite(convert(sample?));
        if let Some(analyzer) = analyzer.as_deref_mut() {
            analyzer.push(value);
        }
        bucket.min = bucket.min.min(value);
        bucket.max = bucket.max.max(value);
        remaining -= 1;
        if remaining == 0 {
            buckets.push(bucket);
            bucket = Peak::EMPTY;
            remaining = bucket_size;
        }
    }
    if remaining != bucket_size {
        buckets.push(bucket);
    }
    Ok(buckets)
}
