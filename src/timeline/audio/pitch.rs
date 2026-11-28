use eframe::egui::{self, Color32, Rect, Stroke};
use libloading::Library;

const FRAME: usize = 4096;
const HOP: usize = 512;
const SILENCE_POWER: f32 = 1e-5; // -50 dB RMS, as in UAPMD's pitch transcription.
const PITCH_COLOR: Color32 = Color32::from_rgb(255, 218, 122);

pub(super) struct Detector {
    _library: Library,
    estimate: unsafe extern "C" fn(*const f32, i32, i32) -> f32,
}

impl Detector {
    pub(super) fn load() -> Option<Self> {
        let path = crate::resources::uapmd_host().ok()?;
        // Keep the host loaded as long as this worker can call the function.
        unsafe {
            let library = Library::new(path).ok()?;
            let estimate = *library.get(b"repl_mpm_pitch\0").ok()?;
            Some(Self {
                _library: library,
                estimate,
            })
        }
    }

    fn midi_pitch(&self, samples: &[f32; FRAME], rate: i32) -> f32 {
        let power = samples.iter().map(|sample| sample * sample).sum::<f32>() / FRAME as f32;
        if power < SILENCE_POWER {
            return f32::NAN;
        }
        // The borrowed array contains exactly FRAME samples for this synchronous call.
        let hz = unsafe { (self.estimate)(samples.as_ptr(), FRAME as i32, rate) };
        if (55.0..=2093.0).contains(&hz) {
            69.0 + 12.0 * (hz / 440.0).log2()
        } else {
            f32::NAN
        }
    }
}

pub(super) struct Analyzer<'a> {
    detector: &'a Detector,
    rate: i32,
    channels: u16,
    channel: u16,
    sum: f32,
    window: [f32; FRAME],
    filled: usize,
    pitches: Vec<f32>,
}

impl<'a> Analyzer<'a> {
    pub(super) fn new(detector: &'a Detector, spec: hound::WavSpec) -> Option<Self> {
        let rate = i32::try_from(spec.sample_rate).ok()?;
        (rate > 0 && spec.channels > 0).then_some(Self {
            detector,
            rate,
            channels: spec.channels,
            channel: 0,
            sum: 0.0,
            window: [0.0; FRAME],
            filled: 0,
            pitches: Vec::new(),
        })
    }

    pub(super) fn push(&mut self, sample: f32) {
        self.sum += sample.clamp(-1.0, 1.0);
        self.channel += 1;
        if self.channel < self.channels {
            return;
        }
        self.window[self.filled] = self.sum / f32::from(self.channels);
        self.channel = 0;
        self.sum = 0.0;
        self.filled += 1;
        if self.filled == FRAME {
            self.pitches
                .push(self.detector.midi_pitch(&self.window, self.rate));
            self.window.copy_within(HOP.., 0);
            self.filled -= HOP;
        }
    }

    pub(super) fn drain(&mut self) -> impl Iterator<Item = f32> + '_ {
        self.pitches.drain(..)
    }

    pub(super) fn finish(self) -> Option<Track> {
        let mut track = Track {
            pitches: self.pitches,
            ..Track::new(self.rate as u32)
        };
        track.update_range(0);
        (track.low <= track.high).then_some(track)
    }
}

pub(super) struct Track {
    pitches: Vec<f32>,
    rate: f64,
    low: f32,
    high: f32,
}

impl Track {
    pub(super) fn new(rate: u32) -> Self {
        Self {
            pitches: Vec::new(),
            rate: f64::from(rate),
            low: f32::INFINITY,
            high: f32::NEG_INFINITY,
        }
    }

    pub(super) fn extend(&mut self, pitches: impl IntoIterator<Item = f32>) {
        let start = self.pitches.len();
        self.pitches.extend(pitches);
        self.update_range(start);
    }

    fn update_range(&mut self, start: usize) {
        for &pitch in &self.pitches[start..] {
            if pitch.is_finite() {
                self.low = self.low.min(pitch);
                self.high = self.high.max(pitch);
            }
        }
    }

    fn at(&self, seconds: f64) -> Option<(usize, f32)> {
        // Timestamp each pitch at the center of the PCM window it describes.
        let frame = (seconds * self.rate - (FRAME / 2) as f64) / HOP as f64;
        let nearest = frame.round();
        if !(0.0..self.pitches.len() as f64).contains(&nearest) {
            return None;
        }
        let index = nearest as usize;
        let pitch = self.pitches[index];
        if !pitch.is_finite() {
            return None;
        }
        let other = if frame < nearest {
            index.checked_sub(1)
        } else {
            Some(index + 1)
        };
        let interpolated = other
            .and_then(|other| self.pitches.get(other))
            .filter(|other| other.is_finite())
            .map_or(pitch, |other| {
                pitch + (other - pitch) * (frame - nearest).abs() as f32
            });
        Some((index, interpolated))
    }

    pub(super) fn draw(
        &self,
        painter: &egui::Painter,
        inner: Rect,
        visible: Rect,
        step: f32,
        source: impl Fn(f32) -> f64,
        muted: bool,
    ) {
        if self.low > self.high {
            return;
        }
        let center = (self.low + self.high) * 0.5;
        let span = (self.high - self.low + 2.0).max(24.0);
        let painter = painter.with_clip_rect(visible);
        let color = PITCH_COLOR.gamma_multiply(if muted { 0.45 } else { 1.0 });
        let draw_line = |line: &mut Vec<egui::Pos2>| {
            if line.len() > 1 {
                painter.add(egui::Shape::line(
                    std::mem::take(line),
                    Stroke::new(1.5, color),
                ));
            } else if let Some(point) = line.pop() {
                painter.circle_filled(point, 0.75, color);
            }
        };
        let columns = (visible.width() / step).ceil() as usize;
        let mut line = Vec::new();
        let mut last = None;
        for column in 0..=columns {
            let x = (visible.left() + column as f32 * step).min(visible.right());
            let Some((index, pitch)) = self.at(source(x)) else {
                draw_line(&mut line);
                last = None;
                continue;
            };
            if last.is_some_and(|previous| {
                self.pitches
                    .get(previous..=index)
                    .is_none_or(|frames| frames.iter().any(|pitch| !pitch.is_finite()))
            }) {
                draw_line(&mut line);
            }
            let y = inner.center().y - (pitch - center) * inner.height() / span;
            line.push(egui::pos2(x, y));
            last = Some(index);
        }
        draw_line(&mut line);
    }
}
