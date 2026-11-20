use super::projection::{Clip, Track, View};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind};

const KEYS: f32 = 44.0;
const SEMITONE: f32 = 10.0;
const HEADER: f32 = 26.0;
const RULER: f32 = 24.0;

pub fn pitch_name(pitch: u8) -> String {
    format!(
        "{}{}",
        ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"][pitch as usize % 12],
        pitch as i32 / 12 - 1
    )
}

pub fn show(ui: &mut egui::Ui, view: &View) {
    let id = ui.make_persistent_id("timeline_zoom");
    let mut zoom = ui.data_mut(|d| d.get_temp::<f32>(id));
    let fit = ((ui.available_width() - KEYS - 12.0).max(40.0) / view.end as f32).clamp(2.0, 160.0);
    ui.horizontal(|ui| {
        ui.heading(crate::i18n::t("m002"));
        if ui
            .small_button("−")
            .on_hover_text(crate::i18n::t("timeline_zoom_out"))
            .clicked()
        {
            zoom = Some((zoom.unwrap_or(fit) / 1.5).max(2.0));
        }
        if ui
            .small_button("+")
            .on_hover_text(crate::i18n::t("timeline_zoom_in"))
            .clicked()
        {
            zoom = Some((zoom.unwrap_or(fit) * 1.5).min(320.0));
        }
        if ui
            .small_button("↔")
            .on_hover_text(crate::i18n::t("timeline_fit"))
            .clicked()
        {
            zoom = None;
        }
    });
    ui.data_mut(|d| {
        if let Some(zoom) = zoom {
            d.insert_temp(id, zoom);
        } else {
            d.remove::<f32>(id);
        }
    });
    let scale = zoom.unwrap_or(fit);
    let width = (KEYS + view.end as f32 * scale).max(ui.available_width() - 12.0);
    let height = RULER
        + view
            .tracks
            .iter()
            .map(|t| HEADER + (t.high - t.low + 1) as f32 * SEMITONE + 8.0)
            .sum::<f32>();
    egui::ScrollArea::both()
        .id_salt("piano_rolls")
        .max_height(ui.available_height())
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
            let pinned_left = rect.left().max(ui.clip_rect().left());
            let plot_clip = rect
                .intersect(ui.clip_rect())
                .with_min_x(pinned_left + KEYS);
            let plot = painter.with_clip_rect(plot_clip);
            let x = |beat: f64| rect.left() + KEYS + beat as f32 * scale;
            let mut top = rect.top() + RULER;
            for track in &view.tracks {
                let h = (track.high - track.low + 1) as f32 * SEMITONE;
                let roll = Rect::from_min_size(
                    egui::pos2(x(0.0), top + HEADER),
                    egui::vec2(width - KEYS, h),
                );
                draw_track(&plot, track, roll, &x, &response);
                let keys = Rect::from_min_max(
                    egui::pos2(pinned_left, roll.top()),
                    egui::pos2(pinned_left + KEYS, roll.bottom()),
                );
                draw_keys(&painter, track, keys);
                let header = Rect::from_min_max(
                    egui::pos2(pinned_left, top),
                    egui::pos2(rect.right(), top + HEADER),
                );
                let background = ui.visuals().extreme_bg_color;
                painter.rect_filled(header, 0.0, background);
                let name = if track.sounding.is_some() || track.plugins.is_empty() {
                    format!("Track {}", track.index)
                } else {
                    let separator = if track.plugins_serial { " → " } else { ", " };
                    format!("Track {} · {}", track.index, track.plugins.join(separator))
                };
                let drawn = painter.text(
                    header.left_center() + egui::vec2(4.0, 0.0),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(12.0),
                    ui.visuals().text_color(),
                );
                if let Some(sounding) = &track.sounding {
                    painter.text(
                        egui::pos2(drawn.right(), header.center().y),
                        Align2::LEFT_CENTER,
                        format!(" · {}", sounding.label),
                        FontId::proportional(12.0),
                        label_color(
                            background,
                            ui.visuals().text_color(),
                            sounding.gain,
                            sounding.fading_out,
                        ),
                    );
                }
                // Stable identity for hover interactions, independent of vector index changes.
                ui.interact(
                    header,
                    ui.scope_id().with((&track.id, "header")),
                    Sense::hover(),
                )
                .on_hover_text(format!("Track {} · {}", track.index, track.id));
                top = roll.bottom() + 8.0;
            }
            let visible = rect.intersect(ui.clip_rect());
            // Draw the ruler last so that it stays visible when scrolling vertically.
            let ruler_top = visible.top();
            let ruler = Rect::from_min_max(
                egui::pos2(pinned_left, ruler_top),
                egui::pos2(rect.right(), ruler_top + RULER),
            );
            painter.rect_filled(ruler, 0.0, ui.visuals().extreme_bg_color);
            draw_grid(&plot, view, rect, ruler_top, scale, &x);
            if let Some(end) = view.loop_end {
                for beat in [view.loop_start.unwrap_or(0.0), end] {
                    plot.line_segment(
                        [
                            egui::pos2(x(beat), ruler_top + 3.0),
                            egui::pos2(x(beat), rect.bottom()),
                        ],
                        Stroke::new(1.5, Color32::from_rgb(62, 180, 142)),
                    );
                }
            }
            if view.playing {
                let px = x(view.playhead);
                plot.line_segment(
                    [
                        egui::pos2(px, ruler_top + RULER),
                        egui::pos2(px, rect.bottom()),
                    ],
                    Stroke::new(1.5, Color32::from_rgb(255, 201, 85)),
                );
                plot.add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(px - 4.0, ruler_top + RULER - 6.0),
                        egui::pos2(px + 4.0, ruler_top + RULER - 6.0),
                        egui::pos2(px, ruler_top + RULER),
                    ],
                    Color32::from_rgb(255, 201, 85),
                    Stroke::NONE,
                ));
            }
        });
}

fn draw_keys(painter: &egui::Painter, track: &Track, rect: Rect) {
    for pitch in track.low..=track.high {
        let y = rect.top() + (track.high - pitch) as f32 * SEMITONE;
        let row = Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(KEYS, SEMITONE));
        let black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
        painter.rect_filled(
            row.shrink(0.5),
            0.0,
            Color32::from_gray(if black { 42 } else { 188 }),
        );
        if pitch % 12 == 0 {
            painter.text(
                row.right_center() - egui::vec2(4.0, 0.0),
                Align2::RIGHT_CENTER,
                pitch_name(pitch),
                FontId::monospace(9.0),
                Color32::from_gray(25),
            );
        }
    }
}

/// Above 1 a fading-out label darkens early and any other label brightens early.
const BRIGHTNESS_EXPONENT: f32 = 2.0;

/// The background at gain 0 and `text` at gain 1. Between, `gain^e` while fading
/// out and `1 - (1 - gain)^e` otherwise.
fn label_color(background: Color32, text: Color32, gain: f32, fading_out: bool) -> Color32 {
    let gain = gain.clamp(0.0, 1.0);
    let level = if fading_out {
        gain.powf(BRIGHTNESS_EXPONENT)
    } else {
        1.0 - (1.0 - gain).powf(BRIGHTNESS_EXPONENT)
    };
    background.lerp_to_gamma(text, level)
}

fn color(clip: &Clip) -> Color32 {
    if clip.muted {
        Color32::from_gray(110)
    } else {
        Color32::from_rgb(78, 156, 222)
    }
}

fn draw_track(
    painter: &egui::Painter,
    track: &Track,
    rect: Rect,
    x: &impl Fn(f64) -> f32,
    response: &egui::Response,
) {
    if !rect.intersects(painter.clip_rect()) {
        return;
    }
    for pitch in track.low..=track.high {
        let y = rect.top() + (track.high - pitch) as f32 * SEMITONE;
        let row = Rect::from_min_size(
            egui::pos2(rect.left(), y),
            egui::vec2(rect.width(), SEMITONE),
        );
        painter.rect_filled(
            row,
            0.0,
            Color32::from_gray(if matches!(pitch % 12, 1 | 3 | 6 | 8 | 10) {
                24
            } else {
                32
            }),
        );
        if pitch % 12 == 0 {
            painter.hline(
                rect.x_range(),
                y + SEMITONE,
                Stroke::new(0.5, Color32::from_gray(68)),
            );
        }
    }
    let mut hover = None;
    for clip in &track.clips {
        let window = Rect::from_min_max(
            egui::pos2(x(clip.start), rect.top()),
            egui::pos2(x(clip.end), rect.bottom()),
        );
        let tint = color(clip);
        painter.rect_stroke(
            window,
            2.0,
            Stroke::new(1.0, tint.gamma_multiply(0.65)),
            StrokeKind::Inside,
        );
        for note in &clip.notes {
            let y = rect.top() + (track.high - note.pitch) as f32 * SEMITONE;
            let bar = Rect::from_min_max(
                egui::pos2(x(note.start), y + 1.0),
                egui::pos2(x(note.end).max(x(note.start) + 1.5), y + SEMITONE - 1.0),
            )
            .intersect(window);
            let fill = if note.open {
                Color32::from_rgb(190, 139, 61)
            } else {
                tint
            };
            painter.rect_filled(
                bar,
                1.5,
                fill.gamma_multiply(if clip.muted { 0.45 } else { 1.0 }),
            );
            if response
                .hover_pos()
                .is_some_and(|p| bar.contains(p) && painter.clip_rect().contains(p))
            {
                painter.rect_stroke(
                    bar,
                    1.5,
                    Stroke::new(1.0, Color32::WHITE),
                    StrokeKind::Inside,
                );
                hover = Some(crate::i18n::render(
                    "timeline_note",
                    &[
                        ("pitch", pitch_name(note.pitch)),
                        ("group", note.group.to_string()),
                        ("channel", note.channel.to_string()),
                        ("start", format!("{:.3}", note.start)),
                        ("end", format!("{:.3}", note.end)),
                        ("clip", clip.label.clone()),
                    ],
                ));
            }
        }
    }
    if let Some(text) = hover {
        response.clone().on_hover_text(text);
    }
}

fn draw_grid(
    painter: &egui::Painter,
    view: &View,
    rect: Rect,
    ruler_top: f32,
    scale: f32,
    x: &impl Fn(f64) -> f32,
) {
    let clip = painter.clip_rect();
    let first = ((clip.left() - rect.left() - KEYS) / scale).max(0.0) as f64;
    let last = ((clip.right() - rect.left() - KEYS) / scale) as f64;
    let mut bar_number = 1;
    for (index, signature) in view.signatures.iter().enumerate() {
        let bar_len = signature.numerator as f64 * 4.0 / signature.denominator as f64;
        let beat_len = 4.0 / signature.denominator as f64;
        let end = view.signatures.get(index + 1).map_or(view.end, |s| s.start);
        let count = ((end - signature.start) / bar_len).ceil().max(0.0) as usize;
        let start_bar = ((first - signature.start) / bar_len).floor().max(0.0) as usize;
        let last_bar = (((last.min(end) - signature.start) / bar_len)
            .ceil()
            .max(0.0) as usize)
            .min(count);
        let stride = (46.0 / (bar_len as f32 * scale)).ceil().max(1.0) as usize;
        for bar in start_bar..=last_bar {
            if bar % stride != 0 {
                continue;
            }
            let beat = signature.start + bar as f64 * bar_len;
            if beat > end || beat > view.end {
                continue;
            }
            painter.vline(
                x(beat),
                (ruler_top + RULER)..=rect.bottom(),
                Stroke::new(0.6, Color32::from_white_alpha(45)),
            );
            painter.text(
                egui::pos2(x(beat) + 4.0, ruler_top + 5.0),
                Align2::LEFT_TOP,
                (bar_number + bar).to_string(),
                FontId::monospace(11.0),
                painter.ctx().global_style().visuals.text_color(),
            );
            if beat_len as f32 * scale >= 12.0 {
                for step in 1..signature.numerator {
                    let pos = beat + step as f64 * beat_len;
                    if pos >= end {
                        break;
                    }
                    painter.vline(
                        x(pos),
                        (ruler_top + RULER)..=rect.bottom(),
                        Stroke::new(0.5, Color32::from_white_alpha(16)),
                    );
                }
            }
        }
        bar_number += count;
    }
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
