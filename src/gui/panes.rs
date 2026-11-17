//! The contents of each tile of the main window, drawn into the `Ui` the tile gives.
use super::{logs, Editor};
use crate::{
    control::{Control, TIMELINE_READ},
    timeline,
};
use eframe::egui;

impl Editor {
    /// The Stop button and the script editor.
    pub(super) fn editor_pane(&mut self, ui: &mut egui::Ui, preparing: bool) {
        if preparing {
            ui.disable();
        }
        let ready = self
            .control
            .as_ref()
            .is_some_and(|c| c.ready && !c.stopping)
            && !preparing;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.control.as_ref().is_some_and(|control| control.ready),
                    egui::Button::new(crate::i18n::t("m000")),
                )
                .clicked()
            {
                if let Some(control) = &mut self.control {
                    if let Err(error) = control.stop("GUI Stop") {
                        self.error(crate::i18n::t("m000"), error);
                    }
                }
            }
            if ready {
                ui.label(crate::i18n::t("m044"));
            }
        });
        self.startup.focus_editor(ui);
        if let Some(request) = self.script_editor.show(ui, &mut self.code, ready) {
            if self.snippet_execution.presents(&request) {
                self.snippet_execution.queue(request);
            } else {
                let _ = self.execute(request);
                self.write_browsed_cell(&ui.ctx().clone());
            }
        }
    }

    /// Results, errors, the installer, the run count and the timeline.
    pub(super) fn main_pane(
        &mut self,
        ui: &mut egui::Ui,
        preparing: bool,
        projected: Option<&timeline::projection::View>,
    ) {
        if preparing {
            ui.disable();
        }
        for error in &self.config_errors {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        if let Some(file) = self.layout_file.as_ref().filter(|f| !f.error.is_empty()) {
            ui.colored_label(egui::Color32::LIGHT_RED, &file.error);
        }
        let missing = self.control.as_ref().is_some_and(|c| c.missing_surge);
        if self.installer.show(ui, missing) {
            self.error.clear();
            if let Some(control) = &mut self.control {
                if let Err(error) = control.retry_surge() {
                    self.error = error;
                }
            }
        }
        if !self.error.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_RED, &self.error);
        }
        egui::ScrollArea::horizontal()
            .id_salt("execution_meters")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.performance.show(
                        ui,
                        self.control.as_ref().map(|control| control.log.execution),
                    );
                });
            });
        if !self.timeline_error.is_empty() {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                crate::i18n::render("f003", &[("arg0", self.timeline_error.to_string())]),
            );
        } else if let Some(projected) = projected {
            timeline::show(ui, projected);
        } else {
            ui.heading(crate::i18n::t("m002"));
            ui.label(crate::i18n::t("m003"));
        }
    }

    pub(super) fn scope_pane(&mut self, ui: &mut egui::Ui) {
        let ready = self
            .control
            .as_ref()
            .is_some_and(|control| control.ready && !control.stopping)
            && !self.snippet_execution.is_pending();
        let mut cycle = None;
        ui.horizontal(|ui| {
            for (symbol, command) in [
                ("◀", "include('builtin/lib/scope.js').cycleTrack(-1)"),
                ("▶", "include('builtin/lib/scope.js').cycleTrack(1)"),
            ] {
                if ui.add_enabled(ready, egui::Button::new(symbol)).clicked() {
                    cycle = Some(command);
                }
            }
            if let Some(snapshot) = &self.timeline {
                if let Some(target) = snapshot["scopeTarget"].as_i64().filter(|id| *id >= 0) {
                    if let Some(track) = snapshot["tracks"].as_array().and_then(|tracks| {
                        tracks.iter().find(|track| {
                            track["nodes"].as_array().is_some_and(|nodes| {
                                nodes
                                    .iter()
                                    .any(|node| node["instanceId"].as_i64() == Some(target))
                            })
                        })
                    }) {
                        if let Some(id) = track["trackId"].as_str() {
                            ui.monospace(id);
                        }
                    }
                }
            }
        });
        if let (Some(command), Some(control)) = (cycle, &mut self.control) {
            if let Err(error) = control.eval(command, "Scope track") {
                self.error("Scope track", error);
            }
            self.refresh_timeline();
        }
        crate::scope_ui::show(ui, self.control.as_ref().and_then(Control::scope));
    }

    pub(super) fn log_pane(&mut self, ui: &mut egui::Ui) {
        if logs::show(ui, &self.log_path).is_err() {
            self.error = crate::i18n::t("log_directory_open_error").into();
        }
    }

    pub(super) fn reference_pane(&self, ui: &mut egui::Ui) {
        if let Some(snapshot) = &self.timeline {
            egui::CollapsingHeader::new(crate::i18n::t("m042"))
                .id_salt("timeline_raw")
                .show(ui, |ui| {
                    egui::ScrollArea::both().max_height(200.0).show(ui, |ui| {
                        ui.monospace(serde_json::to_string_pretty(snapshot).unwrap_or_default());
                    });
                });
        }
        egui::CollapsingHeader::new(crate::i18n::t("m004"))
            .id_salt("m004")
            .show(ui, |ui| {
                egui::ScrollArea::both().max_height(190.0).show(ui, |ui| {
                    ui.monospace(crate::snippets::localize_builtin(
                        &TIMELINE_READ
                            .text()
                            .map(str::to_owned)
                            .unwrap_or_else(|error| error),
                    ));
                });
            });
        egui::CollapsingHeader::new(crate::i18n::t("m005"))
            .id_salt("m005")
            .show(ui, |ui| {
                if let Some(control) = &self.control {
                    ui.monospace(control.stop_script());
                }
            });
        egui::CollapsingHeader::new(crate::i18n::t("m006"))
            .id_salt("m006")
            .show(ui, |ui| {
                ui.monospace("include('builtin/lib/plugins.js').instrumentOn(0)");
                ui.label(crate::i18n::t("comment_13"));
            });
    }
}
