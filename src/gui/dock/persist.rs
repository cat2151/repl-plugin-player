//! Keeps the layout equal to its source: `layout-auto.toml`, which the app writes when
//! the layout changes, or the `[layout]` of `config.toml`, which only the user writes.
//! An edit of the source is applied. While it does not read, nothing is written, so the
//! user's text stays until it is fixed.
use super::{file, Layout, Pane};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(500);

pub(in crate::gui) struct LayoutFile {
    path: PathBuf,
    /// `layout-auto.toml`, read and written; else `config.toml`, only read.
    auto: bool,
    /// The file's text as last read or written.
    text: Option<String>,
    last: Instant,
    /// Why the file's text is not the layout; empty when it is.
    pub error: String,
}

impl LayoutFile {
    /// The source in the user `directory` and the layout it holds with the enabled
    /// `extensions`, or the first layout when it is absent or does not read.
    pub fn open(directory: &Path, auto: bool, extensions: &[String]) -> (Self, Layout) {
        let name = if auto {
            "layout-auto.toml"
        } else {
            "config.toml"
        };
        let mut file = Self {
            path: directory.join(name),
            auto,
            text: None,
            last: Instant::now(),
            error: String::new(),
        };
        let mut layout = super::initial(extensions);
        file.read(&mut layout);
        (file, layout)
    }

    /// Changes to the other source and takes its layout, every window's place included;
    /// `layout-auto.toml` is written first, so it keeps the layout it was left with.
    pub fn switch(&mut self, layout: &mut Layout) {
        self.save(layout);
        let directory = self.path.parent().unwrap_or(&self.path).to_owned();
        let (file, read) = Self::open(&directory, !self.auto, &layout.extensions);
        apply(layout, read);
        layout.switched = true;
        *self = file;
    }

    pub fn poll(&mut self, layout: &mut Layout) {
        if self.last.elapsed() >= POLL {
            self.last = Instant::now();
            self.read(layout);
            self.save(layout);
        }
    }

    /// Writes the layout to `layout-auto.toml` unless it is not read; on exit and every
    /// poll.
    pub fn save(&mut self, layout: &Layout) {
        if !self.auto || !self.error.is_empty() {
            return;
        }
        let text = file::write(layout);
        if self.text.as_ref() != Some(&text) {
            let result = std::fs::create_dir_all(self.path.parent().unwrap_or(&self.path))
                .and_then(|()| std::fs::write(&self.path, &text));
            match result {
                Ok(()) => self.text = Some(text),
                Err(error) => self.error = self.message(error.to_string()),
            }
        }
    }

    /// Applies the file when its text changed since it was last read or written.
    fn read(&mut self, layout: &mut Layout) {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => return self.error = self.message(error.to_string()),
        };
        if self.text.as_ref() == Some(&text) {
            return;
        }
        let extensions = &layout.extensions;
        let read = if self.auto {
            file::read(&text, extensions)
        } else {
            in_config(&text, extensions)
        };
        let read = read.and_then(|mut read| {
            read.resolved = super::monitor::resolve_all(&read.monitor)?;
            Ok(read)
        });
        match read {
            Ok(read) => {
                apply(layout, read);
                self.error.clear();
            }
            Err(error) => self.error = self.message(error),
        }
        self.text = Some(text);
    }

    fn message(&self, error: String) -> String {
        let file = self.path.file_name().unwrap_or_default();
        let file = file.to_string_lossy().into_owned();
        crate::i18n::render("layout_error", &[("file", file), ("error", error)])
    }
}

/// The `[layout]` of `config.toml`; without it, the first layout. The main window is
/// `[window]`, whose errors `config::read` reports at startup.
fn in_config(text: &str, extensions: &[String]) -> Result<Layout, String> {
    let root: toml::Table = toml::from_str(text).map_err(|error| error.to_string())?;
    let mut layout = match root.get("layout") {
        None => super::initial(extensions),
        Some(toml::Value::Table(layout)) => file::read_table(layout, extensions)?,
        Some(_) => {
            return Err(crate::i18n::render(
                "layout_key",
                &[("key", "layout".into())],
            ))
        }
    };
    layout.main = crate::config::parse(text).0.window;
    Ok(layout)
}

/// Takes the read layout; script windows keep being floating or docked.
fn apply(layout: &mut Layout, read: Layout) {
    let old = std::mem::replace(layout, read);
    let script = |pane: &Pane| matches!(pane, Pane::Script(_));
    for float in old.floating {
        if float.panes().iter().all(script) {
            layout.floating.push(float);
        }
    }
    for (_, tile) in old.tree.tiles.iter() {
        if let egui_tiles::Tile::Pane(pane) = tile {
            if script(pane) {
                layout.dock(pane.clone());
            }
        }
    }
}

#[cfg(test)]
#[path = "persist_tests.rs"]
mod tests;
