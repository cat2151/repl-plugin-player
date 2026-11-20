//! Silent real-plugin acceptance through the same egui/control path as the GUI.
//! The test-owned cells are `tests/e2e/script-ui.js`.
use super::*;
use crate::{control::Control, e2e_script::Script, script_editor::ScriptEditor, snippets::Snippet};
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect};

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    enable(true);
    let result = run_enabled(app, script);
    enable(false);
    result
}

pub(super) fn frame(
    ctx: &egui::Context,
    renderer: &mut Renderer,
    app: &mut impl Parameters,
    events: Vec<Event>,
) -> Option<Rect> {
    let mut rect = None;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 800.0))),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        let windows = snapshot();
        renderer.retain(&windows);
        egui::CentralPanel::default().show(ui, |ui| {
            for window in &windows {
                renderer.contents(ui, window, app, |widget, response| {
                    if widget.kind == "slider" {
                        rect = Some(response.rect);
                    }
                });
            }
        });
    })
    .drop_without_applying_deltas();
    rect
}

fn run_enabled(app: &mut Control, script: &Script) -> Result<(), String> {
    let open = &Snippet {
        name: "open".into(),
        group: "User",
        keywords: String::new(),
        code: script.cell("open")?.into(),
    };
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut document = script.cell("unrelated")?.to_owned();
    let request = editor.insert_snippet(&ctx, &mut document, open);
    app.run_request(&request)?;
    let window = snapshot()
        .pop()
        .ok_or("snippet did not register a window")?;
    let widget = window
        .widgets
        .iter()
        .find(|w| w.kind == "slider")
        .ok_or("missing slider")?;
    let binding = widget.binding.as_ref().unwrap();
    let original = app.parameter(binding)?;
    if original.readonly || original.discrete || original.fingerprint["epoch"].as_u64().is_none() {
        return Err("real writable CLAP metadata not captured".into());
    }
    let result = (|| {
        let mut renderer = Renderer::default();
        let mut rect = None;
        for _ in 0..5 {
            rect = frame(&ctx, &mut renderer, app, vec![]);
        }
        let rect = rect.ok_or("real slider disabled")?;
        // egui Slider includes the numeric editor and label; its rail starts left.
        let position = egui::pos2(rect.left() + 65.0, rect.center().y);
        frame(
            &ctx,
            &mut renderer,
            app,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            &ctx,
            &mut renderer,
            app,
            vec![Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let changed = app.parameter(binding)?;
        if (changed.value - original.value).abs() < 1e-8 {
            return Err("egui pointer did not change actual Surge parameter".into());
        }
        let external = original.min + (original.max - original.min) * 0.23;
        let set = serde_json::json!({"instance":binding.instance,
            "parameter":binding.parameter, "value":external});
        app.eval(
            &script.call("set parameter", &set)?,
            "external parameter change",
        )?;
        frame(&ctx, &mut renderer, app, vec![]);
        let observed = renderer.test_parameter(&window, widget, app)?;
        if (observed.value - external).abs() > 1e-5 {
            return Err("external change did not reach script UI".into());
        }
        // Script ends, Stop and closing the script window never dispose the plugin.
        app.stop("script UI Stop acceptance")?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.stopping {
            app.poll()?;
            if std::time::Instant::now() > deadline {
                return Err("Stop did not settle".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if snapshot().len() != 1 {
            return Err("Stop closed script window".into());
        }
        app.run(script.cell("close")?)?;
        if !snapshot().is_empty() {
            return Err("close snippet failed".into());
        }
        app.parameter(binding)?;
        app.run(&open.code)?;
        app.run(&open.code)?;
        if snapshot().len() != 1 {
            return Err("same ID duplicated windows".into());
        }
        app.log.event("script_ui_acceptance", serde_json::json!({"instance":binding.instance,"parameter":binding.parameter,"original":original.value,"pointerValue":changed.value,"externalValue":observed.value,"metadata":original.fingerprint}))?;
        Ok(())
    })();
    let restored = app.set(binding, original.value).and_then(|_| {
        let value = app.parameter(binding)?.value;
        if (value - original.value).abs() > 1e-5 {
            Err("original Surge value restoration failed".into())
        } else {
            Ok(())
        }
    });
    result.and(restored)
}
