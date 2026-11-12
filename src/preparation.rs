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

#[cfg(test)]
mod tests {
    use super::*;
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
}
