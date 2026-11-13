//! `repl.patchBrowser`: the player's patch browser as events for a script, which
//! auditions and restores sounds itself. The last selection the script accepted
//! becomes one cell, written by the GUI after the script returns.
use crate::{
    i18n::t,
    patch_browser::{self, Event, Link},
    resident_ipc::{self, Selected},
    snippets::Snippet,
};
use serde_json::{json, Value};
use std::{cell::RefCell, path::PathBuf};

struct Browser {
    link: Option<Link>,
    start: Start,
    /// The track being browsed, which the chosen patch's cell sets.
    track: i64,
    /// The cell of the selection awaiting its answer.
    offered: Option<Snippet>,
    /// The cell of the last selection answered without an error.
    accepted: Option<Snippet>,
}

thread_local! {
    static BROWSER: RefCell<Browser> = RefCell::new(Browser {
        link: None,
        start: start_player,
        track: 0,
        offered: None,
        accepted: None,
    });
}

/// Connects to a newly started player.
pub type Start = fn() -> Result<Link, String>;

fn start_player() -> Result<Link, String> {
    Link::start(&patch_browser::player()?, patch_browser::spawn)
}

/// Later players on this thread come from `start` instead of the real one.
pub fn start_with(start: Start) {
    BROWSER.with_borrow_mut(|b| b.start = start);
}

pub fn dispatch(api: &str, input: &Value) -> Result<Value, String> {
    BROWSER.with_borrow_mut(|browser| match api {
        "patchBrowser.open" => {
            let track = input["track"]
                .as_i64()
                .ok_or_else(|| t("patch_browse_02"))?;
            browser.open(track).map(|()| Value::Null)
        }
        "patchBrowser.poll" => Ok(Value::Array(browser.poll())),
        "patchBrowser.answer" => {
            let result = match &input["error"] {
                Value::Null => Ok(()),
                error => Err(error.as_str().map_or(error.to_string(), str::to_owned)),
            };
            browser.answer(result).map(|()| Value::Null)
        }
        other => Err(crate::i18n::render("f027", &[("other", other.to_string())])),
    })
}

/// The cell for the selection last accepted since the browser was opened; once.
pub fn take_cell() -> Option<Snippet> {
    BROWSER.with_borrow_mut(|b| b.accepted.take())
}

impl Browser {
    fn open(&mut self, track: i64) -> Result<(), String> {
        self.track = track;
        self.offered = None;
        self.accepted = None;
        if let Some(link) = self.link.as_mut() {
            // Anything left from an earlier browse that ended early is stale.
            let mut gone = false;
            for event in link.poll() {
                match event {
                    Event::Selected(_) => {
                        gone |= link.answer(Err(t("patch_browse_01").into())).is_err()
                    }
                    Event::Hidden => {}
                    Event::Closed(_) => gone = true,
                }
            }
            if !gone && link.show().is_ok() {
                return Ok(());
            }
            self.link = None;
        }
        self.link = Some((self.start)()?);
        Ok(())
    }

    fn poll(&mut self) -> Vec<Value> {
        let Some(link) = self.link.as_mut() else {
            return vec![json!({"kind":"closed","error":t("patch_browse_01")})];
        };
        let mut events = Vec::new();
        for event in link.poll() {
            match event {
                Event::Selected(selected) => match state_file(&selected) {
                    Ok(file) => {
                        self.offered = Some(crate::patch_cell::snippet(&selected, self.track));
                        events.push(json!({"kind":"selected","format":selected.plugin.format,
                            "pluginId":selected.plugin.id,"plugin":selected.plugin.name,
                            "patch":selected.preset_name,"stateFile":file}));
                    }
                    // The player shows the error; the script never sees this selection.
                    Err(error) => {
                        if let Err(error) = link.answer(Err(error)) {
                            self.link = None;
                            events.push(json!({"kind":"closed","error":error}));
                            return events;
                        }
                    }
                },
                Event::Hidden => events.push(json!({"kind":"closed","error":null})),
                Event::Closed(error) => {
                    self.link = None;
                    events.push(json!({"kind":"closed","error":error}));
                    return events;
                }
            }
        }
        events
    }

    fn answer(&mut self, result: Result<(), String>) -> Result<(), String> {
        let offered = self.offered.take();
        let accepted = result.is_ok();
        let link = self.link.as_mut().ok_or_else(|| t("patch_browse_01"))?;
        if let Err(error) = link.answer(result) {
            self.link = None;
            return Err(error);
        }
        if accepted {
            self.accepted = offered.or(self.accepted.take());
        }
        Ok(())
    }
}

/// The selection's state, copied out of the player's shared memory, which the
/// player frees once it has the answer.
fn state_file(selected: &Selected) -> Result<PathBuf, String> {
    let bytes = resident_ipc::read_state(&selected.state).map_err(|e| e.to_string())?;
    let path = std::env::temp_dir()
        .join("repl-plugin-player")
        .join(format!("patch-audition-{}.bin", std::process::id()));
    std::fs::create_dir_all(path.parent().expect("joined")).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
#[path = "patch_browse_tests.rs"]
mod tests;
