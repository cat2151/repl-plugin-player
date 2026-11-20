//! Per-slot loudness trim and peak limiter.
//!
//! A slot is measured for one chord from its first note-on, with the
//! K-weighting of ITU-R BS.1770 and no gating, and the trim that brings that
//! loudness to `TARGET_LUFS` is then fixed. A slot measured at gain 0 gets the
//! trim at once; one heard while measured sounds untrimmed and glides to the
//! trim over `GLIDE_SECONDS`. After the trim, a channel-linked limiter with instant attack keeps every
//! sample within `CEILING`; the crossfade gains sum to 1, so both slots
//! together stay within it too.

pub const MAX_CHANNELS: usize = 16;
/// Near the median of the pads `try-evolving-sequencer` picks, so trimming
/// does not lower the overall level.
pub const TARGET_LUFS: f64 = -18.0;
pub const MAX_TRIM_DB: f64 = 18.0;
/// Below this the slot is treated as silent and left untrimmed.
pub const SILENT_LUFS: f64 = -70.0;
/// -1 dBFS.
pub const CEILING: f64 = 0.891_250_938_133_745_5;
pub const RELEASE_SECONDS: f64 = 0.2;
pub const GLIDE_SECONDS: f64 = 1.0;

pub trait Sample: Copy {
    fn get(self) -> f64;
    fn put(value: f64) -> Self;
}

impl Sample for f32 {
    fn get(self) -> f64 {
        f64::from(self)
    }
    fn put(value: f64) -> Self {
        value as f32
    }
}

impl Sample for f64 {
    fn get(self) -> f64 {
        self
    }
    fn put(value: f64) -> Self {
        value
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    fn normalized(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
        }
    }

    /// The head-related high shelf of the K-weighting, as libebur128
    /// derives it for any rate (48 kHz gives the BS.1770 table).
    fn shelf(rate: f64) -> Self {
        let (f0, gain_db, q) = (
            1_681.974_450_955_533,
            3.999_843_853_973_347,
            0.707_175_236_955_419_6,
        );
        let k = (std::f64::consts::PI * f0 / rate).tan();
        let vh = 10f64.powf(gain_db / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        Self::normalized(
            [
                vh + vb * k / q + k * k,
                2.0 * (k * k - vh),
                vh - vb * k / q + k * k,
            ],
            [
                1.0 + k / q + k * k,
                2.0 * (k * k - 1.0),
                1.0 - k / q + k * k,
            ],
        )
    }

    /// The low-frequency high pass of the K-weighting; its numerator stays
    /// unnormalized as in BS.1770.
    fn high_pass(rate: f64) -> Self {
        let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
        let k = (std::f64::consts::PI * f0 / rate).tan();
        let a0 = 1.0 + k / q + k * k;
        Self {
            b0: 1.0,
            b1: -2.0,
            b2: 1.0,
            a1: 2.0 * (k * k - 1.0) / a0,
            a2: (1.0 - k / q + k * k) / a0,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct History {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl History {
    fn step(&mut self, f: &Biquad, x: f64) -> f64 {
        let y = f.b0 * x + f.b1 * self.x1 + f.b2 * self.x2 - f.a1 * self.y1 - f.a2 * self.y2;
        (self.x2, self.x1, self.y2, self.y1) = (self.x1, x, self.y1, y);
        y
    }
}

pub struct Level {
    rate: f64,
    shelf: Biquad,
    high_pass: Biquad,
    release: f64,
    histories: [[History; 2]; MAX_CHANNELS],
    sounded: bool,
    energy: f64,
    measured: u64,
    peak: f64,
    trim: Option<f64>,
    /// The trim in use: 1 while measuring, then gliding to `trim`.
    applied: f64,
    /// Per-frame factor and frames left of the glide.
    glide: (f64, u64),
    loudness: f64,
    envelope: f64,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            rate: 0.0,
            shelf: Biquad::default(),
            high_pass: Biquad::default(),
            release: 0.0,
            histories: Default::default(),
            sounded: false,
            energy: 0.0,
            measured: 0,
            peak: 0.0,
            trim: None,
            applied: 1.0,
            glide: (1.0, 0),
            loudness: f64::NEG_INFINITY,
            envelope: 1.0,
        }
    }
}

impl Level {
    /// Forgets the previous patch; the filters keep their sample rate.
    pub fn reset(&mut self) {
        *self = Self {
            rate: self.rate,
            shelf: self.shelf,
            high_pass: self.high_pass,
            release: self.release,
            ..Self::default()
        };
    }

    pub fn prepare(&mut self, rate: f64) {
        if rate != self.rate {
            self.rate = rate;
            self.shelf = Biquad::shelf(rate);
            self.high_pass = Biquad::high_pass(rate);
            self.release = (-1.0 / (RELEASE_SECONDS * rate)).exp();
        }
    }

    /// Measuring starts at the slot's first note-on.
    pub fn note_on(&mut self) {
        self.sounded = true;
    }

    /// Fixes the trim once `frames` frames were measured since the first
    /// note-on; `heard` makes the trim glide in. True only on the call that
    /// fixes it.
    pub fn settle(&mut self, frames: u64, heard: bool) -> bool {
        if self.trim.is_some() || !self.sounded || self.measured < frames.max(1) {
            return false;
        }
        let mean = self.energy / self.measured as f64;
        self.loudness = if mean > 0.0 {
            -0.691 + 10.0 * mean.log10()
        } else {
            f64::NEG_INFINITY
        };
        let db = if self.loudness < SILENT_LUFS {
            0.0
        } else {
            (TARGET_LUFS - self.loudness).clamp(-MAX_TRIM_DB, MAX_TRIM_DB)
        };
        let trim = 10f64.powf(db / 20.0);
        self.trim = Some(trim);
        let glide = (GLIDE_SECONDS * self.rate).round() as u64;
        if heard && glide > 0 {
            self.glide = ((trim / self.applied).powf(1.0 / glide as f64), glide);
        } else {
            self.applied = trim;
        }
        true
    }

    pub fn trimmed(&self) -> bool {
        self.trim.is_some()
    }

    /// The fixed linear trim; NaN until fixed.
    pub fn trim(&self) -> f64 {
        self.trim.unwrap_or(f64::NAN)
    }

    /// Measured loudness in LUFS; -inf when silent.
    pub fn loudness(&self) -> f64 {
        self.loudness
    }

    /// Largest untrimmed sample while measuring, in dBFS.
    pub fn peak_db(&self) -> f64 {
        20.0 * self.peak.log10()
    }

    /// Measures (until the trim is fixed), trims, limits and applies `gains`.
    /// Every channel has `frames` samples; `gains` has at least `frames`.
    pub fn process<T: Sample>(&mut self, channels: &mut [&mut [T]], frames: usize, gains: &[f32]) {
        let measuring = self.sounded && self.trim.is_none();
        for (frame, gain) in gains.iter().enumerate().take(frames) {
            if self.glide.1 > 0 {
                self.glide.1 -= 1;
                self.applied = match (self.glide.1, self.trim) {
                    (0, Some(trim)) => trim,
                    _ => self.applied * self.glide.0,
                };
            }
            let trim = self.applied;
            let mut loudest = 0.0f64;
            for (channel, samples) in channels.iter().enumerate() {
                let x = samples[frame].get();
                if measuring {
                    let h = &mut self.histories[channel];
                    let shelved = h[0].step(&self.shelf, x);
                    let y = h[1].step(&self.high_pass, shelved);
                    self.energy += y * y;
                    self.peak = self.peak.max(x.abs());
                }
                loudest = loudest.max((x * trim).abs());
            }
            let released = 1.0 - (1.0 - self.envelope) * self.release;
            let limit = if loudest > CEILING {
                CEILING / loudest
            } else {
                1.0
            };
            self.envelope = released.min(limit);
            let scale = trim * self.envelope * f64::from(*gain);
            for samples in channels.iter_mut() {
                samples[frame] = T::put(samples[frame].get() * scale);
            }
        }
        if measuring {
            self.measured += frames as u64;
        }
    }
}

#[cfg(test)]
mod tests;
