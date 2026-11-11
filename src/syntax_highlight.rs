use eframe::egui::{text::LayoutJob, Color32, FontId, TextFormat};
use std::sync::LazyLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{Color, Style, StyleModifier, Theme, ThemeItem, ThemeSettings},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x27, 0x28, 0x22);
const FOREGROUND: Color32 = Color32::from_rgb(0xf8, 0xf8, 0xf2);
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static MONOKAI: LazyLock<Theme> = LazyLock::new(monokai);

fn color(rgb: u32) -> Color {
    Color {
        r: (rgb >> 16) as u8,
        g: (rgb >> 8) as u8,
        b: rgb as u8,
        a: 255,
    }
}

fn monokai() -> Theme {
    Theme {
        name: Some("Monokai".into()),
        settings: ThemeSettings {
            foreground: Some(color(0xf8f8f2)),
            background: Some(color(0x272822)),
            ..Default::default()
        },
        scopes: [
            ("comment", 0x75715e),
            ("string", 0xe6db74),
            (
                "constant.numeric, constant.language, constant.character",
                0xae81ff,
            ),
            ("keyword, storage", 0xf92672),
            ("storage.type, support.type, support.class", 0x66d9ef),
            ("entity.name.function, support.function", 0xa6e22e),
            ("variable.parameter", 0xfd971f),
        ]
        .into_iter()
        .map(|(scope, rgb)| ThemeItem {
            scope: scope.parse().expect("valid Monokai scope"),
            style: StyleModifier {
                foreground: Some(color(rgb)),
                ..Default::default()
            },
        })
        .collect(),
        ..Default::default()
    }
}

/// Keep parsing out of the 60 Hz repaint path when neither text nor font changes.
#[derive(Default)]
pub struct JavaScriptHighlight {
    cached: Option<(FontId, LayoutJob)>,
}

impl JavaScriptHighlight {
    pub fn layout(&mut self, text: &str, font: FontId) -> LayoutJob {
        if let Some((cached_font, job)) = &self.cached {
            if cached_font == &font && job.text == text {
                return job.clone();
            }
        }
        let job = highlight(text, font.clone());
        self.cached = Some((font, job.clone()));
        job
    }
}

fn highlight(text: &str, font: FontId) -> LayoutJob {
    let syntax = SYNTAXES
        .find_syntax_by_extension("js")
        .expect("bundled JavaScript syntax");
    let mut highlighter = HighlightLines::new(syntax, &MONOKAI);
    let mut job = LayoutJob::default();
    for line in LinesWithEndings::from(text) {
        match highlighter.highlight_line(line, &SYNTAXES) {
            Ok(spans) => {
                for (style, fragment) in spans {
                    append(&mut job, fragment, &font, style);
                }
            }
            Err(_) => job.append(line, 0.0, TextFormat::simple(font.clone(), FOREGROUND)),
        }
    }
    job
}

fn append(job: &mut LayoutJob, text: &str, font: &FontId, style: Style) {
    let fg = style.foreground;
    job.append(
        text,
        0.0,
        TextFormat::simple(font.clone(), Color32::from_rgb(fg.r, fg.g, fg.b)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_color(job: &LayoutJob, token: &str) -> Color32 {
        let index = job.text.find(token).unwrap();
        job.sections
            .iter()
            .find(|section| section.byte_range.contains(&index.into()))
            .unwrap()
            .format
            .color
    }

    #[test]
    fn javascript_uses_monokai_and_preserves_unicode_and_line_endings() {
        let text = "// 日本語\r\nif (true) play('音', 42);\n/* multi\nline */\n";
        let job = highlight(text, FontId::monospace(14.0));
        assert_eq!(job.text, text);
        for (token, rgb) in [
            ("//", 0x75715e),
            ("if", 0xf92672),
            ("true", 0xae81ff),
            ("音", 0xe6db74),
            ("42", 0xae81ff),
            ("line */", 0x75715e),
        ] {
            let expected = color(rgb);
            assert_eq!(
                token_color(&job, token),
                Color32::from_rgb(expected.r, expected.g, expected.b),
                "{token}"
            );
        }
        assert_eq!(job.sections.first().unwrap().byte_range.start.0, 0);
        assert_eq!(job.sections.last().unwrap().byte_range.end.0, text.len());
    }

    #[test]
    fn edits_and_font_changes_invalidate_cached_highlighting() {
        let mut highlighter = JavaScriptHighlight::default();
        let font = FontId::monospace(14.0);
        highlighter.layout("/* open\n42", font.clone());
        let edited = highlighter.layout("/* closed */\n42", font);
        assert_eq!(
            token_color(&edited, "42"),
            Color32::from_rgb(0xae, 0x81, 0xff)
        );
        let resized = highlighter.layout(&edited.text, FontId::monospace(20.0));
        assert!(resized
            .sections
            .iter()
            .all(|s| s.format.font_id.size == 20.0));
        assert!(highlighter
            .layout("", FontId::monospace(14.0))
            .text
            .is_empty());
    }
}
