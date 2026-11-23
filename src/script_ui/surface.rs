//! Script canvases. Rust draws regions and captures the pointer;
//! scripts decide what the input means.
use eframe::egui::{self, Color32, Event, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeMap};

mod gain_meter;

#[derive(Deserialize)]
struct Definition {
    id: String,
    width: f32,
    height: f32,
    regions: Vec<Region>,
    header: Option<Header>,
}

#[derive(Deserialize)]
struct Header {
    checkbox: String,
    checked: bool,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "padGainMeter")]
    pad_gain_meter: bool,
    #[serde(skip)]
    pad_gain_instance: Option<i32>,
}

#[derive(Deserialize)]
struct Region {
    rect: [f32; 4],
    color: [u8; 3],
    #[serde(default)]
    cursor: bool,
}

#[derive(Serialize)]
struct Change {
    kind: &'static str,
    region: usize,
    x: f32,
    y: f32,
}

struct Surface {
    definition: Definition,
    held: Option<Held>,
    hover: Option<Pos2>,
    events: Vec<Change>,
    drawn: bool,
}

struct Held {
    region: usize,
    pointer: Pos2,
    relative: Vec2,
}

thread_local! {
    static SURFACES: RefCell<BTreeMap<String, Surface>> = RefCell::default();
}

pub(super) fn register(input: &Value) -> Result<Value, String> {
    let definition: Definition =
        serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if definition.id.trim().is_empty()
        || definition.id.len() > 4096
        || !(1.0..=8192.0).contains(&definition.width)
        || !(1.0..=8192.0).contains(&definition.height)
        || definition.regions.is_empty()
        || definition.regions.len() > 128
        || definition.regions.iter().any(|r| {
            let [x, y, w, h] = r.rect;
            x < 0.0
                || y < 0.0
                || w <= 0.0
                || h <= 0.0
                || x + w > definition.width
                || y + h > definition.height
        })
    {
        return Err("Invalid script surface dimensions or regions".into());
    }
    SURFACES.with_borrow_mut(|surfaces| {
        if surfaces.len() >= 32 && !surfaces.contains_key(&definition.id) {
            return Err(super::t("ui_limit").into());
        }
        surfaces.insert(
            definition.id.clone(),
            Surface {
                definition,
                held: None,
                hover: None,
                events: Vec::new(),
                drawn: false,
            },
        );
        Ok(Value::Null)
    })
}

pub(super) fn clear() {
    SURFACES.with_borrow_mut(BTreeMap::clear);
}

pub(crate) fn close(id: &str) {
    SURFACES.with_borrow_mut(|surfaces| surfaces.remove(id));
}

pub(super) fn poll(id: &str) -> Value {
    SURFACES.with_borrow_mut(|surfaces| match surfaces.get_mut(id) {
        Some(surface) => {
            let mut frame = json!({"open":true, "events":std::mem::take(&mut surface.events)});
            if let Some(header) = &surface.definition.header {
                frame["checked"] = json!(header.checked);
            }
            frame
        }
        None => json!({"open":false, "events":[]}),
    })
}

pub(super) fn set_title(id: &str, title: String, pad_gain_instance: Option<i32>) {
    SURFACES.with_borrow_mut(|surfaces| {
        if let Some(header) = surfaces
            .get_mut(id)
            .and_then(|s| s.definition.header.as_mut())
        {
            header.title = title;
            if let Some(instance) = pad_gain_instance {
                header.pad_gain_instance = Some(instance);
            }
        }
    });
}

pub(crate) fn ids() -> Vec<String> {
    SURFACES.with_borrow(|surfaces| surfaces.keys().cloned().collect())
}

pub(crate) fn size(id: &str) -> Option<Vec2> {
    SURFACES.with_borrow(|surfaces| {
        let definition = &surfaces.get(id)?.definition;
        Some(egui::vec2(definition.width, definition.height))
    })
}

pub(crate) fn draw(ui: &mut egui::Ui, id: &str, accepts_input: bool) {
    SURFACES.with_borrow_mut(|surfaces| {
        if let Some(surface) = surfaces.get_mut(id) {
            surface.draw(ui, accepts_input);
        }
    });
}

/// Switching tabs stops drawing a canvas. Release its pointer capture even when
/// the button-up event goes to a different pane or viewport.
pub(crate) fn finish_frame() {
    SURFACES.with_borrow_mut(|surfaces| {
        for surface in surfaces.values_mut() {
            if !std::mem::take(&mut surface.drawn) {
                surface.release();
                surface.hover = None;
            }
        }
    });
}

impl Surface {
    fn draw(&mut self, ui: &mut egui::Ui, accepts_input: bool) {
        self.drawn = true;
        if let Some(header) = &mut self.definition.header {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.add_enabled(
                        accepts_input,
                        egui::Checkbox::new(&mut header.checked, &header.checkbox),
                    );
                    if header.pad_gain_meter {
                        gain_meter::draw(ui, header.checked, header.pad_gain_instance);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(&header.title).truncate());
                    });
                });
            });
        }
        let available = ui.available_rect_before_wrap();
        let size = egui::vec2(self.definition.width, self.definition.height);
        let scale = (available.width() / size.x).min(available.height() / size.y);
        if scale <= 0.0 {
            self.release();
            return;
        }
        let canvas = Rect::from_center_size(available.center(), size * scale);
        let regions: Vec<_> = self
            .definition
            .regions
            .iter()
            .map(|region| {
                let [x, y, w, h] = region.rect;
                Rect::from_min_size(
                    canvas.min + Vec2::new(x, y) * scale,
                    Vec2::new(w, h) * scale,
                )
            })
            .collect();
        let (focused, down, hover, events) = ui.input(|input| {
            (
                input.focused,
                input.pointer.primary_down(),
                input.pointer.hover_pos(),
                input.events.clone(),
            )
        });
        if accepts_input && focused {
            let can_press = |pos| {
                ui.clip_rect().contains(pos)
                    && ui
                        .ctx()
                        .layer_id_at(pos)
                        .is_none_or(|layer| layer == ui.layer_id())
            };
            self.capture(&regions, &events, self.hover.or(hover), can_press);
        }
        self.hover = hover;
        if !accepts_input || !focused || !down {
            self.release();
        }
        ui.allocate_rect(available, egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(available, 0, Color32::from_rgb(17, 20, 27));
        for (index, (region, rect)) in self.definition.regions.iter().zip(regions).enumerate() {
            let [r, g, b] = region.color;
            painter.rect_filled(rect, 0, Color32::from_rgb(r, g, b));
            if let Some(held) = self.held.as_ref().filter(|held| held.region == index) {
                painter.rect_stroke(
                    rect,
                    0,
                    egui::Stroke::new(3.0, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
                if region.cursor {
                    painter.circle_filled(
                        rect.min + held.relative * rect.size(),
                        7.0,
                        Color32::WHITE,
                    );
                }
            }
        }
    }

    fn capture(
        &mut self,
        regions: &[Rect],
        events: &[Event],
        mut hover: Option<Pos2>,
        can_press: impl Fn(Pos2) -> bool,
    ) {
        for event in events {
            match *event {
                Event::PointerButton { pos, .. } | Event::PointerMoved(pos) => hover = Some(pos),
                Event::PointerGone => hover = None,
                _ => {}
            }
            match *event {
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if self.held.is_none() && can_press(pos) => {
                    if let Some(index) = regions.iter().position(|rect| rect.contains(pos)) {
                        self.change(regions, "down", index, pos);
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    ..
                } if can_press(pos) => {
                    if let Some(index) = regions.iter().position(|rect| rect.contains(pos)) {
                        self.change(regions, "secondary", index, pos);
                    }
                }
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page,
                    delta,
                    source: egui::MouseWheelSource::Wheel | egui::MouseWheelSource::Unknown,
                    ..
                } if delta.y != 0.0 => {
                    if let Some(pos) = hover.filter(|&pos| can_press(pos)) {
                        if let Some(index) = regions.iter().position(|rect| rect.contains(pos)) {
                            self.change(regions, "wheel", index, pos);
                        }
                    }
                }
                Event::PointerMoved(pos) => {
                    if let Some(held) = self.held.as_ref().filter(|held| {
                        self.definition.regions[held.region].cursor && pos != held.pointer
                    }) {
                        self.change(regions, "move", held.region, pos);
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    if let Some(held) = &self.held {
                        self.change(regions, "up", held.region, pos);
                    }
                }
                _ => {}
            }
        }
    }

    fn release(&mut self) {
        if let Some(held) = self.held.take() {
            self.events.push(Change {
                kind: "up",
                region: held.region,
                x: held.relative.x,
                y: held.relative.y,
            });
        }
    }

    fn change(&mut self, regions: &[Rect], kind: &'static str, region: usize, pos: Pos2) {
        let rect = regions[region];
        let relative = (pos.clamp(rect.min, rect.max) - rect.min) / rect.size();
        self.events.push(Change {
            kind,
            region,
            x: relative.x,
            y: relative.y,
        });
        match kind {
            "down" | "move" => {
                self.held = Some(Held {
                    region,
                    pointer: pos,
                    relative,
                })
            }
            "up" => self.held = None,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
