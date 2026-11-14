//! The extensions section of the settings overlay: a checkbox for each `<dll>/<pane>`,
//! whose choice applies at the next startup.
use crate::extension::Extensions;
use eframe::egui;

pub(super) struct Chooser {
    /// On at startup: what this run shows.
    pub running: Vec<String>,
    /// On for the next startup.
    pub chosen: Vec<String>,
    /// The DLLs there are, listed when the section is first drawn.
    dlls: Option<Vec<String>>,
}

impl Chooser {
    pub fn new(running: Vec<String>) -> Self {
        Self {
            chosen: running.clone(),
            running,
            dlls: None,
        }
    }

    /// Lists every pane of every DLL, loading each DLL the first time; an on name that
    /// no DLL has is listed in red, so it can be turned off. Returns whether a checkbox
    /// changed [`Self::chosen`].
    pub fn show(&mut self, ui: &mut egui::Ui, extensions: &mut Extensions) -> bool {
        ui.label(crate::i18n::t("settings_extensions"));
        let dlls = (self.dlls)
            .get_or_insert_with(crate::resources::extension_names)
            .clone();
        let mut listed = Vec::new();
        let mut changed = false;
        for dll in dlls {
            match extensions.list(&dll) {
                Ok(list) => {
                    for extension in list {
                        let name = format!("{dll}/{}", extension.name);
                        changed |= self.checkbox(ui, &name, None);
                        listed.push(name);
                    }
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
        }
        let mut unlisted: Vec<String> = Vec::new();
        for name in self.running.iter().chain(&self.chosen) {
            if !listed.contains(name) && !unlisted.contains(name) {
                unlisted.push(name.clone());
            }
        }
        for name in unlisted {
            changed |= self.checkbox(ui, &name, Some(egui::Color32::LIGHT_RED));
        }
        if self.restart_needed() {
            ui.label(crate::i18n::t("settings_extensions_restart"));
        }
        changed
    }

    fn checkbox(&mut self, ui: &mut egui::Ui, name: &str, color: Option<egui::Color32>) -> bool {
        let mut on = self.chosen.iter().any(|chosen| chosen == name);
        let mut text = egui::RichText::new(name);
        if let Some(color) = color {
            text = text.color(color);
        }
        if !ui.checkbox(&mut on, text).changed() {
            return false;
        }
        match on {
            true => self.chosen.push(name.to_owned()),
            false => self.chosen.retain(|chosen| chosen != name),
        }
        true
    }

    /// Whether the next startup turns on a different set than this run did.
    fn restart_needed(&self) -> bool {
        let sorted = |names: &[String]| {
            let mut names = names.to_vec();
            names.sort();
            names
        };
        sorted(&self.running) != sorted(&self.chosen)
    }
}
