use eframe::egui::{self, Align, Align2, Color32, FontId, Rect, Sense};

const FLOOR_DB: f32 = -60.0;
const WIDTH: f32 = 200.0;

fn fraction(db: f32) -> f32 {
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

pub(super) fn show(ctx: &egui::Context) {
    if !crate::script_ui::recording_monitor_open() {
        return;
    }
    let (recording, input) = crate::recording::monitor_level();
    show_level(ctx, recording, input);
}

fn show_level(ctx: &egui::Context, recording: bool, input: Option<f32>) {
    let id = egui::Id::unique("recording_monitor");
    let dt = ctx.input(|input| input.stable_dt);
    let db = ctx.data_mut(|data| {
        let previous = data.get_temp::<f32>(id).unwrap_or(FLOOR_DB);
        let next = input.map_or(FLOOR_DB, |peak| {
            (20.0 * peak.log10())
                .max(previous - 48.0 * dt)
                .clamp(FLOOR_DB, 0.0)
        });
        data.insert_temp(id, next);
        next
    });
    egui::Area::new(id)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(WIDTH);
                ui.horizontal(|ui| {
                    let color = if recording {
                        Color32::from_rgb(242, 73, 85)
                    } else {
                        ui.visuals().weak_text_color()
                    };
                    let (dot, _) = ui.allocate_exact_size(egui::vec2(12.0, 15.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.5, color);
                    ui.label(crate::i18n::t("recording_monitor_input"));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
                            crate::script_ui::close_recording_monitor();
                        }
                    });
                });
                meter(ui, db);
            });
        });
}

fn meter(ui: &mut egui::Ui, db: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::hover());
    let painter = ui.painter();
    let bar = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 10.0));
    painter.rect_filled(bar, 2.0, ui.visuals().extreme_bg_color);
    let x = |db| egui::lerp(bar.x_range(), fraction(db));
    for (low, high, color) in [
        (FLOOR_DB, -12.0, Color32::from_rgb(65, 205, 154)),
        (-12.0, -3.0, Color32::from_rgb(241, 191, 82)),
        (-3.0, 0.0, Color32::from_rgb(242, 73, 85)),
    ] {
        if db > low {
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(x(low), bar.top()),
                    egui::pos2(x(db.min(high)), bar.bottom()),
                ),
                0.0,
                color,
            );
        }
    }
    let font = FontId::monospace(10.0);
    let color = ui.visuals().text_color();
    for (label, tick, align) in [
        ("−60", FLOOR_DB, Align2::LEFT_TOP),
        ("−30", -30.0, Align2::CENTER_TOP),
        ("0 dBFS", 0.0, Align2::RIGHT_TOP),
    ] {
        painter.text(
            egui::pos2(x(tick), bar.bottom() + 4.0),
            align,
            label,
            font.clone(),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_is_centered_and_paints_live_input_before_recording() {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let frame = |peak| {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |_| show_level(&ctx, false, Some(peak)),
            );
            let width = output
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Rect(shape) if shape.fill == Color32::from_rgb(65, 205, 154) => {
                        Some(shape.rect.width())
                    }
                    _ => None,
                });
            output.drop_without_applying_deltas();
            width
        };
        for _ in 0..20 {
            frame(0.0);
        }
        let area = ctx
            .memory(|memory| memory.area_rect(egui::Id::unique("recording_monitor")))
            .unwrap();
        assert!(area.center().distance(screen.center()) < 1.0, "{area:?}");
        let quiet = frame(0.01).expect("quiet input should be visible");
        let loud = frame(0.2).expect("loud input should be visible");
        assert!(loud > quiet, "{quiet} to {loud}");
    }
}
