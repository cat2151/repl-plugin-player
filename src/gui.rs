use crate::{
    config,
    control::{Control, INIT},
    log::Log,
};
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

mod console;
mod dock;
pub(crate) use dock::regression::run as dock_regression;
pub(crate) use dock::script_regression::run as dock_script_ui_regression;
mod help;
mod logs;
mod panes;
mod patch;
mod player_download;
mod settings;
mod snippet_execution;
mod startup;

/// The main window's title.
const TITLE: &str = "REPL Plugin Player";

pub fn run() -> Result<(), String> {
    let config_path = config::path();
    let (config, config_errors) = match &config_path {
        Ok(path) => config::read(path),
        Err(error) => (config::Config::default(), vec![error.clone()]),
    };
    let settings = settings::Settings::new(crate::status::path(), config_path);
    let auto = settings.layout_auto;
    let layout = (crate::resources::user_directory().ok())
        .map(|directory| dock::persist::LayoutFile::open(&directory, auto));
    let main = layout
        .as_ref()
        .map_or(config.window, |(_, layout)| layout.main);
    let mut viewport = egui::ViewportBuilder::default().with_inner_size(main.size);
    if let Some(position) = main.position {
        viewport = viewport.with_position(position);
    }
    eframe::run_native(
        TITLE,
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::fonts(&cc.egui_ctx);
            Ok(Box::new(Editor::new(config_errors, settings, layout)))
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
    layout: dock::Layout,
    /// The layout's source, chosen in the settings; `None` keeps the layout in memory
    /// only.
    layout_file: Option<dock::persist::LayoutFile>,
    config_errors: Vec<String>,
    settings: settings::Settings,
    help: help::Help,
    player_download: player_download::Offer,
}
impl Editor {
    fn new(
        config_errors: Vec<String>,
        settings: settings::Settings,
        layout: Option<(dock::persist::LayoutFile, dock::Layout)>,
    ) -> Self {
        let (layout_file, layout) = match layout {
            Some((file, layout)) => (Some(file), layout),
            None => (None, dock::initial()),
        };
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
            layout,
            layout_file,
            config_errors,
            settings,
            help: help::Help::default(),
            player_download: player_download::Offer::default(),
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
    /// The queued run, once its overlay has been presented.
    fn run_queued(&mut self, ctx: &egui::Context) {
        if let Some(request) = self.snippet_execution.take_ready(ctx) {
            let _ = self.execute(request);
            self.write_browsed_cell(ctx);
        }
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
        self.run_queued(&ctx);
        self.player_download.update();
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
        let active = self.snippet_execution.is_pending()
            || ctx.input(|input| !input.events.is_empty() || input.pointer.any_down());
        if let Some(control) = &mut self.control {
            if let Err(error) = control.refresh_catalog_when_idle(active) {
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
        self.dock(ui, preparing);
        self.startup.show_overlay(&ctx);
        self.snippet_execution.show_overlay(&ctx);
        self.settings.show(&ctx);
        if let Some(file) = self.layout_file.as_mut() {
            if self.settings.take_layout_switch() {
                file.switch(&mut self.layout);
            }
        }
        self.help.show(&ctx);
        self.player_download.show(&ctx);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(file) = &mut self.layout_file {
            file.save(&self.layout);
        }
        if let Some(control) = &mut self.control {
            let _ = control.close();
        }
    }
}
