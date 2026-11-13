//! Headless frames of `Editor::dock`: pointer events in, drawn text shapes out.
use super::Pane;
use crate::gui::Editor;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect, Shape, Vec2};

pub(super) const DEFAULT: Vec2 = egui::vec2(1250.0, 800.0);

pub(super) struct Harness {
    pub ctx: egui::Context,
    pub editor: Editor,
    shapes: Vec<egui::epaint::ClippedShape>,
    /// Every viewport command the frames sent, oldest first.
    pub commands: Vec<(egui::ViewportId, egui::ViewportCommand)>,
}

impl Harness {
    pub fn new() -> Self {
        let ctx = egui::Context::default();
        crate::i18n::fonts(&ctx);
        let missing = std::env::temp_dir().join("rpp-dock-test-absent");
        let settings = super::super::settings::Settings::new(
            Ok(missing.join("status.json")),
            Ok(missing.join("config.toml")),
        );
        let editor = Editor::new(vec![], settings, None);
        Self {
            ctx,
            editor,
            shapes: vec![],
            commands: vec![],
        }
    }

    pub fn frame(&mut self, size: Vec2, events: Vec<Event>) {
        let editor = &mut self.editor;
        let output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                // The parts of `App::ui` that run the editor's requests.
                let ctx = ui.ctx().clone();
                editor.run_queued(&ctx);
                editor.dock(ui, editor.snippet_execution.is_pending());
                editor.snippet_execution.show_overlay(&ctx);
            },
        );
        self.shapes = output.shapes.clone();
        for (id, viewport) in &output.viewport_output {
            let sent = viewport
                .commands
                .iter()
                .map(|command| (*id, command.clone()));
            self.commands.extend(sent);
        }
        output.drop_without_applying_deltas();
    }

    pub fn settle(&mut self, size: Vec2) {
        for _ in 0..4 {
            self.frame(size, vec![]);
        }
    }

    /// The topmost drawn text equal to `needle`.
    pub fn text(&self, needle: &str) -> Option<Rect> {
        fn find(shape: &Shape, needle: &str, found: &mut Vec<Rect>) {
            match shape {
                Shape::Text(text) if text.galley.text() == needle => {
                    found.push(Rect::from_min_size(text.pos, text.galley.size()));
                }
                Shape::Vec(shapes) => shapes.iter().for_each(|s| find(s, needle, found)),
                _ => {}
            }
        }
        let mut found = vec![];
        for shape in &self.shapes {
            find(&shape.shape, needle, &mut found);
        }
        found.into_iter().min_by(|a, b| a.top().total_cmp(&b.top()))
    }

    pub fn click(&mut self, at: Pos2, button: PointerButton) {
        let press = |pressed| Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(DEFAULT, vec![Event::PointerMoved(at)]);
        self.frame(DEFAULT, vec![press(true)]);
        self.frame(DEFAULT, vec![press(false)]);
        self.settle(DEFAULT);
    }

    pub fn click_text(&mut self, needle: &str, button: PointerButton) -> Result<(), String> {
        let rect = self
            .text(needle)
            .ok_or(format!("{needle:?} is not drawn"))?;
        self.click(rect.center(), button);
        Ok(())
    }

    /// The tab's menu item that moves `pane` into its own window.
    pub fn float(&mut self, pane: &Pane) -> Result<(), String> {
        let title = pane.title(&crate::script_ui::snapshot());
        self.click_text(&title, PointerButton::Secondary)?;
        self.click_text(crate::i18n::t("pane_float"), PointerButton::Primary)
    }

    /// The button in the only floating window that puts its pane back.
    pub fn dock(&mut self) -> Result<(), String> {
        self.click_text(crate::i18n::t("pane_dock"), PointerButton::Primary)
    }

    pub fn in_tree(&self, pane: &Pane) -> bool {
        self.editor.layout.tree.tiles.find_pane(pane).is_some()
    }
}
