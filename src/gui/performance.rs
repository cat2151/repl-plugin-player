//! UI-thread aggregation of engine timings; no work is added to the audio callback.
use eframe::egui;
use serde_json::Value;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_millis(500);
mod meters;

#[derive(Default)]
struct Batch {
    callbacks: usize,
    callback_ms: f64,
    budget_ms: f64,
    peak_percent: f64,
    note_peak: Option<u64>,
}

#[derive(Default)]
pub(super) struct Performance {
    batches: VecDeque<(Instant, Batch)>,
    counters: Option<(u32, u32)>,
    missed: u64,
    dropped: u64,
    notes: Option<(Instant, u64)>,
    execution_width: usize,
}

impl Performance {
    pub fn record(&mut self, observation: &Value, now: Instant) {
        self.notes = observation["noteCount"]
            .as_f64()
            .map(|count| (now, count as u64));
        // Poll batches approximate the trailing window to the 100 ms UI poll interval.
        // Gaps cannot be mistaken for recent measurements after the UI was blocked.
        let gap = self
            .batches
            .back()
            .is_some_and(|(time, _)| now.duration_since(*time) >= WINDOW);
        if gap {
            self.batches.clear();
        }
        let mut batch = Batch {
            note_peak: self.notes.map(|(_, count)| count),
            ..Batch::default()
        };
        if let Some(timings) = observation["processingTimings"].as_array().filter(|_| !gap) {
            for timing in timings {
                let (Some(stage), Some(ms), Some(frames), Some(rate)) = (
                    timing["stage"].as_f64(),
                    timing["durationMs"].as_f64(),
                    timing["frames"].as_f64(),
                    timing["sampleRate"].as_f64(),
                ) else {
                    continue;
                };
                if !ms.is_finite() || ms < 0.0 || frames <= 0.0 || rate <= 0.0 {
                    continue;
                }
                if stage == 5.0 {
                    let budget = frames * 1000.0 / rate;
                    batch.callbacks += 1;
                    batch.callback_ms += ms;
                    batch.budget_ms += budget;
                    batch.peak_percent = batch.peak_percent.max(ms / budget * 100.0);
                }
            }
        }
        if let (Some(missed), Some(dropped)) = (
            observation["deadlineMisses"].as_f64(),
            observation["droppedTimings"].as_f64(),
        ) {
            let current = (missed as u32, dropped as u32);
            if let Some(previous) = self.counters.replace(current) {
                self.missed += current.0.wrapping_sub(previous.0) as u64;
                self.dropped += current.1.wrapping_sub(previous.1) as u64;
            }
        }
        self.batches.push_back((now, batch));
        self.expire(now);
    }

    fn expire(&mut self, now: Instant) {
        while self
            .batches
            .front()
            .is_some_and(|(time, _)| now.duration_since(*time) >= WINDOW)
        {
            self.batches.pop_front();
        }
    }

    fn totals(&mut self, now: Instant) -> Batch {
        self.expire(now);
        let mut total = Batch::default();
        for (_, batch) in &self.batches {
            total.callbacks += batch.callbacks;
            total.callback_ms += batch.callback_ms;
            total.budget_ms += batch.budget_ms;
            total.peak_percent = total.peak_percent.max(batch.peak_percent);
            total.note_peak = total.note_peak.max(batch.note_peak);
        }
        total
    }

    pub fn show(&mut self, ui: &mut egui::Ui, execution: Option<u64>) {
        let execution = execution.map_or_else(|| "—".into(), |n| n.to_string());
        self.execution_width = self.execution_width.max(6).max(execution.len());
        let execution = format!("{execution:>width$}", width = self.execution_width);
        ui.add(
            egui::Label::new(
                egui::RichText::new(crate::i18n::render("f001", &[("arg0", execution)]))
                    .monospace(),
            )
            .extend(),
        );
        let now = Instant::now();
        let notes = self
            .notes
            .filter(|(time, _)| now.duration_since(*time) < WINDOW);
        let total = self.totals(now);
        ui.label(crate::i18n::t("performance_notes"))
            .on_hover_text(crate::i18n::t("performance_notes_help"));
        meters::notes(ui, [notes.map(|(_, count)| count), total.note_peak]);
        ui.add_space(8.0);
        ui.label(crate::i18n::t("performance_load"))
            .on_hover_text(crate::i18n::t("performance_load_help"));
        let load = (total.callbacks > 0).then(|| {
            [
                total.callback_ms / total.budget_ms * 100.0,
                total.peak_percent,
            ]
        });
        meters::load(ui, load);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn note_peak_holds_then_expires_with_the_load_window() {
        let mut meter = Performance::default();
        let now = Instant::now();
        meter.record(&json!({"noteCount":12.0}), now);
        meter.record(&json!({"noteCount":3.0}), now + Duration::from_millis(100));
        assert_eq!(meter.notes.unwrap().1, 3);
        assert_eq!(
            meter.totals(now + Duration::from_millis(100)).note_peak,
            Some(12)
        );
        assert_eq!(
            meter.totals(now + Duration::from_millis(500)).note_peak,
            Some(3)
        );
        assert_eq!(
            meter.totals(now + Duration::from_millis(600)).note_peak,
            None
        );
    }

    #[test]
    fn weights_callback_budgets_and_ignores_other_stages() {
        let mut meter = Performance::default();
        let now = Instant::now();
        let timings: Vec<_> = [
            (0, 1.0),
            (1, 2.0),
            (1, 3.0),
            (2, 4.0),
            (3, 1.0),
            (4, 1.0),
            (5, 6.0),
        ]
        .into_iter()
        .map(|(stage, ms)| json!({"stage":f64::from(stage),"durationMs":ms,"frames":480,"sampleRate":48000}))
        .collect();
        meter.record(&json!({"processingTimings":timings}), now);
        let total = meter.totals(now);
        assert_eq!(total.callback_ms / total.budget_ms * 100.0, 60.0);
        meter.record(&json!({"processingTimings":[{"stage":5,"durationMs":24,"frames":960,"sampleRate":48000}]}), now + Duration::from_millis(100));
        let total = meter.totals(now + Duration::from_millis(100));
        assert_eq!(total.callback_ms / total.budget_ms * 100.0, 100.0);
        assert_eq!(total.peak_percent, 120.0);
        assert_eq!(meter.totals(now + Duration::from_millis(600)).callbacks, 0);
    }

    #[test]
    fn counters_preserve_short_misses_and_wrap() {
        let mut meter = Performance::default();
        let now = Instant::now();
        meter.record(&json!({"deadlineMisses":u32::MAX,"droppedTimings":10}), now);
        meter.record(&json!({"deadlineMisses":1,"droppedTimings":13}), now);
        assert_eq!((meter.missed, meter.dropped), (2, 3));
        meter.totals(now + Duration::from_secs(1));
        assert_eq!(meter.missed, 2);
    }

    #[test]
    fn accepts_native_float_zeros_and_discards_timings_after_a_poll_gap() {
        let mut meter = Performance::default();
        let now = Instant::now();
        let value = json!({"noteCount":0.0,"deadlineMisses":0.0,"droppedTimings":0.0,
            "processingTimings":[{"stage":5.0,"durationMs":1,"frames":480,"sampleRate":48000}]});
        meter.record(&value, now);
        assert_eq!(meter.notes.unwrap().1, 0);
        assert_eq!(meter.counters, Some((0, 0)));
        assert_eq!(meter.totals(now).callbacks, 1);
        meter.record(&value, now + Duration::from_secs(1));
        assert_eq!(meter.totals(now + Duration::from_secs(1)).callbacks, 0);
    }
}
