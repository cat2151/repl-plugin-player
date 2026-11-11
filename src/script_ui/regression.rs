//! Silent real-plugin acceptance through the same egui/control path as the GUI.
use super::*;
use crate::{control::Control, script_editor::ScriptEditor, snippets::Snippet};
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect};

pub fn run(app: &mut Control) -> Result<(), String> {
    enable(true);
    let result = run_enabled(app);
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
    ctx.run_ui(input, |_| {
        renderer.show_observed(ctx, app, |widget, response| {
            if widget.kind == "slider" {
                rect = Some(response.rect);
            }
        });
    })
    .drop_without_applying_deltas();
    rect
}

// Test-owned scripts: one slider bound to a writable continuous parameter, and its close.
const OPEN: &str = "(() => {
    var id = globalThis.surgeId;
    var parameter = uapmd.instance(id).getParameters().find(p => !p.isReadonly && !p.isDiscrete &&
        Number.isInteger(p.id) && p.id >= 0 && Number.isFinite(p.minValue) && Number.isFinite(p.maxValue) &&
        p.minValue < p.maxValue);
    if (!parameter) throw new Error('E2E: no writable continuous parameter');
    repl.ui.show({id:'e2e-parameters', title:'E2E', widgets:[
        {id:'parameter', type:'slider', label:parameter.name, binding:{instanceId:id, parameterId:parameter.id}}
    ]});
})();";
const CLOSE: &str = "repl.ui.close('e2e-parameters');";

fn run_enabled(app: &mut Control) -> Result<(), String> {
    let open = &Snippet {
        name: "open".into(),
        group: "User",
        code: OPEN.into(),
    };
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut document = "throw new Error('unrelated cell');".to_owned();
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
        app.eval(
            &format!(
                "uapmd.instance({}).setParameterValue({}, {})",
                binding.instance, binding.parameter, external
            ),
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
        app.run(CLOSE)?;
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
