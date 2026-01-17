use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessState {
    Ready,
    NotReady,
    Starting,
    Stopping,
}

impl ReadinessState {
    pub fn is_ready(&self) -> bool {
        matches!(self, ReadinessState::Ready)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReadinessState::Ready => "ready",
            ReadinessState::NotReady => "not_ready",
            ReadinessState::Starting => "starting",
            ReadinessState::Stopping => "stopping",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReadinessProbe {
    pub name: String,
    pub ready: bool,
    pub message: Option<String>,
    pub last_check: Instant,
    pub consecutive_failures: u32,
}

impl ReadinessProbe {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ready: false,
            message: None,
            last_check: Instant::now(),
            consecutive_failures: 0,
        }
    }

    pub fn set_ready(&mut self) {
        self.ready = true;
        self.message = None;
        self.last_check = Instant::now();
        self.consecutive_failures = 0;
    }

    pub fn set_not_ready(&mut self, message: &str) {
        self.ready = false;
        self.message = Some(message.to_string());
        self.last_check = Instant::now();
        self.consecutive_failures += 1;
    }
}

pub struct ReadinessChecker {
    probes: Arc<Mutex<HashMap<String, ReadinessProbe>>>,
    startup_time: Instant,
    startup_grace_period: Duration,
    failure_threshold: u32,
}

impl ReadinessChecker {
    pub fn new() -> Self {
        Self {
            probes: Arc::new(Mutex::new(HashMap::new())),
            startup_time: Instant::now(),
            startup_grace_period: Duration::from_secs(30),
            failure_threshold: 3,
        }
    }

    pub fn with_grace_period(mut self, period: Duration) -> Self {
        self.startup_grace_period = period;
        self
    }

    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    pub fn register_probe(&self, name: &str) {
        let mut probes = self.probes.lock().unwrap();
        probes.insert(name.to_string(), ReadinessProbe::new(name));
    }

    pub fn set_ready(&self, name: &str) {
        let mut probes = self.probes.lock().unwrap();
        if let Some(probe) = probes.get_mut(name) {
            probe.set_ready();
        }
    }

    pub fn set_not_ready(&self, name: &str, message: &str) {
        let mut probes = self.probes.lock().unwrap();
        if let Some(probe) = probes.get_mut(name) {
            probe.set_not_ready(message);
        }
    }

    pub fn is_ready(&self) -> bool {
        if self.startup_time.elapsed() < self.startup_grace_period {
            return self.is_starting();
        }

        let probes = self.probes.lock().unwrap();

        if probes.is_empty() {
            return true;
        }

        probes.values().all(|p| p.ready)
    }

    pub fn is_starting(&self) -> bool {
        self.startup_time.elapsed() < self.startup_grace_period
    }

    pub fn state(&self) -> ReadinessState {
        if self.is_starting() {
            return ReadinessState::Starting;
        }

        if self.is_ready() {
            ReadinessState::Ready
        } else {
            ReadinessState::NotReady
        }
    }

    pub fn get_probe(&self, name: &str) -> Option<ReadinessProbe> {
        let probes = self.probes.lock().unwrap();
        probes.get(name).cloned()
    }

    pub fn get_all_probes(&self) -> Vec<ReadinessProbe> {
        let probes = self.probes.lock().unwrap();
        probes.values().cloned().collect()
    }

    pub fn get_failed_probes(&self) -> Vec<ReadinessProbe> {
        let probes = self.probes.lock().unwrap();
        probes
            .values()
            .filter(|p| !p.ready)
            .cloned()
            .collect()
    }

    pub fn get_probes_exceeding_threshold(&self) -> Vec<ReadinessProbe> {
        let probes = self.probes.lock().unwrap();
        probes
            .values()
            .filter(|p| p.consecutive_failures >= self.failure_threshold)
            .cloned()
            .collect()
    }

    pub fn time_since_startup(&self) -> Duration {
        self.startup_time.elapsed()
    }

    pub fn reset_startup(&mut self) {
        self.startup_time = Instant::now();
    }

    pub fn clear_probes(&self) {
        let mut probes = self.probes.lock().unwrap();
        probes.clear();
    }
}

impl Default for ReadinessChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ReadinessChecker {
    fn clone(&self) -> Self {
        Self {
            probes: self.probes.clone(),
            startup_time: self.startup_time,
            startup_grace_period: self.startup_grace_period,
            failure_threshold: self.failure_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readiness_state() {
        assert!(ReadinessState::Ready.is_ready());
        assert!(!ReadinessState::NotReady.is_ready());
        assert!(!ReadinessState::Starting.is_ready());
    }

    #[test]
    fn test_readiness_probe() {
        let mut probe = ReadinessProbe::new("database");
        assert!(!probe.ready);

        probe.set_ready();
        assert!(probe.ready);
        assert_eq!(probe.consecutive_failures, 0);

        probe.set_not_ready("Connection failed");
        assert!(!probe.ready);
        assert_eq!(probe.consecutive_failures, 1);

        probe.set_not_ready("Still failing");
        assert_eq!(probe.consecutive_failures, 2);
    }

    #[test]
    fn test_readiness_checker_empty() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::ZERO);

        assert!(checker.is_ready());
    }

    #[test]
    fn test_readiness_checker_with_probes() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::ZERO);

        checker.register_probe("database");
        checker.register_probe("cache");

        assert!(!checker.is_ready());

        checker.set_ready("database");
        assert!(!checker.is_ready());

        checker.set_ready("cache");
        assert!(checker.is_ready());
    }

    #[test]
    fn test_readiness_checker_grace_period() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::from_secs(60));

        checker.register_probe("database");

        assert!(checker.is_starting());
        assert_eq!(checker.state(), ReadinessState::Starting);
    }

    #[test]
    fn test_readiness_checker_failed_probes() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::ZERO);

        checker.register_probe("database");
        checker.register_probe("cache");

        checker.set_ready("database");
        checker.set_not_ready("cache", "Connection refused");

        let failed = checker.get_failed_probes();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].name, "cache");
    }

    #[test]
    fn test_readiness_checker_failure_threshold() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::ZERO)
            .with_failure_threshold(3);

        checker.register_probe("database");

        checker.set_not_ready("database", "Error 1");
        checker.set_not_ready("database", "Error 2");

        let exceeding = checker.get_probes_exceeding_threshold();
        assert_eq!(exceeding.len(), 0);

        checker.set_not_ready("database", "Error 3");

        let exceeding = checker.get_probes_exceeding_threshold();
        assert_eq!(exceeding.len(), 1);
    }

    #[test]
    fn test_readiness_checker_get_all_probes() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::ZERO);

        checker.register_probe("database");
        checker.register_probe("cache");
        checker.register_probe("queue");

        let probes = checker.get_all_probes();
        assert_eq!(probes.len(), 3);
    }

    #[test]
    fn test_readiness_checker_clear() {
        let checker = ReadinessChecker::new()
            .with_grace_period(Duration::ZERO);

        checker.register_probe("database");
        checker.register_probe("cache");

        checker.clear_probes();

        let probes = checker.get_all_probes();
        assert_eq!(probes.len(), 0);
    }
}
