use crate::{
    config,
    control::{Control, INIT, TIMELINE_READ},
    log::Log,
    timeline,
};
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

mod console;
mod help;
mod logs;
mod patch;
mod settings;
mod snippet_execution;
mod startup;

/// The left code pane; the right pane is the `CentralPanel` that follows it.
/// Each pane keeps its configured minimum share of `available_width`.
fn code_panel(available_width: f32, panes: config::Panes) -> egui::Panel {
    egui::Panel::left("code")
        .default_size(630.0)
        .min_size(available_width * panes.left_min)
        .max_size(available_width * (1.0 - panes.right_min))
        .resizable(true)
}

#[cfg(test)]
mod pane_regression;

pub fn run() -> Result<(), String> {
    let config_path = config::path();
    let (config, config_errors) = match &config_path {
        Ok(path) => config::read(path),
        Err(error) => (config::Config::default(), vec![error.clone()]),
    };
    let mut viewport = egui::ViewportBuilder::default().with_inner_size(config.window.size);
    if let Some(position) = config.window.position {
        viewport = viewport.with_position(position);
    }
    eframe::run_native(
        "REPL Plugin Player",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::fonts(&cc.egui_ctx);
            let settings =
                settings::Settings::new(crate::external_editor::status_path(), config_path);
            Ok(Box::new(Editor::new(config.panes, config_errors, settings)))
        }),
    )
    .map_err(|e| e.to_string())
}
struct Editor {
    control: Option<Control>,
    code: String,
    script_editor: crate::script_editor::ScriptEditor,
    error: String,
    log_path: PathBuf,
    last_observation: Instant,
    timeline: Option<serde_json::Value>,
    timeline_error: String,
    startup: startup::Startup,
    installer: crate::install::Installer,
    script_ui: crate::script_ui::Renderer,
    snippet_execution: snippet_execution::Pending,
    panes: config::Panes,
    config_errors: Vec<String>,
    settings: settings::Settings,
    help: help::Help,
}
impl Editor {
    fn new(panes: config::Panes, config_errors: Vec<String>, settings: settings::Settings) -> Self {
        let mut script_editor = crate::script_editor::ScriptEditor::default();
        let code = script_editor.initial_code();
        Self {
            control: None,
            code,
            script_editor,
            error: String::new(),
            log_path: PathBuf::new(),
            last_observation: Instant::now(),
            timeline: None,
            timeline_error: String::new(),
            startup: startup::Startup::default(),
            installer: crate::install::Installer::default(),
            script_ui: crate::script_ui::Renderer::default(),
            snippet_execution: snippet_execution::Pending::default(),
            panes,
            config_errors,
            settings,
            help: help::Help::default(),
        }
    }
    fn initialize(&mut self) {
        let result = Log::new().and_then(|log| {
            self.log_path = log.path.clone();
            Control::open(log, INIT)
        });
        match result {
            Ok(mut control) => {
                control.enable_gui();
                self.control = Some(control);
            }
            Err(error) => self.error = crate::i18n::render("f000", &[("error", error.to_string())]),
        }
    }
    fn refresh_timeline(&mut self) {
        if let Some(control) = &self.control {
            match control.read_timeline() {
                Ok(snapshot) => {
                    if snapshot["view"]["loop"]["ready"] == false {
                        return; // Audio is exchanging its guarded display snapshot.
                    }
                    self.timeline = Some(snapshot);
                    self.timeline_error.clear();
                }
                Err(error) => {
                    self.timeline = None;
                    self.timeline_error = error;
                }
            }
        }
    }
    fn error(&mut self, stage: &str, error: String) {
        if let Some(control) = &mut self.control {
            let _ = control
                .log
                .event("error", serde_json::json!({"stage":stage,"error":error}));
        }
        self.error = format!("{stage}: {error}");
    }
    fn show_reference(&self, ui: &mut egui::Ui) {
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
    fn execute(&mut self, request: crate::execution::Request) -> Result<(), String> {
        self.error.clear();
        let Some(control) = &mut self.control else {
            return Err(crate::i18n::t("m061").into());
        };
        let result = control.run_request(&request);
        if let Err(error) = &result {
            self.error(crate::i18n::t("stage_script"), error.clone());
        }
        self.script_editor.executed(request);
        self.refresh_timeline();
        result
    }
}
impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.settings.shortcut(&ctx);
        self.help.shortcut(&ctx, self.script_editor.snippets_open());
        if let Some(request) = self.snippet_execution.take_ready(&ctx) {
            let _ = self.execute(request);
            self.write_browsed_cell(&ctx);
        }
        // Present the overlay before synchronous native initialization begins.
        // Keep all native calls on the UI/control thread.
        if self.startup.can_initialize(&ctx) && self.control.is_none() && self.error.is_empty() {
            self.initialize();
        }
        if self.error.is_empty()
            || self
                .control
                .as_ref()
                .is_some_and(|control| control.ready || control.stopping)
        {
            let result = self.control.as_mut().map(Control::poll).transpose();
            if let Err(error) = result {
                self.error(crate::i18n::t("stage_prepare"), error);
            }
        }
        if self.last_observation.elapsed() >= Duration::from_millis(100) {
            if let Some(control) = &mut self.control {
                if control.ready {
                    let _ = control.observe("GUI periodic; history snapshot");
                }
            }
            if self.control.as_ref().is_some_and(|control| control.ready) {
                self.refresh_timeline();
            }
            self.last_observation = Instant::now();
        }
        self.startup.update(
            self.control
                .as_ref()
                .is_some_and(|c| c.ready && !c.stopping),
            !self.error.is_empty() || self.control.as_ref().is_some_and(|c| c.missing_surge),
        );
        let preparing = self.startup.is_preparing() || self.snippet_execution.is_pending();
        code_panel(ui.available_width(), self.panes).show(ui, |ui| {
            if preparing {
                ui.disable();
            }
            ui.heading("JavaScript");
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
                    self.write_browsed_cell(&ctx);
                }
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(crate::i18n::t("m001"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.settings.button(ui);
                });
            });
            // Before the disable below: results stay selectable while a run is pending.
            console::show(ui, self.control.as_ref().map_or(&[], |c| &c.results[..]));
            if preparing || self.snippet_execution.is_pending() {
                ui.disable();
            }
            for error in &self.config_errors {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
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
            let notes = self
                .control
                .as_ref()
                .filter(|control| control.ready)
                .and_then(|control| control.read_notes().ok())
                .unwrap_or_default();
            let projected = self.timeline.as_ref().map(timeline::projection::project);
            // Bottom panels stack upward from the bottom edge, so every section stays
            // visible and the timeline takes only the height left above them.
            egui::Panel::bottom("reference").show(ui, |ui| self.show_reference(ui));
            egui::Panel::bottom("logs").show(ui, |ui| {
                if logs::show(ui, &self.log_path).is_err() {
                    self.error = crate::i18n::t("log_directory_open_error").into();
                }
            });
            egui::Panel::bottom("scope").show(ui, |ui| {
                crate::scope_ui::show(ui, self.control.as_ref().and_then(Control::scope));
            });
            egui::Panel::bottom("keyboard").show(ui, |ui| {
                crate::keyboard::show(
                    ui,
                    &notes,
                    projected.as_ref().and_then(|view| view.pitch_range()),
                );
            });
            if let Some(control) = &self.control {
                ui.label(crate::i18n::render(
                    "f001",
                    &[("arg0", format!("{}", control.log.execution))],
                ));
            }
            if !self.timeline_error.is_empty() {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    crate::i18n::render("f003", &[("arg0", self.timeline_error.to_string())]),
                );
            } else if let Some(projected) = &projected {
                timeline::show(ui, projected);
            } else {
                ui.heading(crate::i18n::t("m002"));
                ui.label(crate::i18n::t("m003"));
            }
        });
        if let Some(control) = &mut self.control {
            if control.ready && !control.stopping && !self.snippet_execution.is_pending() {
                self.script_ui.show(&ctx, control);
            }
        }
        self.startup.show_overlay(&ctx);
        self.snippet_execution.show_overlay(&ctx);
        self.settings.show(&ctx);
        self.help.show(&ctx);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(control) = &mut self.control {
            let _ = control.close();
        }
    }
}
