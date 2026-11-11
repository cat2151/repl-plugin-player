use crate::execution::{self, Request, Scope};
use eframe::egui::{self, text::CCursor, Color32, Key, Modifiers};
use std::time::{Duration, Instant};

mod cell_navigation;
mod folding;

#[derive(Default)]
pub struct ScriptEditor {
    primary: usize,
    secondary: usize,
    flash: Option<(Request, Instant)>,
    snippets: crate::snippets_ui::SnippetsPanel,
    highlight: crate::syntax_highlight::JavaScriptHighlight,
    folds: folding::Folding,
}

impl ScriptEditor {
    pub fn initial_code(&mut self) -> String {
        self.snippets.initial()
    }

    pub fn insert_snippet(
        &mut self,
        ctx: &egui::Context,
        code: &mut String,
        snippet: &crate::snippets::Snippet,
    ) -> Request {
        use egui::text::CCursorRange;
        let id = egui::Id::unique("script_text");
        self.folds.reveal_all();
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let before = CCursorRange {
            primary: CCursor::new(self.primary),
            secondary: CCursor::new(self.secondary),
            h_pos: None,
        };
        let mut undoer = state.undoer();
        undoer.add_undo(&(before, code.clone()));
        let request = crate::snippet_insert::insert(code, self.primary, snippet);
        let selected = CCursorRange {
            primary: CCursor::new(request.chars.start),
            secondary: CCursor::new(request.chars.end),
            h_pos: None,
        };
        undoer.add_undo(&(selected, code.clone()));
        state.set_undoer(undoer);
        state.cursor.set_char_range(Some(selected));
        state.store(ctx, id);
        self.primary = request.chars.start;
        self.secondary = request.chars.end;
        self.flash = None;
        ctx.memory_mut(|memory| memory.request_focus(id));
        request
    }

    pub fn executed(&mut self, request: Request) {
        self.flash = Some((request, Instant::now()));
    }

    pub fn show(&mut self, ui: &mut egui::Ui, code: &mut String, ready: bool) -> Option<Request> {
        let id = egui::Id::unique("script_text");
        // Consume execution shortcuts before TextEdit can insert a newline.
        // Also consume them while preparing/stopping, without executing anything.
        let shortcut = if ui.memory(|m| m.has_focus(id)) {
            ui.input_mut(|input| {
                if input.consume_key(Modifiers::CTRL, Key::Enter) {
                    Some(Scope::Cell)
                } else if input.consume_key(Modifiers::SHIFT, Key::Enter) {
                    Some(Scope::Line)
                } else {
                    None
                }
            })
        } else {
            None
        };
        let mut action = shortcut.filter(|_| ready);
        let mut snippet_request = None;
        ui.horizontal_wrapped(|ui| {
            for (label, scope) in [
                (crate::i18n::t("m012"), Scope::All),
                (crate::i18n::t("m013"), Scope::Cell),
                (crate::i18n::t("m014"), Scope::Line),
            ] {
                if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                    action = Some(scope);
                }
            }
            if let Some(snippet) = self.snippets.show(ui, code, self.primary, ready) {
                snippet_request = Some(self.insert_snippet(ui.ctx(), code, &snippet));
            }
        });
        ui.small(crate::i18n::t("m015"));
        egui::CollapsingHeader::new(crate::i18n::t("m016"))
            .id_salt("m016")
            .show(ui, |ui| {
                ui.label(crate::i18n::t("m018"));
                ui.label(crate::i18n::t("m019"));
                ui.label(crate::i18n::t("m020"));
                ui.label(crate::i18n::t("m021"));
            });
        let size = ui.available_size();
        egui::ScrollArea::both()
            .id_salt("script_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let cell_target = cell_navigation::handle(ui, id, code);
                self.folds.prepare(ui, id, code);
                page_navigation(ui, id, size.y);
                ui.visuals_mut().text_cursor.stroke.color = Color32::from_rgb(0xf8, 0xf8, 0xf0);
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(0x49, 0x48, 0x3e);
                ui.visuals_mut().selection.stroke.color = Color32::from_rgb(0xf8, 0xf8, 0xf2);
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let mut job = self.highlight.layout(
                        text.as_str(),
                        egui::TextStyle::Monospace.resolve(ui.style()),
                    );
                    job.wrap.max_width = wrap_width;
                    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                    self.folds.layout(galley)
                };
                let gutter = 20.0;
                let origin = ui.cursor().min;
                let text_rect = egui::Rect::from_min_size(
                    origin + egui::vec2(gutter, 0.0),
                    egui::vec2((size.x - gutter).max(0.0), size.y),
                );
                let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
                let mut output = egui::TextEdit::multiline(code)
                    .id(id)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .background_color(crate::syntax_highlight::BACKGROUND)
                    .layouter(&mut layouter)
                    .desired_width(f32::INFINITY)
                    .min_size(text_rect.size())
                    .show(&mut text_ui);
                ui.allocate_rect(
                    egui::Rect::from_min_max(origin, output.response.rect.max),
                    egui::Sense::hover(),
                );
                self.folds.controls(ui, id, code, &mut output);
                paint_cell_separators(ui, &output);
                if let Some(range) = output.cursor_range {
                    self.primary = range.primary.index.0;
                    self.secondary = range.secondary.index.0;
                }
                if let Some(target) = snippet_request
                    .as_ref()
                    .map(|r| r.chars.start)
                    .or(cell_target)
                {
                    let cursor = output.galley.pos_from_cursor(CCursor::new(target));
                    ui.scroll_to_rect(
                        cursor.translate(output.galley_pos.to_vec2()),
                        snippet_request.as_ref().map(|_| egui::Align::Center),
                    );
                }
                if output.response.changed() {
                    self.flash = None;
                }
                if let Some((request, started)) = &self.flash {
                    if started.elapsed() < Duration::from_millis(250) {
                        let start = output
                            .galley
                            .pos_from_cursor(CCursor::new(request.chars.start));
                        let end = output
                            .galley
                            .pos_from_cursor(CCursor::new(request.chars.end.saturating_sub(1)));
                        let rect = egui::Rect::from_min_max(
                            output.galley_pos + egui::vec2(0.0, start.top()),
                            output.galley_pos + egui::vec2(3.0, end.bottom()),
                        );
                        ui.painter()
                            .with_clip_rect(output.text_clip_rect)
                            .rect_filled(rect, 0.0, Color32::from_rgb(70, 220, 150));
                    }
                }
            });
        snippet_request.or_else(|| {
            action
                .map(|scope| execution::request(code, self.primary, self.secondary, scope))
                .filter(|request| !request.code.trim().is_empty())
        })
    }
}

fn page_navigation(ui: &mut egui::Ui, id: egui::Id, available_height: f32) {
    if !ui.memory(|memory| memory.has_focus(id)) {
        return;
    }
    let height = ui.clip_rect().height().min(available_height);
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let rows = ((height / row_height).floor() as usize)
        .saturating_sub(1)
        .max(1);
    // egui 0.37 has no PageUp/PageDown handling in TextEdit. Expand in place
    // so its native cursor, selection, scrolling and event ordering still apply.
    ui.input_mut(|input| {
        input.events = std::mem::take(&mut input.events)
            .into_iter()
            .flat_map(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    repeat,
                    ..
                } = &event
                {
                    if !modifiers.ctrl && !modifiers.alt && !modifiers.command && !modifiers.mac_cmd
                    {
                        let arrow = match key {
                            Key::PageUp => Some(Key::ArrowUp),
                            Key::PageDown => Some(Key::ArrowDown),
                            _ => None,
                        };
                        if let Some(key) = arrow {
                            return vec![
                                egui::Event::Key {
                                    key,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: *repeat,
                                    modifiers: *modifiers,
                                };
                                rows
                            ];
                        }
                    }
                }
                vec![event]
            })
            .collect();
    });
}

fn paint_cell_separators(ui: &egui::Ui, output: &egui::text_edit::TextEditOutput) {
    let clip = output.text_clip_rect.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    let mut offset = 0;
    for line in output.galley.job.text.split_inclusive('\n') {
        if execution::marker(line) {
            // Use laid-out character positions so Unicode, CRLF and scrolling
            // follow the same coordinates as the editable text.
            let row = output.galley.pos_from_cursor(CCursor::new(offset));
            let y = output.galley_pos.y + row.top() + 0.5;
            if y >= clip.top() && y <= clip.bottom() {
                painter.line_segment(
                    [egui::pos2(clip.left(), y), egui::pos2(clip.right(), y)],
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(0x60, 0x60, 0x54)),
                );
            }
        }
        offset += line.chars().count();
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod navigation_tests;
