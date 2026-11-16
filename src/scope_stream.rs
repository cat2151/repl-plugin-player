//! Consume every source sample and publish consecutive complete scope windows.
use crate::scope::Display;
use crate::scope_boundary::Boundary;
use crate::scope_capture::Sample;
use crate::scope_drift::Drift;
use crate::scope_trigger;

pub struct Stream {
    rate: u32,
    key: Option<(u64, u8)>,
    serial: Option<u64>,
    samples: Vec<[f32; 2]>,
    boundary: Boundary,
    drift: Drift,
}

impl Stream {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            key: None,
            serial: None,
            samples: Vec::new(),
            boundary: Boundary::default(),
            drift: Drift::default(),
        }
    }

    pub fn sample(&mut self, sample: Sample) -> Option<Display> {
        if self
            .serial
            .is_some_and(|s| sample.serial != s.wrapping_add(1))
            || sample.note == 128
        {
            self.samples.clear();
            self.boundary.reset();
            self.drift.reset();
            self.key = None;
        }
        self.serial = Some(sample.serial);
        if sample.note == 128 {
            return None;
        }
        let key = (sample.epoch, sample.note);
        if self.key != Some(key) {
            self.samples.clear();
            self.key = Some(key);
        }
        // Master mixes have no single reference pitch: show 20 ms without
        // running the pitched similarity/drift analysis.
        let master = sample.note == 129;
        let width = if master {
            (self.rate as usize / 50).max(2)
        } else {
            scope_trigger::width(self.rate, sample.note)
        };
        self.samples.push(sample.stereo);
        if self.samples.len() < width {
            return None;
        }
        if master {
            return Some(Display {
                samples: std::mem::take(&mut self.samples),
                note: sample.note,
                sample_rate: self.rate,
                shift_samples: 0.0,
            });
        }
        let record = self.boundary.observe(
            sample.epoch,
            sample.note,
            sample.serial + 1,
            &self.samples[..width],
        );
        let shift = self.drift.observe(&record, self.rate);
        let result = Display {
            samples: self.samples[..width].to_vec(),
            note: sample.note,
            sample_rate: self.rate,
            shift_samples: shift * width as f64 / 4.0,
        };
        self.samples.clear();
        Some(result)
    }
}

#[cfg(test)]
#[path = "scope_stream_tests.rs"]
mod tests;
