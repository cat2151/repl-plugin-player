use crate::{bridge::Bridge, log::Log};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub const TIMELINE_READ: &str = include_str!("timeline-read.js");
pub const SURGE: &str = "org.surge-synth-team.surge-xt";

pub struct Control {
    bridge: Bridge,
    pub log: Log,
    pub ready: bool,
    pub message: String,
    pub instance: i64,
    full_scan: bool,
    pub missing_surge: bool,
    gui_mode: bool,
    create_started: bool,
    target: String,
    pub stopping: bool,
    closed: bool,
    prepare_deadline: crate::preparation::Deadline,
    readiness_window: Option<(Instant, u64)>,
    readiness_draining: bool,
}
impl Control {
    pub fn enable_gui(&mut self) {
        self.gui_mode = true;
        crate::script_ui::enable(true);
    }
    pub fn retry_surge(&mut self) -> Result<(), String> {
        self.bridge.call(json!({"command":"scan"}))?;
        self.full_scan = true;
        self.missing_surge = false;
        self.prepare_deadline.reset(Instant::now());
        self.message = crate::i18n::t("m043").into();
        Ok(())
    }
    pub fn parameter_safety(&self, instance: i32, parameter: i32) -> Result<Value, String> {
        self.bridge.call(
            json!({"command":"parameter-safety", "instanceId":instance, "parameterId":parameter}),
        )
        .map_err(|error| crate::i18n::render("ui_target", &[("instance", instance.to_string()), ("parameter", parameter.to_string()), ("error", error)]))
    }
    pub fn creation_requested(&self) -> bool {
        self.create_started
    }
    pub fn pump_cli_messages(&self) -> Result<(), String> {
        let result = self.bridge.call(json!({"command":"cli-pump"}))?;
        if result["quit"] == true {
            return Err(crate::i18n::t("m065").into());
        }
        Ok(())
    }
    pub fn open(mut log: Log, target: &str) -> Result<Self, String> {
        let dll = crate::resources::bridge()?;
        let mut bridge = Bridge::load(&dll)?;
        let data = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(crate::i18n::t("m064"))?)
            .join("repl-plugin-player/uapmd");
        std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
        let args =
            json!({"command":"open","data":data,"diagnostics":log.path.with_extension("native")});
        log.event("startup", json!({"api":"AppModel::instantiate / UapmdJSRuntime::ensureApiBootstrapped","arguments":args,"surgeDataHome":std::env::var_os("SURGE_DATA_HOME").map(|s|s.to_string_lossy().into_owned()),"defaultSurgeData":"C:/ProgramData/Surge XT","templatesExist":PathBuf::from("C:/ProgramData/Surge XT/patches_factory/Templates").is_dir()}))?;
        let opened = bridge.call(args);
        log.event("bootstrap", reply_value(&opened))?;
        let opened = opened?;
        let sample_rate = opened["sampleRate"].as_f64().unwrap_or(48_000.0) as u32;
        bridge.enable_scope(sample_rate)?;
        if opened["bootstrap"] != "\"function\"" {
            return Err(crate::i18n::t("m046").into());
        }
        bridge.call(json!({"command":"eval","code":include_str!("instrument.js")}))?;
        bridge.call(json!({"command":"eval","code":include_str!("script-api.js")}))?;
        Ok(Self {
            bridge,
            log,
            ready: false,
            message: crate::i18n::t("m043").into(),
            instance: -1,
            full_scan: false,
            missing_surge: false,
            gui_mode: false,
            create_started: false,
            target: target.into(),
            stopping: false,
            closed: false,
            prepare_deadline: crate::preparation::Deadline::new(Instant::now()),
            readiness_window: None,
            readiness_draining: false,
        })
    }
    pub fn eval(&mut self, code: &str, reason: &str) -> Result<String, String> {
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
            self.log.event(
                "api",
                json!({"call":event,"reason":reason,"instance":self.instance,"track":0}),
            )?;
        }
        result?
            .get("result")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| crate::i18n::t("m049").into())
    }
    pub fn observe(&mut self, window: &str) -> Result<Value, String> {
        let result = self.bridge.call(json!({"command":"observe"}))?;
        self.log.event(
            "observation",
            json!({"window":window,"instance":self.instance,"track":0,"values":result}),
        )?;
        Ok(result)
    }
    pub fn read_timeline(&self) -> Result<Value, String> {
        // Public clip data plus private, read-only playback/display metadata.
        let reply = self
            .bridge
            .call(json!({"command":"timeline-view", "code":TIMELINE_READ}))?;
        let mut snapshot: Value =
            serde_json::from_str(reply["result"].as_str().ok_or(crate::i18n::t("m050"))?)
                .map_err(|error| error.to_string())?;
        snapshot["view"] = reply["view"].clone();
        Ok(snapshot)
    }
    pub fn read_notes(&self) -> Result<Vec<u8>, String> {
        let reply = self.bridge.call(json!({"command":"notes"}))?;
        if reply["available"] != true {
            return Err(crate::i18n::t("m051").into());
        }
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
    pub fn loop_trace(&self, start: bool) -> Result<Value, String> {
        self.bridge
            .call(json!({"command":"loop-trace", "start":start}))
    }
    pub fn loop_checks(&self) -> Result<(), String> {
        self.bridge.call(json!({"command":"loop-checks"}))?;
        Ok(())
    }
    pub fn poll(&mut self) -> Result<(), String> {
        let state = self.bridge.call(json!({"command":"poll"}))?;
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
            if !self.create_started && state["scanned"] == true {
                let scan_error = state["scanError"].as_str().unwrap_or("");
                let catalog = self.eval("Array.from({length:uapmd.catalog.getCount()}, (_,i)=>uapmd.catalog.getPluginAt(i))", "catalog selection")?;
                let catalog: Value = serde_json::from_str(&catalog).map_err(|e| e.to_string())?;
                let decision =
                    crate::preparation::decide(&catalog, &self.target, self.full_scan, scan_error)?;
                if let crate::preparation::ScanDecision::Create(selected) = &decision {
                    self.log.event(
                        "catalog_selected",
                        json!({"api":"uapmd.catalog.getPluginAt","plugin":selected}),
                    )?;
                    self.log.event("create_started", json!({"api":"AppModel::createPluginInstanceAsync","format":"CLAP","pluginId":self.target,"track":0}))?;
                    self.bridge
                        .call(json!({"command":"create","pluginId":self.target}))?;
                    self.create_started = true;
                } else if matches!(decision, crate::preparation::ScanDecision::FullScan) {
                    self.bridge.call(json!({"command":"scan"}))?;
                    self.full_scan = true;
                    self.log.event(
                        "scan_started",
                        json!({"api":"AppModel::performPluginScanning","fastOnly":false}),
                    )?;
                } else if self.gui_mode && self.target == SURGE {
                    self.missing_surge = true;
                    self.prepare_deadline.pause();
                    self.message = crate::i18n::t("install_missing").into();
                    return Ok(());
                } else if self.target == SURGE {
                    return Err(crate::i18n::t("install_cli_missing").into());
                } else {
                    return Err(crate::i18n::render(
                        "f017",
                        &[("arg0", self.target.clone())],
                    ));
                }
            }
            if state["stateReady"] == true
                && self.readiness_window.is_none()
                && !self.readiness_draining
            {
                self.instance = state["instanceId"].as_i64().ok_or(crate::i18n::t("m056"))?;
                self.log.event("create_completed", json!({"api":"AppModel::createPluginInstanceAsync / AudioPluginInstanceAPI::requestState","instance":self.instance,"track":0}))?;
                let bytes = self.bridge.call(json!({"command":"state"}))?;
                let state = decode_hex(bytes["hex"].as_str().ok_or(crate::i18n::t("m057"))?)?;
                let info = crate::surge_state::verify(&state)?;
                self.log.event("init_saw_verified", json!({"api":"AudioPluginInstanceAPI::requestState","instance":self.instance,"source":"actual newly created CLAP state","evidence":info,"bytes":state.len()}))?;
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
                    self.message = crate::i18n::t("m044").into();
                    self.log.event("ready",json!({"instance":self.instance,"track":0,"readiness":"silent realtime progress and engine drain completed"}))?;
                } else {
                    self.message = crate::i18n::t("m037").into();
                }
            }
        }
        Ok(())
    }
    pub fn stop_script(&self) -> String {
        let note_off = if self.instance >= 0 {
            format!("uapmd.sequencer.sendNoteOff({}, 60);\n", self.instance)
        } else {
            String::new()
        };
        format!("uapmd.sequencer.stopPlayback();\n{note_off}uapmd.audio.setEngineEnabled(false);")
    }
    pub fn stop(&mut self, reason: &str) -> Result<(), String> {
        self.eval(&self.stop_script(), reason)?;
        self.stopping = true;
        self.message = crate::i18n::t("m045").into();
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
        match self.eval(&request.code, &reason) {
            Ok(result) => {
                self.message = crate::i18n::render(
                    "f018",
                    &[
                        ("arg0", crate::i18n::scope(request.scope).to_owned()),
                        ("arg1", format!("{}", request.first_line)),
                        ("result", result.to_string()),
                    ],
                );
                Ok(result)
            }
            Err(error) => {
                self.message = crate::i18n::render(
                    "f019",
                    &[
                        ("arg0", crate::i18n::scope(request.scope).to_owned()),
                        ("arg1", format!("{}", request.first_line)),
                        ("error", error.to_string()),
                    ],
                );
                Err(error)
            }
        }
    }
    pub fn read_plugin_state(
        &mut self,
        instance: Option<i64>,
        deadline: Instant,
    ) -> Result<Vec<u8>, String> {
        self.bridge.call(
            json!({"command":"request-state", "instanceId":instance.unwrap_or(self.instance)}),
        )?;
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
        self.closed = true;
        if self.gui_mode {
            crate::script_ui::enable(false);
        }
        self.log.event("cleanup", json!({"api":"unregister listeners / destroy runtime / AppModel::cleanupInstance / restore event loop","result":reply_value(&result)}))?;
        result.map(|_| ())
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
