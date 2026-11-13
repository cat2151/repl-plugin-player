//! A catalog read from the plugin list cache at startup, refreshed by one scan
//! once the GUI has been idle for a while.
use std::time::{Duration, Instant};

/// How long the GUI has to be without input before the refresh scan starts.
const IDLE: Duration = Duration::from_secs(2);

#[derive(Debug, Default, PartialEq)]
pub enum Refresh {
    /// Nothing to refresh: the startup scan already ran, or the refresh finished.
    #[default]
    Done,
    /// The catalog came from the cache; `idle_since` is when input last stopped.
    Pending {
        idle_since: Option<Instant>,
    },
    Running,
}

impl Refresh {
    pub fn pending() -> Self {
        Self::Pending { idle_since: None }
    }
    /// True once, when the refresh should start now.
    pub fn should_start(&mut self, active: bool, now: Instant) -> bool {
        let Self::Pending { idle_since } = self else {
            return false;
        };
        if active {
            *idle_since = None;
            return false;
        }
        if now.duration_since(*idle_since.get_or_insert(now)) < IDLE {
            return false;
        }
        *self = Self::Running;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_once_after_two_idle_seconds_and_input_restarts_the_wait() {
        let start = Instant::now();
        let mut refresh = Refresh::pending();
        assert!(!refresh.should_start(false, start));
        assert!(!refresh.should_start(false, start + Duration::from_millis(1900)));
        assert!(!refresh.should_start(true, start + Duration::from_millis(1950)));
        assert!(!refresh.should_start(false, start + Duration::from_millis(2000)));
        assert!(!refresh.should_start(false, start + Duration::from_millis(3900)));
        assert!(refresh.should_start(false, start + Duration::from_millis(4000)));
        assert_eq!(refresh, Refresh::Running);
        assert!(!refresh.should_start(false, start + Duration::from_secs(10)));
    }

    #[test]
    fn never_starts_without_a_cached_catalog() {
        let mut refresh = Refresh::Done;
        assert!(!refresh.should_start(false, Instant::now() + Duration::from_secs(60)));
    }
}
