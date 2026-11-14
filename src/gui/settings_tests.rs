//! Headless frames in the same order as `Editor::ui`: the shortcut first, then
//! the code pane with the script editor, and the overlay last.
use super::*;
use crate::script_editor::ScriptEditor;
use egui::{epaint::Shape, Event, Pos2, Rect};

const SCREEN: egui::Vec2 = egui::vec2(1250.0, 800.0);

struct Harness {
    ctx: egui::Context,
    settings: Settings,
    extensions: crate::extension::Extensions,
    editor: ScriptEditor,
    code: String,
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Harness {
    fn new() -> Self {
        let missing = std::env::temp_dir().join("rpp-settings-test-absent");
        Self::with_status(missing.join("status.json"))
    }

    fn with_status(status: PathBuf) -> Self {
        let ctx = egui::Context::default();
        crate::i18n::fonts(&ctx);
        let config = status.with_file_name("config.toml");
        let mut harness = Self {
            ctx,
            settings: Settings::new(Ok(status), Ok(config)),
            extensions: crate::extension::Extensions::default(),
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
        let extensions = &mut self.extensions;
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
                settings.show(&ctx, extensions);
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
fn easy_style_is_on_by_default_and_first() {
    let mut harness = Harness::new();
    assert!(harness.settings.easy);
    harness.settle(ctrl_comma());
    let easy = harness.text_rect(crate::i18n::t("settings_easy")).unwrap();
    let editor = harness
        .text_rect(crate::i18n::t("settings_editor"))
        .unwrap();
    assert!(easy.bottom() <= editor.top(), "{easy:?} {editor:?}");
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

#[test]
fn panes_of_the_builtin_dll_are_listed() {
    let mut harness = Harness::new();
    harness.settle(ctrl_comma());
    assert!(harness.text_rect("builtin/keyboard").is_some());
    assert!(harness
        .text_rect(crate::i18n::t("settings_extensions_restart"))
        .is_none());
}

#[test]
fn a_checkbox_saves_at_once_and_tells_that_restart_applies_it() {
    let directory =
        std::env::temp_dir().join(format!("rpp-settings-extensions-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let status = directory.join("status.json");
    std::fs::write(&status, r#"{"extensions": ["gone/probe"]}"#).unwrap();
    let mut harness = Harness::with_status(status.clone());
    assert_eq!(harness.settings.extensions(), ["gone/probe"]);
    harness.settle(ctrl_comma());
    let restart = crate::i18n::t("settings_extensions_restart");
    assert!(harness.text_rect(restart).is_none());

    let row = harness
        .text_rect("gone/probe")
        .expect("an on name no DLL has is listed");
    harness.click(row.center());
    assert_eq!(crate::status::extensions(&status), (vec![], None));
    assert!(harness.text_rect(restart).is_some());
    assert_eq!(harness.settings.extensions(), ["gone/probe"]);

    let row = harness
        .text_rect("gone/probe")
        .expect("the row stays until restart");
    harness.click(row.center());
    assert_eq!(
        crate::status::extensions(&status),
        (vec!["gone/probe".into()], None)
    );
    assert!(harness.text_rect(restart).is_none());
    std::fs::remove_dir_all(&directory).unwrap();
}
