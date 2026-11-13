//! Headless frames that draw through `repl_keyboard.dll`, so a missing DLL fails them.
use eframe::egui::{self, epaint::Shape, Color32, Event, Pos2, Rect};

const WHITE_ACTIVE: Color32 = Color32::from_rgb(60, 190, 150);
const WHITE: Color32 = Color32::from_gray(225);
const BLACK: Color32 = Color32::from_gray(35);

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|style| {
        style.animation_time = 0.0;
        style.interaction.tooltip_delay = 0.0;
    });
    ctx
}

/// One frame at `time`; returns every drawn shape.
fn frame(
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
    mut draw: impl FnMut(&mut egui::Ui),
) -> Vec<Shape> {
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
            time: Some(time),
            events,
            ..Default::default()
        },
        // A wheel over the bare root `Ui` does not scroll a `ScrollArea`; inside a panel it does.
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        },
    );
    let mut shapes = vec![];
    fn flatten(shape: Shape, out: &mut Vec<Shape>) {
        match shape {
            Shape::Vec(inner) => inner.into_iter().for_each(|s| flatten(s, out)),
            other => out.push(other),
        }
    }
    for clipped in output.shapes.clone() {
        flatten(clipped.shape, &mut shapes);
    }
    output.drop_without_applying_deltas();
    shapes
}

/// Filled key rectangles of `fill`, left to right.
fn keys(shapes: &[Shape], fill: Color32) -> Vec<Rect> {
    let mut found: Vec<Rect> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| a.left().total_cmp(&b.left()));
    found
}

#[test]
fn active_pitch_is_drawn_in_the_active_color() {
    let ctx = context();
    let quiet = frame(&ctx, 0.0, vec![], |ui| super::show(ui, &[], Some((60, 60))));
    assert!(keys(&quiet, WHITE_ACTIVE).is_empty());
    // (60, 60) widens to G3..G4: the C is the fourth of eight white keys.
    assert_eq!(keys(&quiet, WHITE).len(), 8);

    let playing = frame(&ctx, 0.1, vec![], |ui| {
        super::show(ui, &[60], Some((60, 60)))
    });
    let active = keys(&playing, WHITE_ACTIVE);
    assert_eq!(active, vec![keys(&quiet, WHITE)[3]]);
    assert_eq!(keys(&playing, WHITE).len(), 7);
}

#[test]
fn range_widens_to_an_octave_with_white_edges() {
    let ctx = context();
    let shapes = frame(&ctx, 0.0, vec![], |ui| super::show(ui, &[], Some((61, 63))));
    // A3..A4: 8 white keys and 5 black keys.
    assert_eq!(keys(&shapes, WHITE).len(), 8);
    assert_eq!(keys(&shapes, BLACK).len(), 5);
}

#[test]
fn hovering_shows_the_text_the_app_passes() {
    let ctx = context();
    let hover = crate::i18n::t("m034");
    let draw = |ui: &mut egui::Ui| super::show(ui, &[], Some((60, 60)));
    let tooltip = |shapes: &[Shape]| {
        shapes
            .iter()
            .any(|shape| matches!(shape, Shape::Text(text) if text.galley.text() == hover))
    };
    assert!(!tooltip(&frame(&ctx, 0.0, vec![], draw)));
    let mut shown = false;
    for step in 1..6 {
        let events = vec![Event::PointerMoved(egui::pos2(40.0, 40.0))];
        shown |= tooltip(&frame(&ctx, f64::from(step) * 0.1, events, draw));
    }
    assert!(shown, "{hover:?} is not drawn as a tooltip");
}

/// The scroll offset the DLL's `ScrollArea` stores is read back by the app's `egui`, so
/// both sides agree on the `TypeId` and the `Id` hashing.
#[test]
fn scroll_state_stored_by_the_dll_is_read_by_the_app() {
    type Probe = fn(&mut egui::Ui);
    let path = crate::resources::keyboard().unwrap();
    // SAFETY: Built in this workspace with the same `egui`, as in `super::load`.
    let library = unsafe { libloading::Library::new(&path) }
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let probe = *unsafe { library.get::<Probe>(b"repl_keyboard_probe_scroll\0") }.unwrap();

    let ctx = context();
    let drawn = std::cell::Cell::new(None);
    let mut draw = |ui: &mut egui::Ui| {
        drawn.set(Some(
            ui.make_persistent_id(egui::IdSalt::new("repl_keyboard_probe")),
        ));
        probe(ui);
    };
    frame(&ctx, 0.0, vec![], &mut draw);
    let id = drawn.get().unwrap();
    let state = egui::scroll_area::State::load(&ctx, id).expect("no scroll state under the id");
    assert_eq!(state.clamped_offset().y, 0.0);

    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -200.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &ctx,
        0.1,
        vec![Event::PointerMoved(egui::pos2(20.0, 50.0))],
        &mut draw,
    );
    frame(&ctx, 0.2, vec![wheel], &mut draw);
    for step in 3..30 {
        frame(&ctx, f64::from(step) * 0.1, vec![], &mut draw);
    }
    let scrolled = egui::scroll_area::State::load(&ctx, id)
        .unwrap()
        .clamped_offset()
        .y;
    assert!(scrolled > 0.0, "the wheel did not scroll: {scrolled}");

    frame(&ctx, 4.0, vec![], &mut draw);
    let kept = egui::scroll_area::State::load(&ctx, id)
        .unwrap()
        .clamped_offset()
        .y;
    assert_eq!(kept, scrolled);
}
