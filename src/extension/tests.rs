//! Headless frames that draw through `builtin.dll`, so a missing DLL fails them.
use super::Extensions;
use eframe::egui::{self, epaint::Shape, Color32, Event, Pos2, Rect};
use repl_extension_api::Frame;

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

/// Draws `builtin/keyboard` through the app's loader.
fn keyboard<'a>(
    extensions: &'a mut Extensions,
    active: &[u8],
    range: Option<(u8, u8)>,
    language: &str,
) -> impl FnMut(&mut egui::Ui) + 'a {
    let active = active.to_vec();
    let language = language.to_owned();
    move |ui| {
        let frame = Frame {
            active_notes: &active,
            range,
            language: &language,
        };
        extensions.show("builtin/keyboard", ui, &frame);
    }
}

#[test]
fn builtin_dll_lists_keyboard() {
    let list = super::load("builtin").unwrap();
    let names: Vec<_> = list.iter().map(|extension| extension.name).collect();
    assert_eq!(names, ["keyboard"]);
}

#[test]
fn active_pitch_is_drawn_in_the_active_color() {
    let ctx = context();
    let mut extensions = Extensions::default();
    let quiet = frame(
        &ctx,
        0.0,
        vec![],
        keyboard(&mut extensions, &[], Some((60, 60)), "en"),
    );
    assert!(keys(&quiet, WHITE_ACTIVE).is_empty());
    // (60, 60) widens to G3..G4: the C is the fourth of eight white keys.
    assert_eq!(keys(&quiet, WHITE).len(), 8);

    let playing = frame(
        &ctx,
        0.1,
        vec![],
        keyboard(&mut extensions, &[60], Some((60, 60)), "en"),
    );
    let active = keys(&playing, WHITE_ACTIVE);
    assert_eq!(active, vec![keys(&quiet, WHITE)[3]]);
    assert_eq!(keys(&playing, WHITE).len(), 7);
}

#[test]
fn range_widens_to_an_octave_with_white_edges() {
    let ctx = context();
    let mut extensions = Extensions::default();
    let shapes = frame(
        &ctx,
        0.0,
        vec![],
        keyboard(&mut extensions, &[], Some((61, 63)), "en"),
    );
    // A3..A4: 8 white keys and 5 black keys.
    assert_eq!(keys(&shapes, WHITE).len(), 8);
    assert_eq!(keys(&shapes, BLACK).len(), 5);
}

/// The texts drawn after hovering that were not drawn before.
fn tooltip(language: &str) -> Vec<String> {
    let ctx = context();
    let mut extensions = Extensions::default();
    let mut draw = keyboard(&mut extensions, &[], Some((60, 60)), language);
    let texts = |shapes: Vec<Shape>| -> Vec<String> {
        shapes
            .into_iter()
            .filter_map(|shape| match shape {
                Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    };
    let before = texts(frame(&ctx, 0.0, vec![], &mut draw));
    let mut shown = vec![];
    for step in 1..6 {
        let events = vec![Event::PointerMoved(egui::pos2(40.0, 40.0))];
        for text in texts(frame(&ctx, f64::from(step) * 0.1, events, &mut draw)) {
            if !before.contains(&text) && !shown.contains(&text) {
                shown.push(text);
            }
        }
    }
    shown
}

/// The DLL holds its own tooltip text and picks it by `Frame::language`.
#[test]
fn hovering_shows_a_tooltip_in_the_app_language() {
    let en = tooltip("en");
    let ja = tooltip("ja");
    println!("en {en:?} ja {ja:?}");
    assert_eq!(en.len(), 1, "{en:?}");
    assert_eq!(ja.len(), 1, "{ja:?}");
    let kana = |text: &str| text.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c));
    assert!(!kana(&en[0]) && kana(&ja[0]), "{en:?} {ja:?}");
}

/// The scroll offset the DLL's `ScrollArea` stores is read back by the app's `egui`, so
/// both sides agree on the `TypeId` and the `Id` hashing.
#[test]
fn scroll_state_stored_by_the_dll_is_read_by_the_app() {
    type Probe = fn(&mut egui::Ui);
    let path = crate::resources::extension("builtin").unwrap();
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

/// Every directory in `extensions/`, a cloned repository's included, loads its DLL and
/// draws each of its panes in one frame.
#[test]
fn every_extension_in_the_workspace_loads_and_draws() {
    let dlls = crate::resources::extension_names();
    println!("dlls {dlls:?}");
    assert!(!dlls.is_empty());
    for dll in &dlls {
        let mut extensions = Extensions::default();
        let list = extensions
            .list(dll)
            .unwrap_or_else(|error| panic!("{dll}: {error}"));
        assert!(!list.is_empty(), "{dll} has no pane");
        for extension in list {
            let name = format!("{dll}/{}", extension.name);
            let ctx = context();
            let frame_data = Frame {
                active_notes: &[60],
                range: Some((60, 60)),
                language: "en",
            };
            let shapes = frame(&ctx, 0.0, vec![], |ui| {
                extensions.show(&name, ui, &frame_data);
            });
            println!("{name}: {} shapes", shapes.len());
            assert!(!shapes.is_empty(), "{name} drew nothing");
        }
    }
}

/// A pane the DLL does not have is replaced by the panes it has.
#[test]
fn missing_pane_names_the_panes_the_dll_has() {
    let ctx = context();
    let mut extensions = Extensions::default();
    let frame_data = Frame {
        active_notes: &[],
        range: None,
        language: "en",
    };
    let shapes = frame(&ctx, 0.0, vec![], |ui| {
        extensions.show("builtin/nothing", ui, &frame_data);
    });
    let texts: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    println!("{texts:?}");
    assert!(
        texts
            .iter()
            .any(|text| text.contains("nothing") && text.contains("keyboard")),
        "{texts:?}"
    );
}
