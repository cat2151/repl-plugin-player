//! An E2E case written as a `.js` file in the source tree, read at startup.
//!
//! The file is split into `// %% <title>` cells that run one at a time. Words in
//! a title change how the runner treats the cell: `init` is the startup cell
//! (otherwise `tests/e2e/init.js`), `stop` stops the engine before the cell
//! runs, and `error` must fail. `tests/e2e/lib.js` runs once before the first
//! cell.
//!
//! A case whose checks stay in Rust keeps its JS in the same kind of file and
//! looks cells up by title; a cell called with arguments is a function
//! expression that receives them as one JSON object.
use crate::control::Control;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct Script {
    pub init: String,
    /// The whole file, for cases that run parts of it as the editor does.
    pub text: String,
    lib: String,
    cells: Vec<Cell>,
}

struct Cell {
    title: String,
    code: String,
}

impl Cell {
    fn has(&self, word: &str) -> bool {
        self.title.split_whitespace().any(|w| w == word)
    }
}

fn root() -> Result<PathBuf, String> {
    crate::resources::source_root().ok_or_else(|| "E2E cases need the source tree".into())
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The test-owned startup cell that starts Surge XT on track 0.
fn default_init() -> Result<String, String> {
    read(&root()?.join("tests/e2e/init.js"))
}

/// `tests/e2e/<case>.js` or `patches/uapmd/*/<case>.js`; at most one may exist.
fn find(root: &Path, case: &str) -> Result<Option<PathBuf>, String> {
    let file = format!("{case}.js");
    let mut found: Vec<PathBuf> = [root.join("tests/e2e").join(&file)]
        .into_iter()
        .chain(
            std::fs::read_dir(root.join("patches/uapmd"))
                .map_err(|e| e.to_string())?
                .filter_map(|entry| Some(entry.ok()?.path().join(&file))),
        )
        .filter(|path| path.is_file())
        .collect();
    match found.len() {
        0 | 1 => Ok(found.pop()),
        _ => Err(format!("E2E case {case} is ambiguous: {found:?}")),
    }
}

fn cells(text: &str) -> Vec<Cell> {
    let mut cells = vec![Cell {
        title: String::new(),
        code: String::new(),
    }];
    for line in text.split_inclusive('\n') {
        if crate::execution::marker(line) {
            let title = line.trim().trim_start_matches("// %%").trim().to_owned();
            cells.push(Cell {
                title,
                code: String::new(),
            });
        } else {
            cells.last_mut().expect("first cell").code.push_str(line);
        }
    }
    cells.retain(|cell| !cell.code.trim().is_empty() || cell.has("stop"));
    cells
}

impl Script {
    /// A case that runs its cells needs its file; a Rust case may have none.
    pub fn load(case: &str, required: bool) -> Result<Self, String> {
        let root = root()?;
        let text = match find(&root, case)? {
            Some(path) => read(&path)?,
            None if required => return Err(format!("Unknown E2E case: {case}")),
            None => String::new(),
        };
        let mut cells = cells(&text);
        let init = match cells.iter().position(|cell| cell.has("init")) {
            Some(index) => cells.remove(index).code,
            None => default_init()?,
        };
        Ok(Self {
            init,
            text,
            lib: read(&root.join("tests/e2e/lib.js"))?,
            cells,
        })
    }

    /// The code of the cell titled exactly `title`.
    pub fn cell(&self, title: &str) -> Result<&str, String> {
        self.cells
            .iter()
            .find(|cell| cell.title == title)
            .map(|cell| cell.code.trim())
            .ok_or_else(|| format!("E2E script has no cell '{title}'"))
    }

    /// Code that calls the function-expression cell `title` with `args`.
    pub fn call(&self, title: &str, args: &Value) -> Result<String, String> {
        let function = self.cell(title)?.trim_end_matches(';');
        Ok(format!("(\n{function}\n)({args})"))
    }

    /// The character offset of the marker line of the cell titled `title`.
    pub fn marker(&self, title: &str) -> Result<usize, String> {
        let mut offset = 0;
        for line in self.text.split_inclusive('\n') {
            if crate::execution::marker(line)
                && line.trim().trim_start_matches("// %%").trim() == title
            {
                return Ok(offset);
            }
            offset += line.chars().count();
        }
        Err(format!("E2E script has no cell '{title}'"))
    }

    pub fn run(&self, app: &mut Control, case: &str) -> Result<(), String> {
        app.run(&self.lib)?;
        for cell in &self.cells {
            if cell.has("stop") {
                app.stop("E2E Stop before new Run")?;
                crate::e2e::wait_stopped(app)?;
            }
            let request = crate::execution::request(&cell.code, 0, 0, crate::execution::Scope::All);
            let result = app.run_request_result(&request);
            let value = match (result, cell.has("error")) {
                (Ok(value), false) => serde_json::from_str(&value).unwrap_or(Value::String(value)),
                (Err(error), true) => {
                    app.log
                        .event("expected_error", json!({"stage":"script","error":error}))?;
                    Value::String(error)
                }
                (Ok(value), true) => {
                    return Err(format!("E2E cell '{}' did not fail: {value}", cell.title))
                }
                (Err(error), false) => {
                    return Err(format!("E2E cell '{}' failed: {error}", cell.title))
                }
            };
            app.log.event(
                "e2e_cell",
                json!({"case":case,"title":cell.title,"value":value}),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_split_on_markers_and_keep_titles() {
        let cells = cells("// head\n// %% init\na();\n// %% stop\n// %% error check\nb();\n");
        let titles: Vec<_> = cells.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["", "init", "stop", "error check"]);
        assert_eq!(cells[1].code, "a();\n");
        assert!(cells[3].has("error") && !cells[3].has("check2"));
    }

    fn script(text: &str) -> Script {
        Script {
            init: String::new(),
            text: text.into(),
            lib: String::new(),
            cells: cells(text),
        }
    }

    #[test]
    fn cells_are_found_by_title_and_called_with_json() {
        let s = script("é\n// %% add\n({a, b}) => a + b;\n// %% two words\nx\n");
        assert_eq!(s.cell("two words").unwrap(), "x");
        assert_eq!(
            s.call("add", &json!({"a":1,"b":"q"})).unwrap(),
            "(\n({a, b}) => a + b\n)({\"a\":1,\"b\":\"q\"})"
        );
        assert_eq!(s.marker("add").unwrap(), 2);
        assert!(s.cell("two").is_err() && s.marker("missing").is_err());
    }

    #[test]
    fn blank_cells_are_dropped_unless_they_stop() {
        let titles: Vec<_> = cells("\n// %% empty\n\n// %% stop\n")
            .into_iter()
            .map(|c| c.title)
            .collect();
        assert_eq!(titles, ["stop"]);
    }
}
