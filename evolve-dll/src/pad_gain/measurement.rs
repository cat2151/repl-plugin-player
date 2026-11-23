//! Set a gain ceiling from the unamplified output since the latest onset.

const ATTACK_SECONDS: f64 = 0.04;
const TAIL_WINDOW_SECONDS: f64 = 0.1;
const TAIL_LIMIT_SECONDS: f64 = 2.0;
const MIN_SIGNAL: f64 = 0.0001; // -80 dBFS
const MAX_BACKGROUND: f64 = 0.001; // -60 dBFS
const SIGNAL_TO_BACKGROUND: f64 = 10.0; // 20 dB
const TARGET_PEAK: f64 = 0.251188643150958; // -12 dBFS
const MAX_GAIN: f64 = 7.943282347242816; // +18 dB

#[derive(Default)]
struct Onset {
    peak: f64,
    frames: usize,
    keys: u32,
    /// Output just before this onset, when a larger chord preceded it.
    tail: f64,
    window_peak: f64,
    window_frames: usize,
}

pub(super) struct Measurement {
    attack_frames: usize,
    tail_window: usize,
    tail_limit: usize,
    last_peak: f64,
    held: [u128; 16 * 16], // one bit per key, indexed by UMP group and channel
    held_count: u32,
    signal_floor: f64,
    onset: Option<Onset>,
}

impl Measurement {
    pub(super) fn new(rate: f64) -> Self {
        Self {
            attack_frames: (rate * ATTACK_SECONDS).ceil() as usize,
            tail_window: (rate * TAIL_WINDOW_SECONDS).ceil() as usize,
            tail_limit: (rate * TAIL_LIMIT_SECONDS).ceil() as usize,
            last_peak: 0.0,
            held: [0; 16 * 16],
            held_count: 0,
            signal_floor: MIN_SIGNAL,
            onset: None,
        }
    }

    pub(super) fn events(&mut self, mut words: &[u32]) {
        const LENGTHS: [usize; 16] = [1, 1, 1, 2, 2, 4, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4];
        let previous_keys = self.onset.as_ref().map_or(0, |onset| onset.keys);
        while let Some(&word) = words.first() {
            let kind = (word >> 28) as usize;
            let Some((packet, rest)) = words.split_at_checked(LENGTHS[kind]) else {
                self.onset = None;
                return;
            };
            words = rest;
            if kind != 2 && kind != 4 {
                continue;
            }
            let group = (word >> 24) & 15;
            let channel = (word >> 16) & 15;
            let slot = (16 * group + channel) as usize;
            let status = (word >> 20) & 15;
            let key = (word >> 8) & 127;
            let mask = 1u128 << key;
            let already_held = self.held[slot] & mask != 0;
            let velocity = if kind == 4 {
                packet[1] >> 16
            } else {
                word & 127
            };
            match status {
                9 if velocity != 0 => {
                    // All note-ons in this block share the same audio peak.
                    self.held[slot] |= mask;
                    self.held_count += u32::from(!already_held);
                    let keys = self.held_count;
                    let tail = if previous_keys > keys {
                        self.last_peak
                    } else {
                        0.0
                    };
                    self.onset = Some(Onset {
                        keys,
                        tail,
                        ..Onset::default()
                    });
                }
                8 | 9 => {
                    self.held[slot] &= !mask;
                    self.held_count -= u32::from(already_held);
                }
                11 if key == 120 || key == 123 => {
                    self.held_count -= self.held[slot].count_ones();
                    self.held[slot] = 0;
                    if key == 120 {
                        self.onset = None;
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn observe(&mut self, peak: f64, frames: usize, applied_gain: f64) -> f64 {
        self.last_peak = peak;
        // A low idle signal can be plugin noise. A louder tail must not make
        // the next attack wait for silence.
        if self.held_count == 0 {
            self.signal_floor = if (MIN_SIGNAL..MAX_BACKGROUND).contains(&peak) {
                peak * SIGNAL_TO_BACKGROUND
            } else {
                MIN_SIGNAL
            };
        }
        let gain_for = |peak: f64| (TARGET_PEAK / peak).clamp(1.0, MAX_GAIN);
        let Some(onset) = self.onset.as_mut() else {
            return applied_gain.min(gain_for(peak));
        };
        onset.peak = onset.peak.max(peak);
        onset.frames = onset.frames.saturating_add(frames);
        // Under a larger chord's tail the peak is not this note's own. Follow it
        // down for a limited time; a plain repeated note never qualifies.
        if self.held_count > 0 && onset.tail > onset.peak * 0.5 && onset.frames <= self.tail_limit {
            onset.window_peak = onset.window_peak.max(peak);
            onset.window_frames += frames;
            if onset.window_frames >= self.tail_window {
                if onset.window_peak >= self.signal_floor {
                    onset.peak = onset.window_peak;
                }
                onset.window_peak = 0.0;
                onset.window_frames = 0;
            }
        }
        let ceiling = gain_for(onset.peak);
        // Only a fresh attack can loosen the ceiling. A fading held note keeps
        // its peak, and even an unfinished glide stops growing at note-off.
        if self.held_count > 0 && onset.frames >= self.attack_frames && peak >= self.signal_floor {
            ceiling
        } else {
            applied_gain.min(ceiling)
        }
    }
}
