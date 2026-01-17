use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryResult {
    Success,
    PartialRecovery,
    Failed,
    InProgress,
    NotStarted,
}

impl RecoveryResult {
    pub fn is_successful(&self) -> bool {
        matches!(self, RecoveryResult::Success | RecoveryResult::PartialRecovery)
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryMetrics {
    pub target: String,
    pub failure_start: Instant,
    pub recovery_start: Option<Instant>,
    pub recovery_end: Option<Instant>,
    pub requests_during_failure: u64,
    pub successful_requests_during_failure: u64,
    pub requests_during_recovery: u64,
    pub successful_requests_during_recovery: u64,
    pub result: RecoveryResult,
}

impl RecoveryMetrics {
    pub fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
            failure_start: Instant::now(),
            recovery_start: None,
            recovery_end: None,
            requests_during_failure: 0,
            successful_requests_during_failure: 0,
            requests_during_recovery: 0,
            successful_requests_during_recovery: 0,
            result: RecoveryResult::NotStarted,
        }
    }

    pub fn failure_duration(&self) -> Duration {
        match self.recovery_start {
            Some(start) => start.duration_since(self.failure_start),
            None => self.failure_start.elapsed(),
        }
    }

    pub fn recovery_duration(&self) -> Option<Duration> {
        match (self.recovery_start, self.recovery_end) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(start.elapsed()),
            _ => None,
        }
    }

    pub fn total_duration(&self) -> Duration {
        match self.recovery_end {
            Some(end) => end.duration_since(self.failure_start),
            None => self.failure_start.elapsed(),
        }
    }

    pub fn success_rate_during_failure(&self) -> f64 {
        if self.requests_during_failure == 0 {
            return 0.0;
        }
        self.successful_requests_during_failure as f64 / self.requests_during_failure as f64
    }

    pub fn success_rate_during_recovery(&self) -> f64 {
        if self.requests_during_recovery == 0 {
            return 0.0;
        }
        self.successful_requests_during_recovery as f64 / self.requests_during_recovery as f64
    }
}

pub struct RecoveryVerifier {
    metrics: Arc<Mutex<HashMap<String, RecoveryMetrics>>>,
    success_threshold: f64,
    recovery_timeout: Duration,
}

impl RecoveryVerifier {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            success_threshold: 0.999,
            recovery_timeout: Duration::from_secs(60),
        }
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.success_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.recovery_timeout = timeout;
        self
    }

    pub fn start_failure_tracking(&self, target: &str) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.insert(target.to_string(), RecoveryMetrics::new(target));
    }

    pub fn record_request(&self, target: &str, success: bool) {
        let mut metrics = self.metrics.lock().unwrap();

        if let Some(m) = metrics.get_mut(target) {
            match m.result {
                RecoveryResult::NotStarted | RecoveryResult::InProgress => {
                    if m.recovery_start.is_none() {
                        m.requests_during_failure += 1;
                        if success {
                            m.successful_requests_during_failure += 1;
                        }
                    } else {
                        m.requests_during_recovery += 1;
                        if success {
                            m.successful_requests_during_recovery += 1;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn start_recovery(&self, target: &str) {
        let mut metrics = self.metrics.lock().unwrap();

        if let Some(m) = metrics.get_mut(target) {
            m.recovery_start = Some(Instant::now());
            m.result = RecoveryResult::InProgress;
        }
    }

    pub fn verify_recovery(&self, target: &str) -> RecoveryResult {
        let mut metrics = self.metrics.lock().unwrap();

        if let Some(m) = metrics.get_mut(target) {
            if m.recovery_start.is_none() {
                return RecoveryResult::NotStarted;
            }

            let recovery_duration = m.recovery_start.unwrap().elapsed();

            if recovery_duration > self.recovery_timeout {
                m.result = RecoveryResult::Failed;
                m.recovery_end = Some(Instant::now());
                return RecoveryResult::Failed;
            }

            let success_rate = m.success_rate_during_recovery();

            if m.requests_during_recovery >= 10 {
                if success_rate >= self.success_threshold {
                    m.result = RecoveryResult::Success;
                    m.recovery_end = Some(Instant::now());
                    return RecoveryResult::Success;
                } else if success_rate >= 0.9 {
                    m.result = RecoveryResult::PartialRecovery;
                    return RecoveryResult::PartialRecovery;
                }
            }

            RecoveryResult::InProgress
        } else {
            RecoveryResult::NotStarted
        }
    }

    pub fn complete_recovery(&self, target: &str, result: RecoveryResult) {
        let mut metrics = self.metrics.lock().unwrap();

        if let Some(m) = metrics.get_mut(target) {
            m.result = result;
            m.recovery_end = Some(Instant::now());
        }
    }

    pub fn get_metrics(&self, target: &str) -> Option<RecoveryMetrics> {
        let metrics = self.metrics.lock().unwrap();
        metrics.get(target).cloned()
    }

    pub fn get_all_metrics(&self) -> HashMap<String, RecoveryMetrics> {
        let metrics = self.metrics.lock().unwrap();
        metrics.clone()
    }

    pub fn clear(&self, target: &str) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.remove(target);
    }

    pub fn clear_all(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.clear();
    }

    pub fn generate_report(&self, target: &str) -> Option<String> {
        let metrics = self.metrics.lock().unwrap();

        metrics.get(target).map(|m| {
            format!(
                "Recovery Report for {}\n\
                 Result: {:?}\n\
                 Failure Duration: {:?}\n\
                 Recovery Duration: {:?}\n\
                 Total Duration: {:?}\n\
                 Requests During Failure: {} (Success Rate: {:.2}%)\n\
                 Requests During Recovery: {} (Success Rate: {:.2}%)",
                m.target,
                m.result,
                m.failure_duration(),
                m.recovery_duration().unwrap_or(Duration::ZERO),
                m.total_duration(),
                m.requests_during_failure,
                m.success_rate_during_failure() * 100.0,
                m.requests_during_recovery,
                m.success_rate_during_recovery() * 100.0,
            )
        })
    }
}

impl Default for RecoveryVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RecoveryVerifier {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            success_threshold: self.success_threshold,
            recovery_timeout: self.recovery_timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_metrics_new() {
        let metrics = RecoveryMetrics::new("stripe");
        assert_eq!(metrics.target, "stripe");
        assert_eq!(metrics.result, RecoveryResult::NotStarted);
    }

    #[test]
    fn test_recovery_verifier_tracking() {
        let verifier = RecoveryVerifier::new();

        verifier.start_failure_tracking("stripe");
        verifier.record_request("stripe", false);
        verifier.record_request("stripe", false);
        verifier.record_request("stripe", true);

        let metrics = verifier.get_metrics("stripe").unwrap();
        assert_eq!(metrics.requests_during_failure, 3);
        assert_eq!(metrics.successful_requests_during_failure, 1);
    }

    #[test]
    fn test_recovery_verifier_recovery_phase() {
        let verifier = RecoveryVerifier::new();

        verifier.start_failure_tracking("stripe");
        verifier.record_request("stripe", false);

        verifier.start_recovery("stripe");

        for _ in 0..15 {
            verifier.record_request("stripe", true);
        }

        let result = verifier.verify_recovery("stripe");
        assert_eq!(result, RecoveryResult::Success);
    }

    #[test]
    fn test_recovery_verifier_partial_recovery() {
        let verifier = RecoveryVerifier::new().with_threshold(0.99);

        verifier.start_failure_tracking("stripe");
        verifier.start_recovery("stripe");

        for _ in 0..9 {
            verifier.record_request("stripe", true);
        }
        verifier.record_request("stripe", false);

        let result = verifier.verify_recovery("stripe");
        assert_eq!(result, RecoveryResult::PartialRecovery);
    }

    #[test]
    fn test_recovery_result_is_successful() {
        assert!(RecoveryResult::Success.is_successful());
        assert!(RecoveryResult::PartialRecovery.is_successful());
        assert!(!RecoveryResult::Failed.is_successful());
        assert!(!RecoveryResult::InProgress.is_successful());
    }

    #[test]
    fn test_recovery_verifier_report() {
        let verifier = RecoveryVerifier::new();

        verifier.start_failure_tracking("stripe");
        verifier.record_request("stripe", false);
        verifier.start_recovery("stripe");
        verifier.record_request("stripe", true);
        verifier.complete_recovery("stripe", RecoveryResult::Success);

        let report = verifier.generate_report("stripe");
        assert!(report.is_some());
        assert!(report.unwrap().contains("stripe"));
    }

    #[test]
    fn test_recovery_metrics_success_rates() {
        let mut metrics = RecoveryMetrics::new("stripe");
        metrics.requests_during_failure = 10;
        metrics.successful_requests_during_failure = 3;
        metrics.requests_during_recovery = 20;
        metrics.successful_requests_during_recovery = 19;

        assert_eq!(metrics.success_rate_during_failure(), 0.3);
        assert_eq!(metrics.success_rate_during_recovery(), 0.95);
    }
}
