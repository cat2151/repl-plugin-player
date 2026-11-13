//! `monitor_placement` of a `[[window]]`: the outer window, frame and title bar
//! included, as percentages of one monitor's work area, in the form of
//! `../cat-smf-editor`'s `layout.toml`. Rectangles are physical pixels.
use eframe::egui;
use serde::Deserialize;
use std::collections::HashMap;

mod win32;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub(super) enum Monitor {
    Position(Position),
    /// The Windows DISPLAY number.
    Number(u32),
}

/// By the monitors' horizontal centers.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Position {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MonitorPlacement {
    pub monitor: Monitor,
    pub left_percent: f32,
    pub top_percent: f32,
    pub width_percent: f32,
    pub height_percent: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Screen {
    pub number: u32,
    pub bounds: [i32; 4],
    pub work: [i32; 4],
}

/// The key of the placement in a `[[window]]` table.
pub(super) const KEY: &str = "monitor_placement";

impl MonitorPlacement {
    pub fn read(value: &toml::Value) -> Result<Self, String> {
        let placement: Self = (value.clone().try_into()).map_err(|_| refused())?;
        let percents = [
            placement.left_percent,
            placement.top_percent,
            placement.width_percent,
            placement.height_percent,
        ];
        if !percents.iter().all(|n| n.is_finite() && *n >= 0.0)
            || placement.width_percent <= 0.0
            || placement.height_percent <= 0.0
            || placement.left_percent + placement.width_percent > 100.0
            || placement.top_percent + placement.height_percent > 100.0
            || placement.monitor == Monitor::Number(0)
        {
            return Err(refused());
        }
        Ok(placement)
    }

    /// The table's text, header included.
    pub fn write(&self) -> String {
        let monitor = match self.monitor {
            Monitor::Number(number) => number.to_string(),
            Monitor::Position(Position::Left) => "\"left\"".into(),
            Monitor::Position(Position::Center) => "\"center\"".into(),
            Monitor::Position(Position::Right) => "\"right\"".into(),
        };
        format!(
            "[window.{KEY}]
monitor = {monitor}
left_percent = {}
top_percent = {}
width_percent = {}
height_percent = {}
",
            self.left_percent, self.top_percent, self.width_percent, self.height_percent
        )
    }

    /// The outer rectangle on `screens`.
    pub fn resolve(&self, screens: &[Screen]) -> Result<[i32; 4], String> {
        let screen = match self.monitor {
            Monitor::Number(number) => (screens.iter())
                .find(|s| s.number == number)
                .ok_or_else(|| missing(number.to_string()))?,
            Monitor::Position(position) => {
                let center = |s: &Screen| i64::from(s.bounds[0]) * 2 + i64::from(s.bounds[2]);
                let mut ordered: Vec<_> = screens.iter().collect();
                ordered.sort_by_key(|s| center(s));
                let index = match position {
                    _ if ordered.is_empty() => {
                        return Err(missing(format!("{position:?}").to_lowercase()))
                    }
                    Position::Left => 0,
                    Position::Right => ordered.len() - 1,
                    Position::Center if ordered.len() % 2 == 1 => ordered.len() / 2,
                    Position::Center => {
                        return Err(crate::i18n::t("layout_monitor_ambiguous").into())
                    }
                };
                let selected = ordered[index];
                if ordered
                    .iter()
                    .filter(|s| center(s) == center(selected))
                    .count()
                    != 1
                {
                    return Err(crate::i18n::t("layout_monitor_ambiguous").into());
                }
                selected
            }
        };
        let [x, y, w, h] = screen.work;
        let edge = |origin: i32, size: i32, percent: f32| {
            origin + (size as f64 * f64::from(percent) / 100.0).round() as i32
        };
        let left = edge(x, w, self.left_percent);
        let top = edge(y, h, self.top_percent);
        let right = edge(x, w, self.left_percent + self.width_percent);
        let bottom = edge(y, h, self.top_percent + self.height_percent);
        if right <= left || bottom <= top {
            return Err(refused());
        }
        Ok([left, top, right - left, bottom - top])
    }
}

fn refused() -> String {
    crate::i18n::t("layout_monitor").into()
}

fn missing(monitor: String) -> String {
    crate::i18n::render("layout_monitor_missing", &[("monitor", monitor)])
}

/// Each window's rectangle on the monitors connected now; they are looked up only
/// when a window asks for one.
pub(super) fn resolve_all(
    placements: &HashMap<egui::ViewportId, MonitorPlacement>,
) -> Result<HashMap<egui::ViewportId, [i32; 4]>, String> {
    if placements.is_empty() {
        return Ok(HashMap::new());
    }
    let screens = win32::screens()?;
    (placements.iter())
        .map(|(id, placement)| Ok((*id, placement.resolve(&screens)?)))
        .collect()
}

/// Moves the window of this viewport, titled `title`, to `outer` over its first few
/// frames; after those, the user's moves are left alone until `outer` or the window
/// changes.
pub(super) fn apply(ctx: &egui::Context, title: &str, outer: [i32; 4]) {
    win32::apply(ctx, title, outer);
}

#[cfg(test)]
#[path = "monitor_tests.rs"]
mod tests;
