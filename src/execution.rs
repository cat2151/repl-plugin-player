//! Editor ranges use Unicode character offsets, matching egui's cursors.
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    All,
    Line,
    Cell,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub scope: &'static str,
    pub chars: Range<usize>,
    pub code: String,
    pub first_line: usize,
}

pub fn request(text: &str, primary: usize, secondary: usize, scope: Scope) -> Request {
    let chars: Vec<char> = text.chars().collect();
    let cursor = primary.min(chars.len());
    let anchor = secondary.min(chars.len());
    let (name, range) = if scope == Scope::All {
        ("whole editor", 0..chars.len())
    } else if cursor != anchor {
        ("selection", cursor.min(anchor)..cursor.max(anchor))
    } else if scope == Scope::Line {
        let start = chars[..cursor]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1);
        let end = chars[cursor..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(chars.len(), |i| cursor + i);
        ("line", start..end)
    } else {
        let mut start = 0;
        let mut end = chars.len();
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            let line_end = offset + line.chars().count();
            // A marker line belongs to the cell that follows it. Markers are
            // reserved editor delimiters, including inside multiline JS strings.
            if marker(line) {
                if offset <= cursor {
                    start = line_end;
                } else {
                    end = offset;
                    break;
                }
            }
            offset = line_end;
        }
        ("cell", start..end)
    };
    Request {
        scope: name,
        first_line: chars[..range.start].iter().filter(|c| **c == '\n').count() + 1,
        code: chars[range.clone()].iter().collect(),
        chars: range,
    }
}

pub fn marker(line: &str) -> bool {
    line.trim()
        .strip_prefix("// %%")
        .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_has_priority_and_uses_characters_in_both_directions() {
        for scope in [Scope::Line, Scope::Cell] {
            for (a, b) in [(2, 5), (5, 2)] {
                let r = request("あ\n猫😀犬\n後", a, b, scope);
                assert_eq!(r.code, "猫😀犬");
                assert_eq!(r.scope, "selection");
                assert_eq!(r.first_line, 2);
            }
        }
        assert_eq!(request("abc", 1, 2, Scope::All).code, "abc");
    }

    #[test]
    fn lines_handle_newline_eof_crlf_and_empty_document() {
        let text = "あ\r\nb\n";
        assert_eq!(request(text, 2, 2, Scope::Line).code, "あ\r");
        assert_eq!(request(text, 3, 3, Scope::Line).code, "b");
        assert_eq!(request(text, 5, 5, Scope::Line).code, "");
        assert_eq!(request("", 9, 9, Scope::Line).code, "");
        assert_eq!(request("abc", 3, 3, Scope::Line).code, "abc");
    }

    #[test]
    fn cells_exclude_delimiters_and_do_not_run_neighbors() {
        let text = "head();\n// %% first\n猫();\n// %% second\ntail();";
        let first = text.find("// %% first").unwrap();
        let second = text.chars().position(|c| c == '猫').unwrap();
        assert_eq!(request(text, 0, 0, Scope::Cell).code, "head();\n");
        for cursor in [first, first + 5, second] {
            let r = request(text, cursor, cursor, Scope::Cell);
            assert_eq!(r.code, "猫();\n");
            assert_eq!(r.first_line, 3);
        }
        assert_eq!(
            request(text, usize::MAX, usize::MAX, Scope::Cell).code,
            "tail();"
        );
        assert_eq!(request("// %%\n// %%", 0, 0, Scope::Cell).code, "");
        assert_eq!(request("var x = 1;", 4, 4, Scope::Cell).code, "var x = 1;");
        assert!(marker("  // %% label\r\n"));
        assert!(!marker("// %%%"));
        assert!(!marker("var x = '// %%';"));
    }
}
