//! Headless frames of `Editor::dock` without native code: the first layout,
//! a window resize, a border drag and a pane's trip to its own window and back.
use super::harness::{Harness, DEFAULT};
use super::*;
use egui::{Event, Modifiers, PointerButton, Pos2, Rect};

const COLUMN: [Pane; 6] = [
    Pane::Main,
    Pane::Keyboard,
    Pane::Scope,
    Pane::Log,
    Pane::Reference,
    Pane::Console,
];

impl Harness {
    fn rect(&self, pane: &Pane) -> Rect {
        let tiles = &self.editor.layout.tree.tiles;
        let rect = tiles.find_pane(pane).and_then(|id| tiles.rect(id));
        rect.unwrap_or_else(|| panic!("{pane:?} has no rect"))
    }

    fn report(&self, label: &str) {
        for pane in [Pane::Editor].into_iter().chain(COLUMN) {
            println!("{label}: {pane:?} {:?}", self.rect(&pane));
        }
    }
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn first_layout_puts_editor_left_sections_under_main_and_console_bottom_right() {
    let mut harness = Harness::new();
    harness.settle(DEFAULT);
    harness.report("1250x800");
    let editor = harness.rect(&Pane::Editor);
    let main = harness.rect(&Pane::Main);
    assert!(editor.right() <= main.left(), "{editor:?} {main:?}");
    assert!((editor.width() - 630.0).abs() < 20.0, "{editor:?}");
    for pair in COLUMN.windows(2) {
        let (upper, lower) = (harness.rect(&pair[0]), harness.rect(&pair[1]));
        assert!(
            upper.bottom() <= lower.top(),
            "{pair:?}: {upper:?} {lower:?}"
        );
        assert_eq!(upper.x_range(), lower.x_range(), "{pair:?}");
    }
    // The keyboard and scope drawings (82 and 100 high plus the frame margin) are not
    // cut.
    assert!(harness.rect(&Pane::Keyboard).height() >= 98.0);
    assert!(harness.rect(&Pane::Scope).height() >= 116.0);
    assert!(harness.editor.layout.floating.is_empty());
    let console = harness.rect(&Pane::Console);
    assert_eq!(console.max, DEFAULT.to_pos2(), "{console:?}");
}

#[test]
fn shrinking_the_window_keeps_every_pane() {
    let mut harness = Harness::new();
    harness.settle(DEFAULT);
    harness.settle(egui::vec2(800.0, 600.0));
    harness.report("800x600");
    for pane in [Pane::Editor].into_iter().chain(COLUMN) {
        let rect = harness.rect(&pane);
        assert!(
            rect.width() > 0.0 && rect.height() > 0.0,
            "{pane:?} {rect:?}"
        );
    }
}

#[test]
fn dragging_the_border_changes_the_split() {
    let mut harness = Harness::new();
    harness.settle(DEFAULT);
    let before = harness.rect(&Pane::Editor).width();
    let x = (harness.rect(&Pane::Editor).right() + harness.rect(&Pane::Main).left()) / 2.0;
    let from = egui::pos2(x, DEFAULT.y / 2.0);
    harness.frame(DEFAULT, vec![Event::PointerMoved(from)]);
    harness.frame(DEFAULT, vec![press(from, true)]);
    for step in 1..=4 {
        let to = egui::pos2(x - 50.0 * step as f32, from.y);
        harness.frame(DEFAULT, vec![Event::PointerMoved(to)]);
    }
    harness.frame(DEFAULT, vec![press(egui::pos2(x - 200.0, from.y), false)]);
    harness.settle(DEFAULT);
    let after = harness.rect(&Pane::Editor).width();
    println!("editor width {before} -> {after}");
    assert!((before - after - 200.0).abs() < 20.0, "{before} -> {after}");
}

#[test]
fn keyboard_goes_to_its_own_window_and_back_into_the_clicked_tabs() {
    let mut harness = Harness::new();
    harness.settle(DEFAULT);
    let scope = harness.text("Scope").expect("scope tab");
    harness.click(scope.center(), PointerButton::Primary);
    harness.float(&Pane::Keyboard).unwrap();
    assert!(!harness.in_tree(&Pane::Keyboard));
    assert_eq!(harness.editor.layout.floating_panes(), [Pane::Keyboard]);
    // Headless, the immediate viewport is an embedded window with the viewport's id.
    let window = egui::Id::unique(Pane::Keyboard.viewport());
    let window = harness.ctx.memory(|memory| memory.area_rect(window));
    println!("keyboard window {window:?}");
    assert!(window.is_some_and(|rect| rect.width() > 0.0 && rect.height() > 0.0));
    harness.dock().unwrap();
    assert!(harness.editor.layout.floating.is_empty());
    let tiles = &harness.editor.layout.tree.tiles;
    let parent = |pane| tiles.find_pane(&pane).and_then(|id| tiles.parent_of(id));
    assert!(parent(Pane::Keyboard).is_some());
    assert_eq!(parent(Pane::Keyboard), parent(Pane::Scope));
    assert!(harness.text(crate::i18n::t("pane_dock")).is_none());
}

#[test]
fn script_windows_start_in_their_own_window_and_follow_the_registry() {
    let show = |title: &str| {
        let definition = serde_json::json!({"definition": {"id": "w", "title": title,
            "widgets": [{"id": "l", "type": "label", "label": "text"}]}});
        crate::script_ui::dispatch("ui.show", &definition).unwrap();
    };
    let pane = Pane::Script("w".into());
    crate::script_ui::enable(true);
    let mut harness = Harness::new();
    harness.settle(DEFAULT);
    show("Script");
    harness.settle(DEFAULT);
    assert_eq!(
        harness.editor.layout.floating_panes(),
        std::slice::from_ref(&pane)
    );
    harness.dock().unwrap();
    assert!(harness.in_tree(&pane) && harness.editor.layout.floating.is_empty());
    // Shown again with a new title, the window stays a tab.
    show("Renamed");
    harness.settle(DEFAULT);
    assert!(harness.in_tree(&pane));
    let tab = harness.text("Renamed").expect("renamed tab");
    harness.click(
        egui::pos2(tab.right() + 10.0, tab.center().y),
        PointerButton::Primary,
    );
    assert!(crate::script_ui::snapshot().is_empty());
    assert!(!harness.in_tree(&pane) && harness.editor.layout.floating.is_empty());
    show("Script");
    harness.settle(DEFAULT);
    crate::script_ui::dispatch("ui.close", &serde_json::json!({"id": "w"})).unwrap();
    harness.settle(DEFAULT);
    assert!(harness.editor.layout.floating.is_empty());
    assert!(harness.text(crate::i18n::t("pane_dock")).is_none());
    crate::script_ui::enable(false);
}

#[test]
fn a_pane_window_is_brought_to_the_front_once_when_it_opens() {
    let focused = |harness: &Harness, pane: &Pane| {
        let focus = (pane.viewport(), egui::ViewportCommand::Focus);
        harness
            .commands
            .iter()
            .filter(|sent| **sent == focus)
            .count()
    };
    let pane = Pane::Script("front".into());
    crate::script_ui::enable(true);
    let mut harness = Harness::new();
    // Panes get viewports of their own, as under eframe: commands sent to an embedded
    // window's viewport are dropped.
    harness.ctx.set_embed_viewports(false);
    egui::Context::set_immediate_viewport_renderer(|ctx, viewport| {
        let id = viewport.ids.this;
        let input = egui::RawInput {
            viewport_id: id,
            viewports: std::iter::once((id, egui::ViewportInfo::default())).collect(),
            ..Default::default()
        };
        let mut contents = viewport.viewport_ui_cb;
        let output = ctx.run_ui(input, |ui| contents(ui));
        output.drop_without_applying_deltas();
    });
    harness.settle(DEFAULT);
    let definition = serde_json::json!({"definition": {"id": "front", "title": "Front",
        "widgets": [{"id": "l", "type": "label", "label": "text"}]}});
    crate::script_ui::dispatch("ui.show", &definition).unwrap();
    harness.settle(DEFAULT);
    harness.settle(DEFAULT);
    assert_eq!(focused(&harness, &pane), 1);
    let keyboard = harness.text("Keyboard").expect("keyboard tab");
    harness.click(keyboard.center(), PointerButton::Secondary);
    harness
        .click_text(crate::i18n::t("pane_float"), PointerButton::Primary)
        .unwrap();
    harness.settle(DEFAULT);
    assert_eq!(
        harness.editor.layout.floating_panes(),
        [pane.clone(), Pane::Keyboard]
    );
    assert_eq!(focused(&harness, &Pane::Keyboard), 1);
    assert_eq!(focused(&harness, &pane), 1);
    crate::script_ui::dispatch("ui.close", &serde_json::json!({"id": "front"})).unwrap();
    harness.settle(DEFAULT);
    crate::script_ui::enable(false);
}

#[test]
fn a_window_from_the_file_holds_a_tree_whose_border_drags_and_goes_back_whole() {
    let text = "[[window]]\ncolumn = [{ view = \"keyboard\" }, { view = \"scope\" }]\nx = 0\ny = 0\nwidth = 600\nheight = 500\n";
    let mut harness = Harness::new();
    harness.editor.layout = super::file::read(text).unwrap();
    harness.settle(DEFAULT);
    assert_eq!(
        harness.editor.layout.floating_panes(),
        [Pane::Keyboard, Pane::Scope]
    );
    let rect = |harness: &Harness, pane: &Pane| {
        let tiles = &harness.editor.layout.floating[0].tree.tiles;
        let rect = tiles.find_pane(pane).and_then(|id| tiles.rect(id));
        rect.unwrap_or_else(|| panic!("{pane:?} has no rect in the window"))
    };
    let (keyboard, scope) = (
        rect(&harness, &Pane::Keyboard),
        rect(&harness, &Pane::Scope),
    );
    println!("window: keyboard {keyboard:?} scope {scope:?}");
    assert!(keyboard.bottom() <= scope.top(), "{keyboard:?} {scope:?}");
    // The border is at keyboard's bottom; scope's tab bar is under it.
    let y = keyboard.bottom() + 1.0;
    let from = egui::pos2(keyboard.center().x, y);
    harness.frame(DEFAULT, vec![Event::PointerMoved(from)]);
    harness.frame(DEFAULT, vec![press(from, true)]);
    for step in 1..=4 {
        let to = egui::pos2(from.x, y + 20.0 * step as f32);
        harness.frame(DEFAULT, vec![Event::PointerMoved(to)]);
    }
    harness.frame(DEFAULT, vec![press(egui::pos2(from.x, y + 80.0), false)]);
    harness.settle(DEFAULT);
    let after = rect(&harness, &Pane::Keyboard).height();
    println!("keyboard height {} -> {after}", keyboard.height());
    assert!((after - keyboard.height() - 80.0).abs() < 20.0);
    // Clicks in the window are not the main tree's: the main tree keeps its target.
    let target = harness.editor.layout.target();
    harness.dock().unwrap();
    assert!(harness.editor.layout.floating.is_empty());
    assert!(harness.in_tree(&Pane::Keyboard) && harness.in_tree(&Pane::Scope));
    let tiles = &harness.editor.layout.tree.tiles;
    let parent = |pane| tiles.find_pane(&pane).and_then(|id| tiles.parent_of(id));
    assert_eq!(parent(Pane::Keyboard), target);
    assert_eq!(parent(Pane::Scope), target);
}
