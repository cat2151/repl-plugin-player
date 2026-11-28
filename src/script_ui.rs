//! Host callbacks only register data. The normal egui frame accesses parameters.
use crate::i18n::{render, t};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
};

mod view;
pub use view::Renderer;
pub mod regression;
pub mod surface;

#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub instance: i32,
    pub parameter: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub binding: Option<Binding>,
}
#[derive(Clone, Debug)]
pub struct Window {
    pub id: String,
    pub title: String,
    pub widgets: Vec<Widget>,
    pub revision: u64,
}
/// A modal notice with only an OK button, shown in the order given.
#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub title: String,
    pub text: String,
}
#[derive(Default)]
struct Registry {
    enabled: bool,
    recording_monitor: bool,
    revision: u64,
    windows: BTreeMap<String, Window>,
    dialogs: VecDeque<Dialog>,
}
thread_local! { static REGISTRY: RefCell<Registry> = RefCell::default(); }

pub fn enable(enabled: bool) {
    surface::clear();
    REGISTRY.with_borrow_mut(|registry| {
        registry.enabled = enabled;
        if registry.recording_monitor {
            crate::recording::close_monitor();
        }
        registry.recording_monitor = false;
        registry.windows.clear();
        registry.dialogs.clear();
    });
}

fn string(input: &Value, key: &str) -> Result<String, String> {
    input[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .map(str::to_owned)
        .ok_or_else(|| render("ui_string", &[("key", key.into())]))
}

fn identifier(input: &Value, key: &str) -> Result<i32, String> {
    input[key]
        .as_i64()
        .filter(|n| (0..=i64::from(i32::MAX)).contains(n))
        .map(|n| n as i32)
        .ok_or_else(|| render("ui_integer", &[("key", key.into())]))
}

fn definition(input: &Value) -> Result<Window, String> {
    let id = string(input, "id")?;
    let title = string(input, "title")?;
    let widgets = input["widgets"]
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or(t("ui_widgets"))?;
    let mut ids = BTreeSet::new();
    let widgets = widgets
        .iter()
        .map(|value| {
            let id = string(value, "id")?;
            if !ids.insert(id.clone()) {
                return Err(t("ui_duplicate").into());
            }
            let kind = string(value, "type")?;
            if !matches!(kind.as_str(), "label" | "slider" | "checkbox") {
                return Err(t("ui_type").into());
            }
            let label = string(value, "label")?;
            let binding = if kind == "label" {
                None
            } else {
                Some(Binding {
                    instance: identifier(&value["binding"], "instanceId")?,
                    parameter: identifier(&value["binding"], "parameterId")?,
                })
            };
            Ok(Widget {
                id,
                kind,
                label,
                binding,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Window {
        id,
        title,
        widgets,
        revision: 0,
    })
}

pub fn dispatch(api: &str, input: &Value) -> Result<Value, String> {
    REGISTRY.with_borrow_mut(|registry| {
        if !registry.enabled {
            return Err(t("ui_gui_only").into());
        }
        match api {
            "ui.recordingMonitor.show" => {
                if !registry.recording_monitor {
                    crate::recording::open_monitor()?;
                }
                registry.recording_monitor = true;
                Ok(Value::Null)
            }
            "ui.show" => {
                let mut window = definition(&input["definition"])?;
                if registry.windows.len() >= 32 && !registry.windows.contains_key(&window.id) {
                    return Err(t("ui_limit").into());
                }
                registry.revision += 1;
                window.revision = registry.revision;
                registry.windows.insert(window.id.clone(), window);
                Ok(Value::Null)
            }
            "ui.close" => {
                let id = string(input, "id")?;
                registry.windows.remove(&id);
                surface::close(&id);
                Ok(Value::Null)
            }
            "ui.surface.show" => surface::register(&input["definition"]),
            "ui.surface.poll" => Ok(surface::poll(&string(input, "id")?)),
            "ui.surface.setTitle" => {
                let instance = input["padGainInstance"]
                    .as_i64()
                    .and_then(|id| i32::try_from(id).ok())
                    .filter(|&id| id >= 0);
                surface::set_title(&string(input, "id")?, string(input, "title")?, instance);
                Ok(Value::Null)
            }
            "ui.dialog" => {
                registry.dialogs.push_back(Dialog {
                    title: string(input, "title")?,
                    text: string(input, "text")?,
                });
                Ok(Value::Null)
            }
            _ => Err(t("ui_type").into()),
        }
    })
}

pub fn recording_monitor_open() -> bool {
    REGISTRY.with_borrow(|registry| registry.recording_monitor)
}

pub fn close_recording_monitor() {
    REGISTRY.with_borrow_mut(|registry| registry.recording_monitor = false);
    crate::recording::close_monitor();
}

/// Every registered window, copied so that no borrow is held while entering JS.
pub fn snapshot() -> Vec<Window> {
    REGISTRY.with_borrow(|r| r.windows.values().cloned().collect())
}
/// The oldest dialog not yet shown.
pub fn take_dialog() -> Option<Dialog> {
    REGISTRY.with_borrow_mut(|r| r.dialogs.pop_front())
}
/// Removes the window unless a later `ui.show` has replaced it.
pub fn close(window: &Window) {
    REGISTRY.with_borrow_mut(|r| {
        if r.windows
            .get(&window.id)
            .is_some_and(|w| w.revision == window.revision)
        {
            r.windows.remove(&window.id);
        }
    });
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub fingerprint: Value,
    pub min: f64,
    pub max: f64,
    pub discrete: bool,
    pub readonly: bool,
    pub value: f64,
}

pub trait Parameters {
    fn parameter(&mut self, binding: &Binding) -> Result<Parameter, String>;
    fn set(&mut self, binding: &Binding, value: f64) -> Result<(), String>;
}

impl Parameters for crate::control::Control {
    fn parameter(&mut self, binding: &Binding) -> Result<Parameter, String> {
        let info = self.parameter_safety(binding.instance, binding.parameter)?;
        let value = self.eval(
            &format!(
                "uapmd.instance({}).getParameterValue({})",
                binding.instance, binding.parameter
            ),
            "script UI read",
        )?;
        let value: f64 = serde_json::from_str(&value).map_err(|e| e.to_string())?;
        let min = info["min"].as_f64().ok_or(t("ui_metadata"))?;
        let max = info["max"].as_f64().ok_or(t("ui_metadata"))?;
        if !min.is_finite()
            || !max.is_finite()
            || min >= max
            || !value.is_finite()
            || value < min
            || value > max
        {
            return Err(t("ui_metadata").into());
        }
        Ok(Parameter {
            min,
            max,
            value,
            readonly: info["readonly"] != false,
            discrete: info["discrete"] == true,
            fingerprint: info,
        })
    }
    fn set(&mut self, binding: &Binding, value: f64) -> Result<(), String> {
        let current = self.parameter(binding)?;
        if current.readonly
            || !value.is_finite()
            || value < current.min
            || value > current.max
            || (current.discrete && value.fract() != 0.0)
        {
            return Err(t("ui_metadata").into());
        }
        self.eval(
            &format!(
                "uapmd.instance({}).setParameterValue({}, {})",
                binding.instance, binding.parameter, value
            ),
            "script UI write",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
