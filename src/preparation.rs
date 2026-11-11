use serde_json::Value;
use std::time::{Duration, Instant};

pub struct Deadline {
    started: Instant,
    paused: bool,
}
impl Deadline {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            paused: false,
        }
    }
    pub fn pause(&mut self) {
        self.paused = true;
    }
    pub fn reset(&mut self, now: Instant) {
        *self = Self::new(now);
    }
    pub fn expired(&self, now: Instant) -> bool {
        !self.paused && now.duration_since(self.started) > Duration::from_secs(120)
    }
}

/// Absence is meaningful only after the caller has checked successful full scan.
pub fn select(catalog: &Value, target: &str) -> Result<Option<Value>, String> {
    Ok(catalog
        .as_array()
        .ok_or(crate::i18n::t("m055"))?
        .iter()
        .find(|p| p["format"] == "CLAP" && p["pluginId"] == target)
        .cloned())
}

pub enum ScanDecision {
    Create(Value),
    FullScan,
    Missing,
}

pub fn decide(
    catalog: &Value,
    target: &str,
    full: bool,
    error: &str,
) -> Result<ScanDecision, String> {
    if !error.is_empty() {
        return Err(crate::i18n::render("f016", &[("scan_error", error.into())]));
    }
    Ok(match select(catalog, target)? {
        Some(plugin) => ScanDecision::Create(plugin),
        None if full => ScanDecision::Missing,
        None => ScanDecision::FullScan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn consent_and_install_time_do_not_consume_preparation_deadline() {
        let start = Instant::now();
        let later = start + Duration::from_secs(600);
        let mut deadline = Deadline::new(start);
        assert!(deadline.expired(later));
        deadline.pause();
        assert!(!deadline.expired(later));
        deadline.reset(later);
        assert!(!deadline.expired(later + Duration::from_secs(119)));
        assert!(deadline.expired(later + Duration::from_secs(121)));
    }
    #[test]
    fn vst3_and_other_instruments_are_not_surge_clap() {
        let catalog = json!([
            {"format":"CLAP","pluginId":"other","kind":"Instrument"},
            {"format":"VST3","pluginId":crate::control::SURGE}
        ]);
        assert!(select(&catalog, crate::control::SURGE).unwrap().is_none());
        assert!(matches!(
            decide(&catalog, crate::control::SURGE, true, ""),
            Ok(ScanDecision::Missing)
        ));
        let catalog = json!([{"format":"CLAP","pluginId":crate::control::SURGE}]);
        assert!(select(&catalog, crate::control::SURGE).unwrap().is_some());
        assert!(matches!(
            decide(&catalog, crate::control::SURGE, false, ""),
            Ok(ScanDecision::Create(_))
        ));
        assert!(matches!(
            decide(&json!([]), crate::control::SURGE, false, ""),
            Ok(ScanDecision::FullScan)
        ));
        assert!(matches!(
            decide(&json!([]), crate::control::SURGE, true, ""),
            Ok(ScanDecision::Missing)
        ));
        assert!(decide(&json!([]), crate::control::SURGE, true, "scan failed").is_err());
        assert!(select(&Value::Null, crate::control::SURGE).is_err());
    }
}
