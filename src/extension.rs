//! Extension DLLs (`extensions/<dll>`), each loaded the first time one of its panes is
//! drawn. A pane is named `<dll>/<pane>`.
use eframe::egui;
use libloading::Library;
use repl_extension_api::{Extension, Frame, List, LIST};
use std::collections::BTreeMap;

/// The load result of every DLL drawn so far, by DLL name.
#[derive(Default)]
pub struct Extensions {
    loaded: BTreeMap<String, Result<&'static [Extension], String>>,
}

/// `<dll>/<pane>` as its two parts, when both are there and neither has a `/`.
pub fn split(name: &str) -> Option<(&str, &str)> {
    let (dll, pane) = name.split_once('/')?;
    (!dll.is_empty() && !pane.is_empty() && !pane.contains('/')).then_some((dll, pane))
}

/// The pane's title: the part after `/`.
pub fn title(name: &str) -> &str {
    split(name).map_or(name, |(_, pane)| pane)
}

fn load(dll: &str) -> Result<&'static [Extension], String> {
    let path = crate::resources::extension(dll)?;
    // SAFETY: The DLL is built in this workspace with the same `egui` and
    // `repl-extension-api`, so the Rust signature of its `LIST` matches `List`.
    unsafe {
        let library = Library::new(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let list = *library
            .get::<List>(LIST)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // Never unloaded: the list, its functions and the state they store in `egui`
        // point into the DLL.
        std::mem::forget(library);
        Ok(list())
    }
}

impl Extensions {
    /// Each DLL a draw asked for, with the reason it did not load.
    #[cfg(test)]
    pub fn loaded(&self) -> Vec<(&str, Result<(), &str>)> {
        let loaded = self.loaded.iter();
        loaded
            .map(|(name, result)| {
                (
                    name.as_str(),
                    result.as_ref().map(|_| ()).map_err(String::as_str),
                )
            })
            .collect()
    }

    /// The panes of DLL `dll`, loading it when no draw has yet.
    pub fn list(&mut self, dll: &str) -> Result<&'static [Extension], &str> {
        let loaded = self
            .loaded
            .entry(dll.to_owned())
            .or_insert_with(|| load(dll));
        loaded.as_ref().map(|list| *list).map_err(String::as_str)
    }

    /// Draws pane `name` (`<dll>/<pane>`). When it cannot be drawn, the reason takes its
    /// place: the DLL's path and load error, or the panes the DLL does have.
    pub fn show(&mut self, name: &str, ui: &mut egui::Ui, frame: &Frame) {
        // `config.toml` enables only names that split.
        let (dll, pane) = split(name).unwrap_or((name, ""));
        let error = match self.list(dll) {
            Ok(list) => match list.iter().find(|extension| extension.name == pane) {
                Some(extension) => return (extension.show)(ui, frame),
                None => {
                    let names: Vec<_> = list.iter().map(|extension| extension.name).collect();
                    crate::i18n::render(
                        "extension_missing",
                        &[
                            ("dll", dll.into()),
                            ("pane", pane.into()),
                            ("names", names.join(", ")),
                        ],
                    )
                }
            },
            Err(error) => error.to_owned(),
        };
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
}

#[cfg(test)]
mod tests;
