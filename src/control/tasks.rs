use super::{Control, RunResult};
use serde_json::{json, Value};

impl Control {
    pub(super) fn poll_tasks(&mut self) -> Result<(), String> {
        let reply = self
            .host
            .call(json!({"command":"eval", "code":"__replTick()"}))?;
        let errors: Vec<String> = serde_json::from_str(reply["result"].as_str().unwrap_or("[]"))
            .map_err(|error| error.to_string())?;
        for error in errors {
            self.log
                .event("periodic_script_error", Value::String(error.clone()))?;
            self.results.push(RunResult {
                error: true,
                header: crate::i18n::t("periodic_script_error").into(),
                body: error,
            });
        }
        Ok(())
    }
}
