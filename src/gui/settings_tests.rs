//! Headless frames in the same order as `Editor::ui`: the shortcut first, then
//! the code pane with the script editor, and the overlay last.
use super::*;
use crate::script_editor::ScriptEditor;
use egui::{epaint::Shape, Event, Pos2, Rect};

const SCREEN: egui::Vec2 = egui::vec2(1250.0, 800.0);

struct Harness {
    ctx: egui::Context,
    settings: Settings,
    editor: ScriptEditor,
    code: String,
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::i18n::fonts(&ctx);
        let missing = std::env::temp_dir().join("rpp-settings-test-absent");
        let mut harness = Self {
            ctx,
            settings: Settings::new(
                Ok(missing.join("status.json")),
                Ok(missing.join("config.toml")),
            ),
            editor: ScriptEditor::default(),
            code: "repl.log('x');\n".into(),
            shapes: Vec::new(),
        };
        harness.frame(vec![]);
        harness
            .ctx
            .memory_mut(|memory| memory.request_focus(egui::Id::unique("script_text")));
        harness.frame(vec![]);
        harness
    }

    fn frame(&mut self, events: Vec<Event>) {
        let (settings, editor, code) = (&mut self.settings, &mut self.editor, &mut self.code);
        let output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
                events,
                ..Default::default()
            },
            |ui| {
                let ctx = ui.ctx().clone();
                settings.shortcut(&ctx);
                egui::Panel::left("code").show(ui, |ui| {
                    editor.show(ui, code, true);
                });
                settings.show(&ctx);
            },
        );
        self.shapes = output.shapes.clone();
        output.drop_without_applying_deltas();
    }

    fn settle(&mut self, events: Vec<Event>) {
        self.frame(events);
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    fn editor_focused(&self) -> bool {
        self.ctx
            .memory(|memory| memory.has_focus(egui::Id::unique("script_text")))
    }

    fn text_rect(&self, needle: &str) -> Option<Rect> {
        fn find(shape: &Shape, needle: &str) -> Option<Rect> {
            match shape {
                Shape::Text(text) if text.galley.text() == needle => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, needle)),
                _ => None,
            }
        }
        self.shapes
            .iter()
            .find_map(|shape| find(&shape.shape, needle))
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(at, true)]);
        self.settle(vec![button(at, false)]);
    }
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// `Ctrl+,` with the text the OS may send alongside it.
fn ctrl_comma() -> Vec<Event> {
    vec![key(Key::Comma, Modifiers::CTRL), Event::Text(",".into())]
}

#[test]
fn ctrl_comma_opens_over_the_focused_editor_and_escape_closes() {
    let mut harness = Harness::new();
    assert!(harness.editor_focused());
    harness.settle(ctrl_comma());
    assert!(harness.settings.open);
    assert_eq!(
        harness.code, "repl.log('x');\n",
        "the comma reached the code"
    );
    let label = crate::i18n::t("settings_open_config");
    assert!(harness.text_rect(label).is_some(), "overlay is not drawn");
    harness.settle(vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!harness.settings.open);
    assert!(harness.text_rect(label).is_none(), "overlay is still drawn");
}

#[test]
fn ctrl_comma_toggles() {
    let mut harness = Harness::new();
    harness.settle(ctrl_comma());
    assert!(harness.settings.open);
    harness.settle(ctrl_comma());
    assert!(!harness.settings.open);
    assert_eq!(harness.code, "repl.log('x');\n");
}

#[test]
fn outside_click_closes() {
    let mut harness = Harness::new();
    harness.settle(ctrl_comma());
    assert!(harness.settings.open);
    harness.click(egui::pos2(10.0, SCREEN.y - 10.0));
    assert!(!harness.settings.open);
}

#[test]
fn close_button_closes() {
    let mut harness = Harness::new();
    harness.settle(ctrl_comma());
    let close = harness.text_rect("×").expect("close button is not drawn");
    harness.click(close.center());
    assert!(!harness.settings.open);
}

#[test]
fn input_has_focus_and_enter_saves() {
    let mut harness = Harness::new();
    harness.settle(ctrl_comma());
    assert!(harness
        .ctx
        .memory(|memory| memory.has_focus(egui::Id::unique(INPUT))));
    harness.settle(vec![
        Event::Text("relative.exe".into()),
        key(Key::Enter, Modifiers::NONE),
    ]);
    assert_eq!(harness.settings.input, "relative.exe");
    assert_eq!(
        harness.settings.error,
        crate::i18n::t("settings_editor_invalid")
    );
    assert!(harness.settings.open);
}

#[test]
fn open_config_without_editor_reports() {
    let mut harness = Harness::new();
    harness.settle(ctrl_comma());
    let open = harness
        .text_rect(crate::i18n::t("settings_open_config"))
        .unwrap();
    harness.click(open.center());
    assert_eq!(
        harness.settings.error,
        crate::i18n::t("settings_editor_missing")
    );
}
