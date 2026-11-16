use crate::{bridge::Bridge, catalog_refresh::Refresh, log::Log, resources::Source};
use serde_json::{json, Value};
mod tasks;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub static TIMELINE_READ: Source =
    Source::new("src/timeline-read.js", include_str!("timeline-read.js"));
static INSTRUMENT: Source = Source::new("src/instrument.js", include_str!("instrument.js"));
static SCRIPT_API: Source = Source::new("src/script-api.js", include_str!("script-api.js"));
/// The startup cell: prepares the first instrument before Ready.
pub const INIT: &str = "include('builtin/init.js')";

pub struct RunResult {
    pub error: bool,
    /// Which run it was: scope and first line.
    pub header: String,
    pub body: String,
}

pub struct Control {
    bridge: Bridge,
    pub log: Log,
    pub ready: bool,
    /// Every run's result, oldest first.
    pub results: Vec<RunResult>,
    full_scan: bool,
    pub missing_surge: bool,
    gui_mode: bool,
    init_started: bool,
    init: String,
    pub stopping: bool,
    closed: bool,
    prepare_deadline: crate::preparation::Deadline,
    readiness_window: Option<(Instant, u64)>,
    readiness_draining: bool,
    refresh: Refresh,
}
impl Control {
    pub fn enable_gui(&mut self) {
        self.gui_mode = true;
        crate::script_ui::enable(true);
    }
    pub fn retry_surge(&mut self) -> Result<(), String> {
        self.bridge.call(json!({"command":"scan"}))?;
        self.full_scan = true;
        self.refresh = Refresh::Done;
        self.missing_surge = false;
        self.prepare_deadline.reset(Instant::now());
        Ok(())
    }
    pub fn parameter_safety(&self, instance: i32, parameter: i32) -> Result<Value, String> {
        self.bridge.call(
            json!({"command":"parameter-safety", "instanceId":instance, "parameterId":parameter}),
        )
        .map_err(|error| crate::i18n::render("ui_target", &[("instance", instance.to_string()), ("parameter", parameter.to_string()), ("error", error)]))
    }
    pub fn creation_requested(&self) -> bool {
        self.init_started
    }
    pub fn pump_cli_messages(&self) -> Result<(), String> {
        let result = self.bridge.call(json!({"command":"cli-pump"}))?;
        if result["quit"] == true {
            return Err(crate::i18n::t("m065").into());
        }
        Ok(())
    }
    /// `init` is the cell evaluated once the plugin catalog is scanned.
    pub fn open(mut log: Log, init: &str) -> Result<Self, String> {
        let dll = crate::resources::bridge()?;
        let mut bridge = Bridge::load(&dll)?;
        let data = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(crate::i18n::t("m064"))?)
            .join("repl-plugin-player/uapmd");
        std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
        let args =
            json!({"command":"open","data":data,"diagnostics":log.path.with_extension("native")});
        log.event("startup", json!({"api":"AppModel::instantiate / UapmdJSRuntime::ensureApiBootstrapped","arguments":args,"surgeDataHome":std::env::var_os("SURGE_DATA_HOME").map(|s|s.to_string_lossy().into_owned()),"surgeDataHomeError":crate::surge_home::error(),"defaultSurgeData":"C:/ProgramData/Surge XT","templatesExist":PathBuf::from("C:/ProgramData/Surge XT/patches_factory/Templates").is_dir()}))?;
        let opened = bridge.call(args);
        log.event("bootstrap", reply_value(&opened))?;
        let opened = opened?;
        let sample_rate = opened["sampleRate"].as_f64().unwrap_or(48_000.0) as u32;
        bridge.enable_scope(sample_rate)?;
        if opened["bootstrap"] != "\"function\"" {
            return Err(crate::i18n::t("m046").into());
        }
        bridge.call(json!({"command":"eval","code":INSTRUMENT.text()?}))?;
        bridge.call(json!({"command":"eval","code":SCRIPT_API.text()?}))?;
        let refresh = if opened["catalogFromCache"] == true {
            Refresh::pending()
        } else {
            Refresh::Done
        };
        Ok(Self {
            bridge,
            log,
            ready: false,
            results: Vec::new(),
            full_scan: false,
            missing_surge: false,
            gui_mode: false,
            init_started: false,
            init: init.into(),
            stopping: false,
            closed: false,
            prepare_deadline: crate::preparation::Deadline::new(Instant::now()),
            readiness_window: None,
            readiness_draining: false,
            refresh,
        })
    }
    pub fn eval(&mut self, code: &str, reason: &str) -> Result<String, String> {
        self.finish_refresh()?;
        let result = self.bridge.call(json!({"command":"eval","code":code}));
        self.log.event(
            "evaluation",
            json!({"code":code,"reason":reason,"result":reply_value(&result)}),
        )?;
        let events = self
            .bridge
            .call(json!({"command":"eval","code":"globalThis.__replEvents.splice(0)"}))?;
        let events: Value =
            serde_json::from_str(events["result"].as_str().ok_or(crate::i18n::t("m047"))?)
                .map_err(|e| e.to_string())?;
        for event in events.as_array().ok_or(crate::i18n::t("m048"))? {
            self.log
                .event("api", json!({"call":event,"reason":reason}))?;
        }
        result?
            .get("result")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| crate::i18n::t("m049").into())
    }
    pub fn observe(&mut self, window: &str) -> Result<Value, String> {
        let result = self.bridge.call(json!({"command":"observe"}))?;
        self.log
            .event("observation", json!({"window":window,"values":result}))?;
        Ok(result)
    }
    pub fn read_timeline(&self) -> Result<Value, String> {
        // Public clip data plus private, read-only playback/display metadata.
        let reply = self
            .bridge
            .call(json!({"command":"timeline-view", "code":TIMELINE_READ.text()?}))?;
        let mut snapshot: Value =
            serde_json::from_str(reply["result"].as_str().ok_or(crate::i18n::t("m050"))?)
                .map_err(|error| error.to_string())?;
        snapshot["view"] = reply["view"].clone();
        Ok(snapshot)
    }
    pub fn read_notes(&self) -> Result<Vec<u8>, String> {
        let reply = self.bridge.call(json!({"command":"notes"}))?;
        reply["notes"]
            .as_array()
            .ok_or(crate::i18n::t("m052"))?
            .iter()
            .map(|pitch| {
                pitch
                    .as_u64()
                    .filter(|pitch| *pitch < 128)
                    .map(|pitch| pitch as u8)
                    .ok_or_else(|| crate::i18n::t("m053").into())
            })
            .collect()
    }
    pub fn scope(&self) -> Option<&crate::scope::Scope> {
        self.bridge.scope()
    }
    pub fn poll(&mut self) -> Result<(), String> {
        let state = self.bridge.call(json!({"command":"poll"}))?;
        self.observe_refresh(&state)?;
        if self.ready && !self.stopping {
            self.poll_tasks()?;
        }
        if self.missing_surge {
            return Ok(());
        }
        if !self.ready {
            if self.prepare_deadline.expired(Instant::now()) {
                return Err(crate::i18n::t("m054").into());
            }
            let error = state["preparationError"].as_str().unwrap_or("");
            if !error.is_empty() {
                return Err(crate::i18n::render("f015", &[("error", error.to_string())]));
            }
            if !self.init_started && state["scanned"] == true {
                let scan_error = state["scanError"].as_str().unwrap_or("");
                if !scan_error.is_empty() {
                    return Err(crate::i18n::render(
                        "f016",
                        &[("scan_error", scan_error.into())],
                    ));
                }
                self.init_started = true;
                match self.evaluate_init()? {
                    Init::Prepared => {
                        // Upstream can leave a stop flush pending while its engine is
                        // inactive. Process silence through the official engine before
                        // Ready, then drain/stop it. Otherwise that flush can discard the
                        // first script's NoteOn in its first active audio block.
                        self.eval(
                            "uapmd.audio.setEngineEnabled(true)",
                            "preparation: consume pending graph/stop flush",
                        )?;
                        let observed = self.observe("preparation silent readiness baseline")?;
                        self.readiness_window = Some((
                            Instant::now(),
                            observed["blocks"].as_u64().ok_or(crate::i18n::t("m058"))?,
                        ));
                    }
                    Init::MissingPlugin if !self.full_scan => {
                        self.bridge.call(json!({"command":"scan"}))?;
                        self.full_scan = true;
                        self.refresh = Refresh::Done;
                        self.init_started = false;
                        self.log.event(
                            "scan_started",
                            json!({"api":"AppModel::performPluginScanning","fastOnly":false}),
                        )?;
                    }
                    Init::MissingPlugin if self.gui_mode => {
                        self.init_started = false;
                        self.missing_surge = true;
                        self.prepare_deadline.pause();
                        return Ok(());
                    }
                    Init::MissingPlugin => return Err(crate::i18n::t("install_cli_missing").into()),
                }
            }
            if let Some((started, blocks)) = self.readiness_window {
                let observed = self.observe("preparation silent readiness baseline")?;
                if observed["finite"] != true
                    || observed["engineEnabled"] != true
                    || observed["dispatcherPlaying"] != true
                    || observed["peak"].as_f64().ok_or(crate::i18n::t("m059"))? > 1e-6
                {
                    return Err(crate::i18n::t("m060").into());
                }
                if started.elapsed() >= Duration::from_millis(200)
                    && observed["blocks"].as_u64().ok_or(crate::i18n::t("m058"))? > blocks + 3
                {
                    self.stop("preparation: drain and stop before Ready")?;
                    self.readiness_window = None;
                    self.readiness_draining = true;
                }
            }
        }
        if self.stopping {
            let observed = self.bridge.call(json!({"command":"observe"}))?;
            if observed["dispatcherPlaying"] == false {
                self.stopping = false;
                self.log.event("engine_stopped", observed)?;
                if self.readiness_draining {
                    self.readiness_draining = false;
                    self.ready = true;
                    self.log.event(
                        "ready",
                        json!({"readiness":"silent realtime progress and engine drain completed"}),
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn stop_script(&self) -> String {
        // Stopping a stopped transport leaves its all-notes-off flush pending until the next
        // engine start, where it discards a Note On at position 0 in the same block.
        "if (uapmd.timeline.getState().isPlaying) uapmd.sequencer.stopPlayback();\n\
         for (const id of uapmd.sequencer.getInstanceIds()) uapmd.sequencer.sendNoteOff(id, 60);\n\
         uapmd.audio.setEngineEnabled(false);"
            .into()
    }
    pub fn stop(&mut self, reason: &str) -> Result<(), String> {
        self.eval(&self.stop_script(), reason)?;
        self.stopping = true;
        Ok(())
    }
    pub fn run(&mut self, code: &str) -> Result<(), String> {
        self.run_request(&crate::execution::request(
            code,
            0,
            0,
            crate::execution::Scope::All,
        ))
    }
    pub fn run_request(&mut self, request: &crate::execution::Request) -> Result<(), String> {
        self.run_request_result(request).map(|_| ())
    }
    pub fn run_request_result(
        &mut self,
        request: &crate::execution::Request,
    ) -> Result<String, String> {
        if !self.ready || self.stopping {
            return Err(crate::i18n::t("m061").into());
        }
        self.log.execution += 1;
        self.log.event(
            "run_input",
            json!({"scope":request.scope,"code":request.code,
                "characterRange":[request.chars.start, request.chars.end],
                "sourceStartLine":request.first_line}),
        )?;
        let reason = format!("Run: {}", request.scope);
        let result = self.eval(&request.code, &reason);
        let (key, text) = match &result {
            Ok(value) => ("f018", value),
            Err(error) => ("f019", error),
        };
        // The console shows a string result as its text, with its own line breaks.
        let body = serde_json::from_str::<String>(text).unwrap_or_else(|_| text.clone());
        self.results.push(RunResult {
            error: result.is_err(),
            header: crate::i18n::render(
                key,
                &[
                    ("arg0", crate::i18n::scope(request.scope).to_owned()),
                    ("arg1", format!("{}", request.first_line)),
                ],
            ),
            body,
        });
        result
    }
    pub fn read_plugin_state(
        &mut self,
        instance: i64,
        deadline: Instant,
    ) -> Result<Vec<u8>, String> {
        self.bridge
            .call(json!({"command":"request-state", "instanceId":instance}))?;
        loop {
            if Instant::now() >= deadline {
                return Err(crate::i18n::t("m062").into());
            }
            let status = self.bridge.call(json!({"command":"poll"}))?;
            if let Some(error) = status["preparationError"]
                .as_str()
                .filter(|s| !s.is_empty())
            {
                return Err(error.to_owned());
            }
            if status["stateReady"] == true {
                let state = self.bridge.call(json!({"command":"state"}))?;
                return decode_hex(state["hex"].as_str().ok_or(crate::i18n::t("m057"))?);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        self.stop("normal exit")?;
        let result = self.bridge.call(json!({"command":"close"}));
        crate::patch_jobs::cleanup();
        self.closed = true;
        if self.gui_mode {
            crate::script_ui::enable(false);
        }
        self.log.event("cleanup", json!({"api":"unregister listeners / destroy runtime / AppModel::cleanupInstance / restore event loop","result":reply_value(&result)}))?;
        result.map(|_| ())
    }
}
impl Control {
    /// Starts the one scan that refreshes a cached catalog, once `active` (input or
    /// pending work) has been false for a while after Ready.
    pub fn refresh_catalog_when_idle(&mut self, active: bool) -> Result<(), String> {
        if !self.ready || self.stopping || !self.refresh.should_start(active, Instant::now()) {
            return Ok(());
        }
        self.bridge.call(json!({"command":"refresh"}))?;
        self.log.event(
            "catalog_refresh_started",
            json!({"api":"AppModel::notifyUiReady / notifyPersistentStorageReady"}),
        )
    }
    fn observe_refresh(&mut self, state: &Value) -> Result<(), String> {
        if self.refresh != Refresh::Running || state["scanned"] != true {
            return Ok(());
        }
        self.refresh = Refresh::Done;
        // A failed refresh leaves the cached catalog in place.
        self.log.event(
            "catalog_refresh_finished",
            json!({"catalogCount":state["catalogCount"],"scanError":state["scanError"]}),
        )
    }
    /// The scan replaces the host's catalog on its own thread; scripts read and
    /// instantiate from that catalog, so they wait for it.
    fn finish_refresh(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.refresh == Refresh::Running {
            if Instant::now() >= deadline {
                return Err(crate::i18n::t("catalog_refresh_timeout").into());
            }
            let state = self.bridge.call(json!({"command":"poll"}))?;
            self.observe_refresh(&state)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}
enum Init {
    Prepared,
    MissingPlugin,
}
impl Control {
    /// Evaluates the startup cell; a plugin absent from the catalog is told
    /// apart from other failures by the error the library marks.
    fn evaluate_init(&mut self) -> Result<Init, String> {
        let code = format!(
            "(() => {{ try {{ (0, eval)({}); return JSON.stringify({{}}); }} catch (e) {{ \
             return JSON.stringify({{error: e instanceof Error ? e.message : String(e), \
             missing: !!(e && e.missingPlugin)}}); }} }})()",
            serde_json::to_string(&self.init).map_err(|e| e.to_string())?
        );
        let reply: Value = serde_json::from_str::<String>(&self.eval(&code, "startup instrument")?)
            .and_then(|text| serde_json::from_str(&text))
            .map_err(|e| e.to_string())?;
        self.log
            .event("init_evaluated", json!({"code":self.init,"result":reply}))?;
        match reply["error"].as_str() {
            None => Ok(Init::Prepared),
            Some(_) if reply["missing"] == true => Ok(Init::MissingPlugin),
            Some(error) => Err(crate::i18n::render("f015", &[("error", error.into())])),
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return Err(crate::i18n::t("m063").into());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
fn reply_value(result: &Result<Value, String>) -> Value {
    match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    }
}
