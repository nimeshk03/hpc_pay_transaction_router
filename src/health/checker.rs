use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub timestamp: Instant,
    pub error_message: Option<String>,
}

impl HealthCheckResult {
    pub fn healthy(response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Healthy,
            response_time_ms,
            timestamp: Instant::now(),
            error_message: None,
        }
    }

    pub fn degraded(response_time_ms: u64, message: String) -> Self {
        Self {
            status: HealthStatus::Degraded,
            response_time_ms,
            timestamp: Instant::now(),
            error_message: Some(message),
        }
    }

    pub fn unhealthy(error_message: String) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            response_time_ms: 0,
            timestamp: Instant::now(),
            error_message: Some(error_message),
        }
    }

    pub fn unknown() -> Self {
        Self {
            status: HealthStatus::Unknown,
            response_time_ms: 0,
            timestamp: Instant::now(),
            error_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub probe_interval_secs: u64,
    pub timeout_ms: u64,
    pub healthy_threshold: usize,
    pub unhealthy_threshold: usize,
    pub window_size: usize,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            probe_interval_secs: 30,
            timeout_ms: 5000,
            healthy_threshold: 3,
            unhealthy_threshold: 3,
            window_size: 100,
        }
    }
}

impl HealthCheckConfig {
    pub fn new(
        probe_interval_secs: u64,
        timeout_ms: u64,
        healthy_threshold: usize,
        unhealthy_threshold: usize,
    ) -> Self {
        Self {
            probe_interval_secs,
            timeout_ms,
            healthy_threshold,
            unhealthy_threshold,
            window_size: 100,
        }
    }

    pub fn with_window_size(mut self, window_size: usize) -> Self {
        self.window_size = window_size;
        self
    }
}

#[derive(Debug)]
struct RouteHealthData {
    results: VecDeque<HealthCheckResult>,
    total_probes: u64,
    successful_probes: u64,
    failed_probes: u64,
    consecutive_successes: usize,
    consecutive_failures: usize,
    current_status: HealthStatus,
    last_probe_time: Option<Instant>,
}

impl RouteHealthData {
    fn new(window_size: usize) -> Self {
        Self {
            results: VecDeque::with_capacity(window_size),
            total_probes: 0,
            successful_probes: 0,
            failed_probes: 0,
            consecutive_successes: 0,
            consecutive_failures: 0,
            current_status: HealthStatus::Unknown,
            last_probe_time: None,
        }
    }

    fn record_result(&mut self, result: HealthCheckResult, window_size: usize) {
        if self.results.len() >= window_size {
            if let Some(old_result) = self.results.pop_front() {
                if old_result.status == HealthStatus::Healthy {
                    self.successful_probes = self.successful_probes.saturating_sub(1);
                } else {
                    self.failed_probes = self.failed_probes.saturating_sub(1);
                }
                self.total_probes = self.total_probes.saturating_sub(1);
            }
        }

        self.total_probes += 1;
        if result.status == HealthStatus::Healthy {
            self.successful_probes += 1;
            self.consecutive_successes += 1;
            self.consecutive_failures = 0;
        } else {
            self.failed_probes += 1;
            self.consecutive_failures += 1;
            self.consecutive_successes = 0;
        }

        self.last_probe_time = Some(result.timestamp);
        self.results.push_back(result);
    }

    fn update_status(&mut self, healthy_threshold: usize, unhealthy_threshold: usize) {
        if self.consecutive_successes >= healthy_threshold {
            self.current_status = HealthStatus::Healthy;
        } else if self.consecutive_failures >= unhealthy_threshold {
            self.current_status = HealthStatus::Unhealthy;
        } else if self.total_probes > 0 {
            let success_rate = self.successful_probes as f64 / self.total_probes as f64;
            if success_rate >= 0.9 {
                self.current_status = HealthStatus::Healthy;
            } else if success_rate >= 0.5 {
                self.current_status = HealthStatus::Degraded;
            } else {
                self.current_status = HealthStatus::Unhealthy;
            }
        }
    }

    fn availability(&self) -> f64 {
        if self.total_probes == 0 {
            return 1.0;
        }
        self.successful_probes as f64 / self.total_probes as f64
    }

    fn avg_response_time_ms(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }

        let sum: u64 = self.results.iter().map(|r| r.response_time_ms).sum();
        sum as f64 / self.results.len() as f64
    }
}

#[derive(Debug, Clone)]
pub struct HealthChecker {
    routes: Arc<Mutex<HashMap<String, RouteHealthData>>>,
    config: HealthCheckConfig,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            routes: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(HealthCheckConfig::default())
    }

    fn get_or_create_route_data(&self, route_id: &str) -> bool {
        let mut routes = self.routes.lock().unwrap();
        if !routes.contains_key(route_id) {
            routes.insert(
                route_id.to_string(),
                RouteHealthData::new(self.config.window_size),
            );
            true
        } else {
            false
        }
    }

    pub fn add_route(&self, route_id: &str) {
        self.get_or_create_route_data(route_id);
    }

    pub fn record_probe_success(&self, route_id: &str, response_time_ms: u64) {
        self.get_or_create_route_data(route_id);
        let mut routes = self.routes.lock().unwrap();

        if let Some(data) = routes.get_mut(route_id) {
            let result = if response_time_ms > self.config.timeout_ms {
                HealthCheckResult::degraded(
                    response_time_ms,
                    format!("Response time {}ms exceeds timeout", response_time_ms),
                )
            } else {
                HealthCheckResult::healthy(response_time_ms)
            };

            data.record_result(result, self.config.window_size);
            data.update_status(
                self.config.healthy_threshold,
                self.config.unhealthy_threshold,
            );
        }
    }

    pub fn record_probe_failure(&self, route_id: &str, error_message: String) {
        self.get_or_create_route_data(route_id);
        let mut routes = self.routes.lock().unwrap();

        if let Some(data) = routes.get_mut(route_id) {
            let result = HealthCheckResult::unhealthy(error_message);
            data.record_result(result, self.config.window_size);
            data.update_status(
                self.config.healthy_threshold,
                self.config.unhealthy_threshold,
            );
        }
    }

    pub fn get_status(&self, route_id: &str) -> HealthStatus {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.current_status)
            .unwrap_or(HealthStatus::Unknown)
    }

    pub fn get_availability(&self, route_id: &str) -> f64 {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.availability())
            .unwrap_or(1.0)
    }

    pub fn get_avg_response_time(&self, route_id: &str) -> f64 {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.avg_response_time_ms())
            .unwrap_or(0.0)
    }

    pub fn get_probe_count(&self, route_id: &str) -> u64 {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.total_probes)
            .unwrap_or(0)
    }

    pub fn get_last_probe_time(&self, route_id: &str) -> Option<Instant> {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .and_then(|data| data.last_probe_time)
    }

    pub fn get_all_statuses(&self) -> HashMap<String, HealthStatus> {
        let routes = self.routes.lock().unwrap();
        routes
            .iter()
            .map(|(id, data)| (id.clone(), data.current_status))
            .collect()
    }

    pub fn get_all_availabilities(&self) -> HashMap<String, f64> {
        let routes = self.routes.lock().unwrap();
        routes
            .iter()
            .map(|(id, data)| (id.clone(), data.availability()))
            .collect()
    }

    pub fn is_healthy(&self, route_id: &str) -> bool {
        self.get_status(route_id) == HealthStatus::Healthy
    }

    pub fn reset(&self, route_id: &str) {
        let mut routes = self.routes.lock().unwrap();
        routes.remove(route_id);
    }

    pub fn reset_all(&self) {
        let mut routes = self.routes.lock().unwrap();
        routes.clear();
    }

    pub fn get_config(&self) -> HealthCheckConfig {
        self.config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker_creation() {
        let checker = HealthChecker::with_default_config();
        assert_eq!(checker.get_status("stripe"), HealthStatus::Unknown);
    }

    #[test]
    fn test_add_route() {
        let checker = HealthChecker::with_default_config();
        checker.add_route("stripe");
        assert_eq!(checker.get_status("stripe"), HealthStatus::Unknown);
    }

    #[test]
    fn test_record_probe_success() {
        let config = HealthCheckConfig::new(30, 5000, 3, 3);
        let checker = HealthChecker::new(config);

        checker.record_probe_success("stripe", 100);
        assert_eq!(checker.get_probe_count("stripe"), 1);
        assert_eq!(checker.get_availability("stripe"), 1.0);
    }

    #[test]
    fn test_record_probe_failure() {
        let config = HealthCheckConfig::new(30, 5000, 3, 3);
        let checker = HealthChecker::new(config);

        checker.record_probe_failure("stripe", "Connection timeout".to_string());
        assert_eq!(checker.get_probe_count("stripe"), 1);
        assert_eq!(checker.get_availability("stripe"), 0.0);
    }

    #[test]
    fn test_health_status_transitions() {
        let config = HealthCheckConfig::new(30, 5000, 3, 3);
        let checker = HealthChecker::new(config);

        for _ in 0..3 {
            checker.record_probe_success("stripe", 100);
        }
        assert_eq!(checker.get_status("stripe"), HealthStatus::Healthy);

        for _ in 0..3 {
            checker.record_probe_failure("stripe", "Error".to_string());
        }
        assert_eq!(checker.get_status("stripe"), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_availability_calculation() {
        let checker = HealthChecker::with_default_config();

        for _ in 0..7 {
            checker.record_probe_success("stripe", 100);
        }
        for _ in 0..3 {
            checker.record_probe_failure("stripe", "Error".to_string());
        }

        let availability = checker.get_availability("stripe");
        assert_eq!(availability, 0.7);
    }

    #[test]
    fn test_avg_response_time() {
        let checker = HealthChecker::with_default_config();

        checker.record_probe_success("stripe", 100);
        checker.record_probe_success("stripe", 200);
        checker.record_probe_success("stripe", 300);

        let avg = checker.get_avg_response_time("stripe");
        assert_eq!(avg, 200.0);
    }

    #[test]
    fn test_degraded_status_on_slow_response() {
        let config = HealthCheckConfig::new(30, 1000, 3, 3);
        let checker = HealthChecker::new(config);

        checker.record_probe_success("stripe", 1500);
        assert_eq!(checker.get_probe_count("stripe"), 1);
    }

    #[test]
    fn test_rolling_window() {
        let config = HealthCheckConfig::new(30, 5000, 3, 3).with_window_size(5);
        let checker = HealthChecker::new(config);

        for _ in 0..10 {
            checker.record_probe_success("stripe", 100);
        }

        let routes = checker.routes.lock().unwrap();
        let data = routes.get("stripe").unwrap();
        assert_eq!(data.results.len(), 5);
        assert_eq!(data.total_probes, 5);
    }

    #[test]
    fn test_consecutive_failures() {
        let config = HealthCheckConfig::new(30, 5000, 3, 3);
        let checker = HealthChecker::new(config);

        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("stripe", "Error".to_string());
        checker.record_probe_failure("stripe", "Error".to_string());
        checker.record_probe_failure("stripe", "Error".to_string());

        assert_eq!(checker.get_status("stripe"), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_get_all_statuses() {
        let checker = HealthChecker::with_default_config();

        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("adyen", "Error".to_string());

        let statuses = checker.get_all_statuses();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.contains_key("stripe"));
        assert!(statuses.contains_key("adyen"));
    }

    #[test]
    fn test_get_all_availabilities() {
        let checker = HealthChecker::with_default_config();

        for _ in 0..9 {
            checker.record_probe_success("stripe", 100);
        }
        checker.record_probe_failure("stripe", "Error".to_string());

        for _ in 0..5 {
            checker.record_probe_success("adyen", 100);
        }

        let availabilities = checker.get_all_availabilities();
        assert_eq!(availabilities.get("stripe"), Some(&0.9));
        assert_eq!(availabilities.get("adyen"), Some(&1.0));
    }

    #[test]
    fn test_is_healthy() {
        let config = HealthCheckConfig::new(30, 5000, 3, 3);
        let checker = HealthChecker::new(config);

        for _ in 0..3 {
            checker.record_probe_success("stripe", 100);
        }

        assert!(checker.is_healthy("stripe"));
    }

    #[test]
    fn test_reset_route() {
        let checker = HealthChecker::with_default_config();

        checker.record_probe_success("stripe", 100);
        assert_eq!(checker.get_probe_count("stripe"), 1);

        checker.reset("stripe");
        assert_eq!(checker.get_status("stripe"), HealthStatus::Unknown);
    }

    #[test]
    fn test_reset_all() {
        let checker = HealthChecker::with_default_config();

        checker.record_probe_success("stripe", 100);
        checker.record_probe_success("adyen", 100);

        checker.reset_all();
        assert_eq!(checker.get_status("stripe"), HealthStatus::Unknown);
        assert_eq!(checker.get_status("adyen"), HealthStatus::Unknown);
    }

    #[test]
    fn test_last_probe_time() {
        let checker = HealthChecker::with_default_config();

        assert!(checker.get_last_probe_time("stripe").is_none());

        checker.record_probe_success("stripe", 100);
        assert!(checker.get_last_probe_time("stripe").is_some());
    }

    #[test]
    fn test_degraded_status_calculation() {
        let checker = HealthChecker::with_default_config();

        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("stripe", "Error".to_string());
        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("stripe", "Error".to_string());
        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("stripe", "Error".to_string());
        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("stripe", "Error".to_string());
        checker.record_probe_success("stripe", 100);
        checker.record_probe_failure("stripe", "Error".to_string());

        let status = checker.get_status("stripe");
        assert_eq!(status, HealthStatus::Degraded);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let checker = HealthChecker::with_default_config();
        let checker_clone = checker.clone();

        let handle = thread::spawn(move || {
            for _ in 0..50 {
                checker_clone.record_probe_success("stripe", 100);
            }
        });

        for _ in 0..50 {
            checker.record_probe_success("stripe", 100);
        }

        handle.join().unwrap();

        assert_eq!(checker.get_probe_count("stripe"), 100);
        assert_eq!(checker.get_availability("stripe"), 1.0);
    }

    #[test]
    fn test_unknown_route() {
        let checker = HealthChecker::with_default_config();
        assert_eq!(checker.get_status("nonexistent"), HealthStatus::Unknown);
        assert_eq!(checker.get_availability("nonexistent"), 1.0);
        assert_eq!(checker.get_avg_response_time("nonexistent"), 0.0);
        assert_eq!(checker.get_probe_count("nonexistent"), 0);
    }
}
