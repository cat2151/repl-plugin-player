use crate::snippets::Snippet;
use regex::{Regex, RegexBuilder};

/// Whitespace-separated regex terms are ANDed; a leading `-` excludes a term.
/// Invalid expressions match nothing, including invalid negative expressions.
#[derive(Default)]
pub(super) struct Filter {
    source: String,
    terms: Option<Vec<(bool, Regex)>>,
}

impl Filter {
    pub fn update(&mut self, source: &str) -> bool {
        if self.source == source {
            return false;
        }
        self.source = source.to_owned();
        self.terms = source
            .split_whitespace()
            .map(|term| {
                let (exclude, pattern) = term
                    .strip_prefix('-')
                    .map_or((false, term), |pattern| (true, pattern));
                // A lone minus is still being typed; it does not exclude everything.
                let pattern = if pattern.is_empty() { "-" } else { pattern };
                RegexBuilder::new(pattern)
                    .case_insensitive(true)
                    .build()
                    .ok()
                    .map(|regex| (exclude && term != "-", regex))
            })
            .collect();
        true
    }

    pub fn is_valid(&self) -> bool {
        self.source.is_empty() || self.terms.is_some()
    }

    pub fn matches(&self, snippet: &Snippet) -> bool {
        if self.source.is_empty() {
            return true;
        }
        let Some(terms) = &self.terms else {
            return false;
        };
        let mut fields = vec![
            snippet.group,
            snippet.name.as_str(),
            crate::i18n::group(snippet.group),
            &snippet.keywords,
        ];
        if snippet.group == "Built-in" {
            fields.push(crate::i18n::snippet_name(&snippet.name));
            fields.push(crate::i18n::english_snippet_name(&snippet.name));
        }
        terms
            .iter()
            .all(|(exclude, regex)| fields.iter().any(|field| regex.is_match(field)) != *exclude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_regex_terms_across_fields_and_excludes_matches() {
        let snippet = Snippet {
            name: "Try-Loop-42".into(),
            group: "User",
            keywords: String::new(),
            code: "not searched".into(),
        };
        let mut filter = Filter::default();
        for (query, expected) in [
            ("", true),
            ("  \t ", true),
            ("42 try user", true),
            ("try absent", false),
            ("^try- \\d+$ -easy", true),
            ("loop -(42|99)$", false),
            ("-easy -builtin", true),
            ("-user", false),
            ("not searched", false),
            ("\\D+", true),
            ("\\d{3}", false),
            ("-", true),
            ("[", false),
            ("-[", false),
            ("loop", true),
        ] {
            filter.update(query);
            assert_eq!(filter.matches(&snippet), expected, "{query}");
            assert_eq!(filter.is_valid(), !query.contains('['), "{query}");
        }
    }

    #[test]
    fn browser_and_keyboard_share_filter_and_cannot_run_hidden_preview() {
        use super::super::SnippetsPanel;
        use eframe::egui::{self, Event, Key, Modifiers, RawInput};

        let mut panel = SnippetsPanel {
            entries: ["keep-1", "keep-2", "drop-3"]
                .into_iter()
                .map(|name| Snippet {
                    name: name.into(),
                    group: "User",
                    keywords: String::new(),
                    code: "42".into(),
                })
                .collect(),
            filter: "user ^keep- -1$".into(),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let run = |panel: &mut SnippetsPanel, key| {
            let mut chosen = None;
            ctx.run_ui(
                RawInput {
                    events: vec![Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| chosen = panel.keyboard(ui.ctx(), true),
            )
            .drop_without_applying_deltas();
            chosen
        };
        run(&mut panel, Key::ArrowDown);
        assert_eq!(panel.preview, Some(1));
        assert_eq!(run(&mut panel, Key::Enter).unwrap().name, "keep-2");

        panel.filter = "keep -2$".into();
        assert_eq!(run(&mut panel, Key::Enter).unwrap().name, "keep-1");
        assert_eq!(panel.preview, Some(0));

        panel.preview = Some(1);
        ctx.run_ui(RawInput::default(), |ui| {
            panel.browser(ui, true);
        })
        .drop_without_applying_deltas();
        assert_eq!(panel.preview, Some(0));

        panel.filter = "-[".into();
        assert!(run(&mut panel, Key::ArrowDown).is_none());
        assert_eq!(panel.preview, None);
    }
}
