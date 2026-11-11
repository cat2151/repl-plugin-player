//! Fold only the layout: the TextEdit buffer, character offsets and undo history
//! always contain the complete script.
use crate::execution;
use eframe::egui::{self, text::CCursor, text::CCursorRange, text::Galley, Event, Key, Modifiers};
use std::{collections::BTreeSet, ops::Range, sync::Arc};

#[derive(Default)]
pub(super) struct Folding {
    text: String,
    closed: BTreeSet<usize>,
}

struct Cell {
    start: usize,
    hidden: Range<usize>,
}

fn cells(text: &str) -> Vec<Cell> {
    let mut result: Vec<Cell> = Vec::new();
    let mut offset = 0;
    let length = text.chars().count();
    for line in text.split_inclusive('\n') {
        if offset == 0 || execution::marker(line) {
            if let Some(cell) = result.last_mut() {
                cell.hidden.end = offset;
            }
            result.push(Cell {
                start: offset,
                hidden: offset + line.chars().count()..length,
            });
        }
        offset += line.chars().count();
    }
    result
}

impl Folding {
    pub(super) fn reveal_all(&mut self) {
        self.closed.clear();
    }

    fn sync(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        // Track unchanged headers through ordinary edits and undo. Discard a
        // fold when its header is edited/deleted rather than attaching it to
        // a different cell by ordinal number.
        let old: Vec<_> = self.text.chars().collect();
        let new: Vec<_> = text.chars().collect();
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(new[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.closed = self
            .closed
            .iter()
            .filter_map(|&start| {
                if start < prefix {
                    Some(start)
                } else if start >= old.len() - suffix {
                    Some(new.len() - (old.len() - start))
                } else {
                    None
                }
            })
            .collect();
        let current = cells(text);
        self.closed
            .retain(|start| current.iter().any(|cell| cell.start == *start));
        self.text = text.to_owned();
    }

    fn toggle(&mut self, ctx: &egui::Context, id: egui::Id, cell: &Cell) {
        if !self.closed.remove(&cell.start) && !cell.hidden.is_empty() {
            self.closed.insert(cell.start);
        }
        if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(CCursor::new(cell.start))));
            state.store(ctx, id);
        }
        ctx.memory_mut(|memory| memory.request_focus(id));
        ctx.request_repaint();
    }

    pub(super) fn prepare(&mut self, ui: &mut egui::Ui, id: egui::Id, text: &str) {
        self.sync(text);
        if !ui.memory(|m| m.has_focus(id)) {
            return;
        }
        let Some(range) =
            egui::TextEdit::load_state(ui.ctx(), id).and_then(|s| s.cursor.char_range())
        else {
            return;
        };
        let primary = range.primary.index.0;
        let secondary = range.secondary.index.0;
        let current = cells(text);
        if ui.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Period)) {
            if let Some(cell) = current.iter().rev().find(|cell| cell.start <= primary) {
                self.toggle(ui.ctx(), id, cell);
            }
            return;
        }
        // Reveal the affected region before editing, including deletions across
        // a fold boundary. Native TextEdit performs the actual edit and undo.
        let editing = ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    Event::Text(_)
                        | Event::Paste(_)
                        | Event::Cut
                        | Event::Ime(_)
                        | Event::Key {
                            key: Key::Backspace
                                | Key::Delete
                                | Key::Enter
                                | Key::Tab
                                | Key::Z
                                | Key::Y,
                            pressed: true,
                            ..
                        }
                )
            })
        });
        if editing {
            let broad = ui.input(|i| i.events.iter().any(|e| matches!(e,
                Event::Key { key: Key::Z | Key::Y | Key::Backspace | Key::Delete, modifiers, pressed: true, .. }
                    if modifiers.ctrl || modifiers.command
            )));
            let touched =
                primary.min(secondary).saturating_sub(1)..primary.max(secondary).saturating_add(1);
            self.closed.retain(|start| {
                !broad
                    && !current.iter().any(|cell| {
                        cell.start == *start
                            && touched.start <= cell.hidden.end
                            && touched.end >= cell.start
                    })
            });
        }
    }

    pub(super) fn layout(&mut self, galley: Arc<Galley>) -> Arc<Galley> {
        self.sync(&galley.job.text);
        if self.closed.is_empty() {
            return galley;
        }
        let hidden: Vec<_> = cells(&self.text)
            .into_iter()
            .filter(|cell| self.closed.contains(&cell.start))
            .map(|cell| cell.hidden)
            .collect();
        let mut galley = (*galley).clone();
        let mut offset = 0;
        let mut removed = 0.0;
        galley.mesh_bounds = egui::Rect::NOTHING;
        galley.num_vertices = 0;
        galley.num_indices = 0;
        for placed in &mut galley.rows {
            let count = placed.char_count_including_newline().0;
            placed.pos.y -= removed;
            if hidden.iter().any(|range| range.contains(&offset)) {
                removed += placed.size.y;
                let row = Arc::make_mut(&mut placed.row);
                row.size = egui::Vec2::ZERO;
                row.visuals = Default::default();
                // Selection recoloring uses these indices even on rows with
                // no mesh. Keep the character slots, but point at an empty
                // vertex range so selections spanning a fold remain valid.
                for glyph in &mut row.glyphs {
                    glyph.first_vertex = 0;
                }
            }
            galley.mesh_bounds |= placed.visuals.mesh_bounds.translate(placed.pos.to_vec2());
            galley.num_vertices += placed.visuals.mesh.vertices.len();
            galley.num_indices += placed.visuals.mesh.indices.len();
            offset += count;
        }
        galley.rect.max.y -= removed;
        galley.rect.max.x = galley
            .rows
            .iter()
            .map(|row| row.rect().right())
            .fold(0.0, f32::max);
        Arc::new(galley)
    }

    pub(super) fn controls(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        text: &str,
        output: &mut egui::text_edit::TextEditOutput,
    ) {
        self.sync(text);
        let current = cells(text);
        // Keyboard movement into hidden text reveals it on the following frame.
        if let Some(range) = output.cursor_range {
            for cell in &current {
                if cell.hidden.contains(&range.primary.index.0) && self.closed.remove(&cell.start) {
                    ui.ctx().request_repaint();
                }
            }
        }
        for cell in current {
            if cell.hidden.is_empty() {
                continue;
            }
            let row = output.galley.pos_from_cursor(CCursor::new(cell.start));
            let rect = egui::Rect::from_min_size(
                egui::pos2(
                    output.response.rect.left() - 20.0,
                    output.galley_pos.y + row.top(),
                ),
                egui::vec2(18.0, row.height()),
            );
            if !ui.is_rect_visible(rect) {
                continue;
            }
            let response = ui.interact(rect, id.with(("fold", cell.start)), egui::Sense::click());
            let color = if response.hovered() {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().text_color()
            };
            let center = rect.center();
            let points = if self.closed.contains(&cell.start) {
                vec![
                    center + egui::vec2(-2.0, -4.0),
                    center + egui::vec2(3.0, 0.0),
                    center + egui::vec2(-2.0, 4.0),
                ]
            } else {
                vec![
                    center + egui::vec2(-4.0, -2.0),
                    center + egui::vec2(0.0, 3.0),
                    center + egui::vec2(4.0, -2.0),
                ]
            };
            ui.painter().add(egui::Shape::convex_polygon(
                points,
                color,
                egui::Stroke::NONE,
            ));
            if response.clicked() {
                self.toggle(ui.ctx(), id, &cell);
                output.cursor_range = Some(CCursorRange::one(CCursor::new(cell.start)));
            }
        }
    }
}

#[cfg(test)]
mod tests;
