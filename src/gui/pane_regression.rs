//! Headless measurements of the left/right pane split under window resizes,
//! border drags and a long code line.
use super::code_panel;
use crate::config;
use crate::script_editor::ScriptEditor;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect};

const HEIGHT: f32 = 800.0;

struct Panes {
    ctx: egui::Context,
    editor: ScriptEditor,
    code: String,
    bounds: config::Panes,
}

#[derive(Debug, Clone, Copy)]
struct Widths {
    window: f32,
    left: f32,
    right: f32,
}

impl Widths {
    fn right_percent(self) -> f32 {
        self.right / self.window * 100.0
    }

    fn left_percent(self) -> f32 {
        self.left / self.window * 100.0
    }
}

impl Panes {
    fn new() -> Self {
        Self {
            ctx: egui::Context::default(),
            editor: ScriptEditor::default(),
            code: "repl.log('x');\n".into(),
            bounds: config::Panes::default(),
        }
    }

    /// Same panel pair as `Editor::ui`: `code_panel` with the heading, the
    /// Stop button and the script editor, then the `CentralPanel`.
    fn frame(&mut self, window: f32, events: Vec<Event>) -> Widths {
        let mut left = 0.0;
        let mut right = 0.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(window, HEIGHT))),
            events,
            ..Default::default()
        };
        let (editor, code, bounds) = (&mut self.editor, &mut self.code, self.bounds);
        self.ctx
            .run_ui(input, |ui| {
                left = code_panel(ui.available_width(), bounds)
                    .show(ui, |ui| {
                        ui.heading("JavaScript");
                        let _ = ui.button("Stop");
                        editor.show(ui, code, true);
                    })
                    .response
                    .rect
                    .width();
                right = egui::CentralPanel::default()
                    .show(ui, |ui| ui.heading("Timeline"))
                    .response
                    .rect
                    .width();
            })
            .drop_without_applying_deltas();
        Widths {
            window,
            left,
            right,
        }
    }

    fn settle(&mut self, window: f32) -> Widths {
        let mut widths = self.frame(window, vec![]);
        for _ in 0..4 {
            widths = self.frame(window, vec![]);
        }
        widths
    }

    /// Drags the pane border from its current position to `to_x`.
    fn drag_border(&mut self, window: f32, to_x: f32) -> Widths {
        let from = egui::pos2(self.settle(window).left, HEIGHT / 2.0);
        let to = egui::pos2(to_x, HEIGHT / 2.0);
        self.frame(window, vec![Event::PointerMoved(from)]);
        self.frame(window, vec![press(from, true)]);
        for step in 1..=4 {
            let x = from.x + (to.x - from.x) * step as f32 / 4.0;
            self.frame(window, vec![Event::PointerMoved(egui::pos2(x, to.y))]);
        }
        self.frame(window, vec![press(to, false)]);
        self.settle(window)
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

fn record(log: &mut Vec<(String, Widths)>, label: &str, widths: Widths) {
    println!(
        "{label}: window={} left={} ({:.1}%) right={} ({:.1}%)",
        widths.window,
        widths.left,
        widths.left_percent(),
        widths.right,
        widths.right_percent()
    );
    log.push((label.into(), widths));
}

/// Runs every operation sequence and returns the measured widths.
fn measure() -> Vec<(String, Widths)> {
    let mut log = Vec::new();

    let mut panes = Panes::new();
    for window in [1250.0, 600.0, 300.0, 1250.0] {
        let widths = panes.settle(window);
        record(&mut log, &format!("resize {window}"), widths);
    }

    let mut panes = Panes::new();
    let widths = panes.drag_border(1250.0, 1250.0);
    record(&mut log, "drag border to right edge", widths);
    let widths = panes.settle(300.0);
    record(&mut log, "then resize 300", widths);

    let mut panes = Panes::new();
    let widths = panes.drag_border(1250.0, 0.0);
    record(&mut log, "drag border to left edge", widths);
    let widths = panes.settle(300.0);
    record(&mut log, "then resize 300 (left edge)", widths);

    let mut panes = Panes::new();
    panes.code = format!("var x = '{}';\n", "a".repeat(400));
    let widths = panes.settle(1250.0);
    record(&mut log, "400-char code line", widths);

    log
}

#[test]
fn both_panes_keep_ten_percent() {
    let below: Vec<_> = measure()
        .into_iter()
        .filter(|(_, widths)| widths.left_percent() < 10.0 || widths.right_percent() < 10.0)
        .collect();
    assert!(below.is_empty(), "a pane below 10%: {below:?}");
}

#[test]
fn configured_minimums_bound_the_border() {
    let bounds = config::Panes {
        left_min: 0.3,
        right_min: 0.25,
    };
    for (to_x, window) in [
        (1250.0, 1250.0),
        (0.0, 1250.0),
        (1250.0, 300.0),
        (0.0, 300.0),
    ] {
        let mut panes = Panes::new();
        panes.bounds = bounds;
        let widths = panes.drag_border(window, to_x);
        record(
            &mut Vec::new(),
            &format!("drag to {to_x} at {window}"),
            widths,
        );
        assert!(widths.left_percent() >= 29.9, "{widths:?}");
        assert!(widths.right_percent() >= 24.9, "{widths:?}");
    }
}
