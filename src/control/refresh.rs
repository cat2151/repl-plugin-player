use super::Control;
use crate::catalog_refresh::Refresh;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

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
    pub(super) fn observe_refresh(&mut self, state: &Value) -> Result<(), String> {
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
    pub(super) fn finish_refresh(&mut self) -> Result<(), String> {
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
