use super::*;
use egui::{Modifiers, PointerButton, RawInput, Shape};

fn button(x: f32, y: f32, pressed: bool) -> Event {
    Event::PointerButton {
        pos: Pos2::new(x, y),
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn render(ctx: &egui::Context, focused: bool, events: Vec<Event>) -> egui::FullOutput {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 100.0))),
        events,
        focused,
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                SURFACES
                    .with_borrow_mut(|surfaces| surfaces.get_mut("test").unwrap().draw(ui, true));
            });
    });
    // The headless renderer inspects shapes and has no texture backend.
    output.textures_delta.clear();
    output
}

fn frame(ctx: &egui::Context, focused: bool, events: Vec<Event>) -> Value {
    let output = render(ctx, focused, events);
    assert!(output
        .shapes
        .iter()
        .all(|s| !matches!(s.shape, Shape::Text(_))));
    poll("test")["events"].clone()
}

#[test]
fn header_updates_in_place_and_checkbox_click_stays_outside_the_pads() {
    register(&json!({"id":"test", "width":200, "height":100, "regions":[
        {"rect":[0, 0, 200, 100], "color":[40, 50, 60], "cursor":true}
    ], "header":{"checkbox":"auto gain", "checked":true}}))
    .unwrap();
    let ctx = egui::Context::default();
    assert_eq!(poll("test")["checked"], true);
    set_title("test", "Plugin / Patch".into(), None);
    let output = render(&ctx, true, vec![]);
    let locate = |label| {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap()
    };
    let title = locate("Plugin / Patch");
    let checkbox = locate("auto gain");
    assert!(title.x < checkbox.x);
    assert!((title.y - checkbox.y).abs() < 2.0);
    for pressed in [true, false] {
        render(
            &ctx,
            true,
            vec![
                Event::PointerMoved(checkbox),
                button(checkbox.x, checkbox.y, pressed),
            ],
        );
    }
    assert_eq!(
        poll("test"),
        json!({"open":true,"checked":false,"events":[]})
    );
}

#[test]
fn pad_gain_meter_tracks_the_selected_instance_and_checkbox() {
    unsafe extern "C" fn set(_: *const i32, _: usize) -> bool {
        true
    }
    unsafe extern "C" fn read(instance: i32) -> f32 {
        match instance {
            -2 => 0.5,
            3 => 1.0,
            7 => 2.0,
            _ => f32::NAN,
        }
    }
    crate::script_api::connect_pad_gain(Some(crate::script_api::PadGain { set, read }));
    register(&json!({"id":"test", "width":200, "height":100, "regions":[
        {"rect":[0, 0, 200, 100], "color":[40, 50, 60]}
    ], "header":{"checkbox":"auto gain", "checked":true, "padGainMeter":true}}))
    .unwrap();
    let ctx = egui::Context::default();
    let bar = |output: &egui::FullOutput, color| {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                Shape::Rect(shape) if shape.fill == color => Some(shape.rect),
                _ => None,
            })
    };
    let idle = render(&ctx, true, vec![]);
    assert!(bar(&idle, gain_meter::BOOST).is_none());
    assert!(bar(&idle, gain_meter::LIMIT).is_some());
    set_title("test", "First".into(), Some(3));
    let unity = render(&ctx, true, vec![]);
    assert_eq!(bar(&unity, gain_meter::BOOST).unwrap().width(), 2.0);
    set_title("test", "Second".into(), Some(7));
    let active = render(&ctx, true, vec![]);
    let boost = bar(&active, gain_meter::BOOST).unwrap();
    let limit = bar(&active, gain_meter::LIMIT).unwrap();
    let checkbox = active
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            Shape::Text(text) if text.galley.text() == "auto gain" => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap();
    assert!(boost.right() < checkbox.x && boost.bottom() < limit.top());
    assert!((boost.width() - limit.width()).abs() < 0.01);
    for pressed in [true, false] {
        render(
            &ctx,
            true,
            vec![
                Event::PointerMoved(checkbox),
                button(checkbox.x, checkbox.y, pressed),
            ],
        );
    }
    let disabled = render(&ctx, true, vec![]);
    assert_eq!(poll("test")["checked"], false);
    assert!(bar(&disabled, gain_meter::BOOST).is_none());
    assert!(bar(&disabled, gain_meter::LIMIT).is_none());
    crate::script_api::connect_pad_gain(None);
}

#[test]
fn colored_regions_capture_press_drag_and_release() {
    register(&json!({"id":"test", "width":200, "height":100, "regions":[
        {"rect":[0, 0, 100, 100], "color":[40, 50, 60], "cursor":true},
        {"rect":[110, 10, 60, 60], "color":[70, 80, 90]}
    ]}))
    .unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, true, vec![]);
    assert_eq!(
        frame(&ctx, true, vec![button(25., 25., true)]),
        json!([{"kind":"down","region":0,"x":0.25,"y":0.25}])
    );
    assert_eq!(
        frame(&ctx, true, vec![Event::PointerMoved(Pos2::new(140., 50.))]),
        json!([{"kind":"move","region":0,"x":1.0,"y":0.5}])
    );
    assert_eq!(
        frame(&ctx, false, vec![Event::WindowFocused(false)]),
        json!([{"kind":"up","region":0,"x":1.0,"y":0.5}])
    );
    assert_eq!(
        frame(
            &ctx,
            true,
            vec![button(140., 40., true), button(140., 40., false)]
        ),
        json!([{"kind":"down","region":1,"x":0.5,"y":0.5},{"kind":"up","region":1,"x":0.5,"y":0.5}])
    );
    close("test");
    assert_eq!(poll("test"), json!({"open":false,"events":[]}));
}

#[test]
fn hiding_a_surface_releases_its_held_region_once() {
    register(&json!({"id":"test", "width":200, "height":100, "regions":[
        {"rect":[0, 0, 200, 100], "color":[40, 50, 60], "cursor":true}
    ]}))
    .unwrap();
    let ctx = egui::Context::default();
    assert_eq!(
        frame(&ctx, true, vec![button(50., 25., true)]),
        json!([{"kind":"down","region":0,"x":0.25,"y":0.25}])
    );
    finish_frame();
    assert_eq!(poll("test")["events"], json!([]));
    finish_frame();
    assert_eq!(
        poll("test")["events"],
        json!([{"kind":"up","region":0,"x":0.25,"y":0.25}])
    );
    finish_frame();
    assert_eq!(poll("test")["events"], json!([]));
}

#[test]
fn secondary_press_hits_the_hovered_region_without_changing_primary_capture() {
    register(&json!({"id":"test", "width":200, "height":100, "regions":[
        {"rect":[0, 0, 100, 100], "color":[40, 50, 60], "cursor":true},
        {"rect":[110, 10, 60, 60], "color":[70, 80, 90]}
    ]}))
    .unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, true, vec![]);
    let secondary = |x, y, pressed| Event::PointerButton {
        pos: Pos2::new(x, y),
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(
        frame(
            &ctx,
            true,
            vec![secondary(25., 25., true), secondary(25., 25., false)]
        ),
        json!([{"kind":"secondary","region":0,"x":0.25,"y":0.25}])
    );
    assert_eq!(
        frame(
            &ctx,
            true,
            vec![
                button(25., 25., true),
                secondary(140., 40., true),
                secondary(140., 40., false),
                secondary(105., 40., true),
                secondary(105., 40., false),
                button(25., 25., false),
            ],
        ),
        json!([
            {"kind":"down","region":0,"x":0.25,"y":0.25},
            {"kind":"secondary","region":1,"x":0.5,"y":0.5},
            {"kind":"up","region":0,"x":0.25,"y":0.25}
        ])
    );
}

#[test]
fn windows_mouse_wheel_targets_the_hovered_region_during_primary_capture() {
    register(&json!({"id":"test", "width":200, "height":100, "regions":[
        {"rect":[0, 0, 100, 100], "color":[40, 50, 60]},
        {"rect":[110, 10, 60, 60], "color":[70, 80, 90]}
    ]}))
    .unwrap();
    let ctx = egui::Context::default();
    let wheel = |x, y| Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(x, y),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
        source: egui::MouseWheelSource::Unknown,
    };
    frame(&ctx, true, vec![button(25., 25., true)]);
    assert_eq!(
        frame(
            &ctx,
            true,
            vec![
                Event::PointerMoved(Pos2::new(140., 40.)),
                wheel(0., 1.),
                wheel(1., 0.)
            ]
        ),
        json!([{"kind":"wheel","region":1,"x":0.5,"y":0.5}])
    );
    assert_eq!(
        frame(
            &ctx,
            true,
            vec![
                wheel(0., -1.),
                Event::PointerMoved(Pos2::new(105., 50.)),
                wheel(0., -1.),
                button(105., 50., false)
            ]
        ),
        json!([
            {"kind":"wheel","region":1,"x":0.5,"y":0.5},
            {"kind":"up","region":0,"x":1.0,"y":0.5}
        ])
    );
}
