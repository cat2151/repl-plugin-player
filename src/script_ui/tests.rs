use super::*;
use serde_json::json;

fn window() -> Value {
    json!({"id":"main", "title":"User title", "widgets":[
        {"id":"a","type":"slider","label":"User label","binding":{"instanceId":7,"parameterId":3}}
    ]})
}

#[test]
fn callbacks_only_register_valid_definitions_and_same_id_updates() {
    enable(false);
    assert!(dispatch("ui.show", &json!({"definition":window()}))
        .unwrap_err()
        .contains("GUI"));
    enable(true);
    dispatch("ui.show", &json!({"definition":window()})).unwrap();
    let mut update = window();
    update["title"] = json!("Updated");
    dispatch("ui.show", &json!({"definition":update})).unwrap();
    assert_eq!(snapshot().len(), 1);
    assert_eq!(snapshot()[0].title, "Updated");
    dispatch("ui.close", &json!({"id":"main"})).unwrap();
    assert!(snapshot().is_empty());
    dispatch("ui.show", &json!({"definition":window()})).unwrap();
    assert_eq!(snapshot().len(), 1);
    for value in [
        json!(null),
        json!({"id":"bad"}),
        json!({"id":"x","title":"y","widgets":[{"id":"a","type":"slider","label":"z","binding":{"instanceId":-1,"parameterId":0}}]}),
    ] {
        assert!(dispatch("ui.show", &json!({"definition":value})).is_err());
    }
    assert_eq!(snapshot().len(), 1); // Atomic validation, no partial window.
    enable(false);
    assert!(snapshot().is_empty());
}

struct Fake {
    fingerprint: Value,
    value: f64,
    writes: Vec<(i32, i32, f64)>,
    missing: bool,
    readonly: bool,
}
impl Parameters for Fake {
    fn parameter(&mut self, _: &Binding) -> Result<Parameter, String> {
        if self.missing {
            return Err("disposed".into());
        }
        Ok(Parameter {
            fingerprint: self.fingerprint.clone(),
            min: 0.0,
            max: 1.0,
            readonly: self.readonly,
            discrete: false,
            value: self.value,
        })
    }
    fn set(&mut self, b: &Binding, value: f64) -> Result<(), String> {
        self.writes.push((b.instance, b.parameter, value));
        self.value = value;
        Ok(())
    }
}

#[test]
fn frames_preserve_windows_external_values_refresh_and_metadata_change_disables() {
    enable(true);
    dispatch("ui.show", &json!({"definition":window()})).unwrap();
    let ctx = eframe::egui::Context::default();
    let mut renderer = Renderer::default();
    let mut fake = Fake {
        fingerprint: json!("stable-parameter"),
        value: 0.2,
        writes: vec![],
        missing: false,
        readonly: false,
    };
    for _ in 0..3 {
        regression::frame(&ctx, &mut renderer, &mut fake, vec![]);
    }
    assert_eq!(snapshot().len(), 1);
    assert!(fake.writes.is_empty());
    fake.value = 0.7;
    let w = snapshot().remove(0);
    assert_eq!(
        renderer
            .test_read(&w, &w.widgets[0], &mut fake)
            .unwrap()
            .value,
        0.7
    );
    fake.fingerprint = json!("different-parameter-at-same-index");
    assert!(renderer.test_read(&w, &w.widgets[0], &mut fake).is_err());
    dispatch("ui.show", &json!({"definition":window()})).unwrap();
    let w = snapshot().remove(0);
    assert!(renderer.test_read(&w, &w.widgets[0], &mut fake).is_ok());
    fake.missing = true;
    assert!(renderer.test_read(&w, &w.widgets[0], &mut fake).is_err());
    enable(false);
}

#[test]
fn egui_input_writes_only_bound_parameter_and_readonly_never_writes() {
    use eframe::egui::{Event, Modifiers, PointerButton};
    enable(true);
    dispatch("ui.show", &json!({"definition":window()})).unwrap();
    let ctx = eframe::egui::Context::default();
    let mut renderer = Renderer::default();
    let mut fake = Fake {
        fingerprint: json!("stable"),
        value: 0.1,
        writes: vec![],
        missing: false,
        readonly: false,
    };
    let mut rect = None;
    for _ in 0..5 {
        rect = regression::frame(&ctx, &mut renderer, &mut fake, vec![]);
    }
    let rect = rect.unwrap();
    let position = eframe::egui::pos2(rect.left() + 65.0, rect.center().y);
    for pressed in [true, false] {
        regression::frame(
            &ctx,
            &mut renderer,
            &mut fake,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    assert!(!fake.writes.is_empty());
    assert!(fake
        .writes
        .iter()
        .all(|(instance, parameter, value)| *instance == 7
            && *parameter == 3
            && value.is_finite()
            && (0.0..=1.0).contains(value)));
    fake.readonly = true;
    let count = fake.writes.len();
    assert!(regression::frame(&ctx, &mut renderer, &mut fake, vec![]).is_none());
    assert_eq!(fake.writes.len(), count);
    fake.readonly = false;
    fake.fingerprint = json!("new-metadata-generation");
    assert!(regression::frame(&ctx, &mut renderer, &mut fake, vec![]).is_none());
    fake.missing = true;
    assert!(regression::frame(&ctx, &mut renderer, &mut fake, vec![]).is_none());
    assert_eq!(fake.writes.len(), count);
    enable(false);
}

#[test]
fn dialogs_wait_in_the_order_given_and_need_the_gui() {
    enable(false);
    assert!(dispatch("ui.dialog", &json!({"title":"t","text":"a"})).is_err());
    enable(true);
    dispatch("ui.dialog", &json!({"title":"t","text":"a"})).unwrap();
    dispatch("ui.dialog", &json!({"title":"t","text":"b"})).unwrap();
    assert!(dispatch("ui.dialog", &json!({"title":"t"})).is_err());
    assert_eq!(take_dialog().map(|d| d.text).as_deref(), Some("a"));
    assert_eq!(take_dialog().map(|d| d.text).as_deref(), Some("b"));
    assert_eq!(take_dialog(), None);
    enable(false);
}
