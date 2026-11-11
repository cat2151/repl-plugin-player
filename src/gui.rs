use crate::{
    control::{Control, SURGE, TIMELINE_READ},
    log::Log,
    timeline,
};
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

mod logs;
mod snippet_execution;
mod startup;

pub fn run() -> Result<(), String> {
    eframe::run_native(
        "REPL Plugin Player",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1250.0, 800.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            crate::i18n::fonts(&cc.egui_ctx);
            Ok(Box::new(Editor::new()))
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
}
impl Editor {
    fn new() -> Self {
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
        }
    }
    fn initialize(&mut self) {
        let result = Log::new().and_then(|log| {
            self.log_path = log.path.clone();
            Control::open(log, SURGE)
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
    fn execute(&mut self, request: crate::execution::Request) {
        self.error.clear();
        if let Some(control) = &mut self.control {
            if let Err(error) = control.run_request(&request) {
                self.error(crate::i18n::t("stage_script"), error);
            }
        }
        self.script_editor.executed(request);
        self.refresh_timeline();
    }
}
impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(request) = self.snippet_execution.take_ready(&ctx) {
            self.execute(request);
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
        egui::Panel::left("code")
            .default_size(630.0)
            .max_size(ui.available_width() * 0.9)
            .resizable(true)
            .show(ui, |ui| {
                if preparing {
                    ui.disable();
                }
                ui.heading("JavaScript");
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
                    if let Some(control) = &self.control {
                        ui.label(&control.message);
                    }
                });
                let ready = self
                    .control
                    .as_ref()
                    .is_some_and(|c| c.ready && !c.stopping)
                    && !preparing;
                self.startup.focus_editor(ui);
                if let Some(request) = self.script_editor.show(ui, &mut self.code, ready) {
                    if request.scope == "snippet" {
                        self.snippet_execution.queue(request);
                    } else {
                        self.execute(request);
                    }
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if preparing || self.snippet_execution.is_pending() {
                ui.disable();
            }
            ui.heading(crate::i18n::t("m001"));
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
            if let Some(control) = &self.control {
                ui.label(crate::i18n::render(
                    "f001",
                    &[("arg0", format!("{}", control.log.execution))],
                ));
                if control.ready {
                    ui.label(crate::i18n::render(
                        "f002",
                        &[("arg0", format!("{}", control.instance))],
                    ));
                }
            }
            let notes = self
                .control
                .as_ref()
                .filter(|control| control.ready)
                .map(|control| control.read_notes());
            let view = match &notes {
                Some(Ok(notes)) => Ok(notes.as_slice()),
                Some(Err(error)) => Err(error.as_str()),
                None => Err(crate::i18n::t("m003")),
            };
            let projected = self.timeline.as_ref().map(timeline::projection::project);
            crate::keyboard::show(
                ui,
                view,
                projected.as_ref().and_then(|view| view.pitch_range()),
            );
            crate::scope_ui::show(ui, self.control.as_ref().and_then(Control::scope));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("execution_pane")
                .show(ui, |ui| {
                    if !self.timeline_error.is_empty() {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            crate::i18n::render(
                                "f003",
                                &[("arg0", self.timeline_error.to_string())],
                            ),
                        );
                    } else if let (Some(snapshot), Some(projected)) = (&self.timeline, &projected) {
                        timeline::show(ui, snapshot, projected);
                    } else {
                        ui.heading(crate::i18n::t("m002"));
                        ui.label(crate::i18n::t("m003"));
                    }
                    egui::CollapsingHeader::new(crate::i18n::t("m004"))
                        .id_salt("m004")
                        .show(ui, |ui| {
                            egui::ScrollArea::both().max_height(190.0).show(ui, |ui| {
                                ui.monospace(crate::snippets::localize_builtin(TIMELINE_READ));
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
                            ui.monospace("globalThis.surgeId");
                            ui.label(crate::i18n::t("comment_13"));
                        });
                    ui.separator();
                    if logs::show(ui, &self.log_path).is_err() {
                        self.error = crate::i18n::t("log_directory_open_error").into();
                    }
                });
        });
        if let Some(control) = &mut self.control {
            if control.ready && !control.stopping && !self.snippet_execution.is_pending() {
                self.script_ui.show(&ctx, control);
            }
        }
        self.startup.show_overlay(&ctx);
        self.snippet_execution.show_overlay(&ctx);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(control) = &mut self.control {
            let _ = control.close();
        }
    }
}
