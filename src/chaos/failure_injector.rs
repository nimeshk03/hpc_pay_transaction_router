use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureType {
    CompleteOutage,
    PartialOutage,
    HighLatency,
    IntermittentFailure,
    RateLimitExceeded,
    InvalidResponse,
    Timeout,
    ConnectionRefused,
}

impl FailureType {
    pub fn description(&self) -> &'static str {
        match self {
            FailureType::CompleteOutage => "Complete service outage",
            FailureType::PartialOutage => "Partial service degradation",
            FailureType::HighLatency => "High latency responses",
            FailureType::IntermittentFailure => "Intermittent failures",
            FailureType::RateLimitExceeded => "Rate limit exceeded",
            FailureType::InvalidResponse => "Invalid/malformed responses",
            FailureType::Timeout => "Request timeouts",
            FailureType::ConnectionRefused => "Connection refused",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FailureConfig {
    pub failure_type: FailureType,
    pub target: String,
    pub duration: Duration,
    pub failure_rate: f64,
    pub latency_ms: Option<u64>,
}

impl FailureConfig {
    pub fn new(failure_type: FailureType, target: &str) -> Self {
        Self {
            failure_type,
            target: target.to_string(),
            duration: Duration::from_secs(60),
            failure_rate: 1.0,
            latency_ms: None,
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }
}

#[derive(Debug, Clone)]
struct ActiveFailure {
    config: FailureConfig,
    start_time: std::time::Instant,
    requests_affected: u64,
}

pub struct FailureInjector {
    active_failures: Arc<Mutex<HashMap<String, ActiveFailure>>>,
    failure_history: Arc<Mutex<Vec<FailureConfig>>>,
}

impl FailureInjector {
    pub fn new() -> Self {
        Self {
            active_failures: Arc::new(Mutex::new(HashMap::new())),
            failure_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn inject(&self, config: FailureConfig) {
        let mut failures = self.active_failures.lock().unwrap();
        let mut history = self.failure_history.lock().unwrap();

        let active = ActiveFailure {
            config: config.clone(),
            start_time: std::time::Instant::now(),
            requests_affected: 0,
        };

        failures.insert(config.target.clone(), active);
        history.push(config);
    }

    pub fn remove(&self, target: &str) -> Option<FailureConfig> {
        let mut failures = self.active_failures.lock().unwrap();
        failures.remove(target).map(|f| f.config)
    }

    pub fn clear_all(&self) {
        let mut failures = self.active_failures.lock().unwrap();
        failures.clear();
    }

    pub fn is_failure_active(&self, target: &str) -> bool {
        let failures = self.active_failures.lock().unwrap();
        if let Some(failure) = failures.get(target) {
            failure.start_time.elapsed() < failure.config.duration
        } else {
            false
        }
    }

    pub fn should_fail(&self, target: &str) -> Option<FailureType> {
        let mut failures = self.active_failures.lock().unwrap();

        if let Some(failure) = failures.get_mut(target) {
            if failure.start_time.elapsed() >= failure.config.duration {
                return None;
            }

            failure.requests_affected += 1;

            if failure.config.failure_rate >= 1.0 {
                return Some(failure.config.failure_type);
            }

            let random: f64 = rand::random();
            if random < failure.config.failure_rate {
                return Some(failure.config.failure_type);
            }
        }

        None
    }

    pub fn get_injected_latency(&self, target: &str) -> Option<Duration> {
        let failures = self.active_failures.lock().unwrap();

        if let Some(failure) = failures.get(target) {
            if failure.start_time.elapsed() < failure.config.duration {
                if let Some(latency_ms) = failure.config.latency_ms {
                    return Some(Duration::from_millis(latency_ms));
                }
            }
        }

        None
    }

    pub fn get_active_failures(&self) -> Vec<(String, FailureType)> {
        let failures = self.active_failures.lock().unwrap();
        failures
            .iter()
            .filter(|(_, f)| f.start_time.elapsed() < f.config.duration)
            .map(|(target, f)| (target.clone(), f.config.failure_type))
            .collect()
    }

    pub fn get_failure_stats(&self, target: &str) -> Option<(u64, Duration)> {
        let failures = self.active_failures.lock().unwrap();
        failures.get(target).map(|f| {
            let elapsed = f.start_time.elapsed();
            (f.requests_affected, elapsed)
        })
    }

    pub fn get_history(&self) -> Vec<FailureConfig> {
        let history = self.failure_history.lock().unwrap();
        history.clone()
    }
}

impl Default for FailureInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FailureInjector {
    fn clone(&self) -> Self {
        Self {
            active_failures: self.active_failures.clone(),
            failure_history: self.failure_history.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_config_creation() {
        let config = FailureConfig::new(FailureType::CompleteOutage, "stripe")
            .with_duration(Duration::from_secs(30))
            .with_failure_rate(0.5);

        assert_eq!(config.failure_type, FailureType::CompleteOutage);
        assert_eq!(config.target, "stripe");
        assert_eq!(config.duration, Duration::from_secs(30));
        assert_eq!(config.failure_rate, 0.5);
    }

    #[test]
    fn test_failure_injector_inject_and_check() {
        let injector = FailureInjector::new();

        let config = FailureConfig::new(FailureType::CompleteOutage, "stripe")
            .with_duration(Duration::from_secs(60));

        injector.inject(config);

        assert!(injector.is_failure_active("stripe"));
        assert!(!injector.is_failure_active("adyen"));
    }

    #[test]
    fn test_failure_injector_should_fail() {
        let injector = FailureInjector::new();

        let config = FailureConfig::new(FailureType::Timeout, "stripe")
            .with_failure_rate(1.0);

        injector.inject(config);

        let result = injector.should_fail("stripe");
        assert_eq!(result, Some(FailureType::Timeout));

        let result = injector.should_fail("adyen");
        assert_eq!(result, None);
    }

    #[test]
    fn test_failure_injector_remove() {
        let injector = FailureInjector::new();

        let config = FailureConfig::new(FailureType::CompleteOutage, "stripe");
        injector.inject(config);

        assert!(injector.is_failure_active("stripe"));

        injector.remove("stripe");

        assert!(!injector.is_failure_active("stripe"));
    }

    #[test]
    fn test_failure_injector_latency() {
        let injector = FailureInjector::new();

        let config = FailureConfig::new(FailureType::HighLatency, "stripe")
            .with_latency(500);

        injector.inject(config);

        let latency = injector.get_injected_latency("stripe");
        assert_eq!(latency, Some(Duration::from_millis(500)));
    }

    #[test]
    fn test_failure_injector_stats() {
        let injector = FailureInjector::new();

        let config = FailureConfig::new(FailureType::CompleteOutage, "stripe");
        injector.inject(config);

        injector.should_fail("stripe");
        injector.should_fail("stripe");
        injector.should_fail("stripe");

        let stats = injector.get_failure_stats("stripe");
        assert!(stats.is_some());
        let (affected, _) = stats.unwrap();
        assert_eq!(affected, 3);
    }

    #[test]
    fn test_failure_type_descriptions() {
        assert!(!FailureType::CompleteOutage.description().is_empty());
        assert!(!FailureType::Timeout.description().is_empty());
    }
}
