use crate::control::RunResult;
use crate::syntax_highlight::BACKGROUND;
use eframe::egui::{self, text::LayoutJob, Color32, FontId, TextFormat};

const FOREGROUND: Color32 = Color32::from_rgb(0xf8, 0xf8, 0xf2);
const COMMENT: Color32 = Color32::from_rgb(0x75, 0x71, 0x5e);
const STRING: Color32 = Color32::from_rgb(0xe6, 0xdb, 0x74);
const CONSTANT: Color32 = Color32::from_rgb(0xae, 0x81, 0xff);
const ERROR: Color32 = Color32::from_rgb(0xf9, 0x26, 0x72);

/// Every run's result, readable and selectable in full, like a browser's developer
/// console.
pub(super) fn show(ui: &mut egui::Ui, results: &[RunResult]) {
    let job = layout(results, egui::TextStyle::Monospace.resolve(ui.style()));
    // A `&str` buffer keeps the text read-only while still selectable and copyable.
    let mut view = job.text.as_str();
    let mut layouter = |ui: &egui::Ui, _: &dyn egui::TextBuffer, wrap_width: f32| {
        let mut job = job.clone();
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    egui::Frame::new()
        .fill(BACKGROUND)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(4.0)
        .show(ui, |ui| {
            ui.visuals_mut().selection.bg_fill = Color32::from_rgb(0x49, 0x48, 0x3e);
            ui.visuals_mut().selection.stroke.color = FOREGROUND;
            egui::ScrollArea::both()
                .stick_to_bottom(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut view)
                            .code_editor()
                            .frame(egui::Frame::NONE)
                            .layouter(&mut layouter)
                            .desired_width(f32::INFINITY),
                    );
                });
        });
}

/// The copied text is each result as `header body`, separated by a blank line.
fn layout(results: &[RunResult], font: FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let put = |job: &mut LayoutJob, text: &str, color| {
        job.append(text, 0.0, TextFormat::simple(font.clone(), color));
    };
    for (index, result) in results.iter().enumerate() {
        if index > 0 {
            put(&mut job, "\n\n", FOREGROUND);
        }
        let header = if result.error { ERROR } else { COMMENT };
        put(&mut job, &result.header, header);
        put(&mut job, " ", FOREGROUND);
        for (index, line) in result.body.split('\n').enumerate() {
            if index > 0 {
                put(&mut job, "\n", FOREGROUND);
            }
            for (text, color) in tokens(line) {
                put(&mut job, text, color);
            }
        }
    }
    job
}

/// Colors a body line the way a log viewer does: stack frames dimmed, and strings,
/// numbers, literals and error names picked out.
fn tokens(line: &str) -> Vec<(&str, Color32)> {
    if line.trim_start().starts_with("at ") {
        return vec![(line, COMMENT)];
    }
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(first) = rest.chars().next() {
        let (length, color) = if first == '"' || first == '\'' {
            let end = rest[1..].find(first).map_or(rest.len(), |i| i + 2);
            (end, STRING)
        } else if first.is_alphanumeric() || first == '_' || first == '.' {
            let length = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
                .unwrap_or(rest.len());
            (length, word_color(&rest[..length]))
        } else {
            (first.len_utf8(), FOREGROUND)
        };
        out.push((&rest[..length], color));
        rest = &rest[length..];
    }
    out
}

fn word_color(word: &str) -> Color32 {
    let numeric = word.starts_with(|c: char| c.is_ascii_digit())
        && word.chars().all(|c| c.is_ascii_digit() || c == '.');
    if numeric
        || matches!(
            word,
            "true" | "false" | "null" | "undefined" | "NaN" | "Infinity"
        )
    {
        CONSTANT
    } else if word.ends_with("Error") {
        ERROR
    } else {
        FOREGROUND
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_of(job: &LayoutJob, token: &str) -> Color32 {
        let index = job.text.find(token).unwrap();
        let section = job
            .sections
            .iter()
            .find(|s| s.byte_range.contains(&index.into()));
        section.unwrap().format.color
    }

    #[test]
    fn results_read_as_a_colored_log_and_copy_as_plain_lines() {
        let results = [
            RunResult {
                error: false,
                header: "cell · line 1 · Result:".into(),
                body: "{\"a\": 12.5, \"b\": [true, null]}\nname 音".into(),
            },
            RunResult {
                error: true,
                header: "cell · line 4 · Script error:".into(),
                body: "ReferenceError: 'x' is not defined\n    at <eval> ()".into(),
            },
        ];
        let job = layout(&results, FontId::monospace(14.0));
        assert_eq!(
            job.text,
            "cell · line 1 · Result: {\"a\": 12.5, \"b\": [true, null]}\nname 音\n\n\
             cell · line 4 · Script error: ReferenceError: 'x' is not defined\n    at <eval> ()"
        );
        for (token, color) in [
            ("line 1", COMMENT),
            ("\"a\"", STRING),
            ("12.5", CONSTANT),
            ("true", CONSTANT),
            ("null", CONSTANT),
            ("name", FOREGROUND),
            ("line 4", ERROR),
            ("ReferenceError", ERROR),
            ("'x'", STRING),
            ("at <eval>", COMMENT),
        ] {
            assert_eq!(color_of(&job, token), color, "{token}");
        }
    }

    #[test]
    fn an_unclosed_quote_colors_the_rest_of_the_line() {
        assert_eq!(tokens("it's"), vec![("it", FOREGROUND), ("'s", STRING)]);
    }
}
