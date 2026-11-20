use crate::{
    execution::{self, Request, Scope},
    snippets::Snippet,
};

/// Preserve the existing document and insert after the cell containing the caret.
/// The returned request contains only the snippet, even when it has several cells.
pub fn insert(document: &mut String, caret: usize, snippet: &Snippet) -> Request {
    let current = execution::request(document, caret, caret, Scope::Cell);
    let byte = document
        .char_indices()
        .nth(current.chars.end)
        .map_or(document.len(), |(i, _)| i);
    let prefix = if byte > 0 && !document[..byte].ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let code = snippet.code.trim_start_matches('\u{feff}');
    let header = if code.lines().next().is_some_and(execution::marker) {
        String::new()
    } else {
        format!("// %% {}\n", snippet.name.replace(['\r', '\n'], " "))
    };
    let before = format!("{prefix}{header}");
    let start = current.chars.end + before.chars().count();
    let mut block = format!("{before}{code}");
    if !block.ends_with('\n') {
        block.push('\n');
    }
    document.insert_str(byte, &block);
    Request {
        scope: "snippet",
        chars: start..start + code.chars().count(),
        code: code.into(),
        first_line: document.chars().take(start).filter(|c| *c == '\n').count() + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(code: &str) -> Snippet {
        Snippet {
            name: "test".into(),
            group: "User",
            keywords: String::new(),
            code: code.into(),
        }
    }

    #[test]
    fn inserts_after_current_unicode_cell_and_runs_only_snippet() {
        let mut document = "// %% あ\n猫();\n// %% next\nthrow 'neighbor';".to_owned();
        let r = insert(&mut document, 8, &snippet("globalThis.value = '😀';"));
        assert_eq!(
            document,
            "// %% あ\n猫();\n// %% test\nglobalThis.value = '😀';\n// %% next\nthrow 'neighbor';"
        );
        assert_eq!(r.code, "globalThis.value = '😀';");
        assert_eq!(
            document
                .chars()
                .skip(r.chars.start)
                .take(r.chars.len())
                .collect::<String>(),
            r.code
        );
        assert_eq!(r.first_line, 4);
    }

    #[test]
    fn eof_empty_document_and_multicell_snippets_keep_boundaries() {
        for original in ["", "old();", "old();\r\n"] {
            let mut document = original.to_owned();
            let r = insert(
                &mut document,
                0,
                &snippet("// %% A\none();\n// %% B\ntwo();"),
            );
            assert!(document.starts_with(original));
            assert!(r.code.contains("one();\n// %% B\ntwo();"));
            assert!(!document.contains("// %% test"));
            assert_eq!(
                execution::request(&document, r.chars.start, r.chars.start, Scope::Cell).code,
                "one();\n"
            );
        }
    }
}
