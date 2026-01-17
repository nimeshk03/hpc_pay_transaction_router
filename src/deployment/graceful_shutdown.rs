use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownState {
    Running,
    ShuttingDown,
    Draining,
    Terminated,
}

#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    pub graceful_timeout: Duration,
    pub drain_timeout: Duration,
    pub force_shutdown_timeout: Duration,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(30),
            drain_timeout: Duration::from_secs(15),
            force_shutdown_timeout: Duration::from_secs(60),
        }
    }
}

impl ShutdownConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_graceful_timeout(mut self, timeout: Duration) -> Self {
        self.graceful_timeout = timeout;
        self
    }

    pub fn with_drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }

    pub fn with_force_timeout(mut self, timeout: Duration) -> Self {
        self.force_shutdown_timeout = timeout;
        self
    }
}

pub struct GracefulShutdown {
    config: ShutdownConfig,
    shutdown_requested: Arc<AtomicBool>,
    in_flight_requests: Arc<AtomicU64>,
    shutdown_start: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl GracefulShutdown {
    pub fn new(config: ShutdownConfig) -> Self {
        Self {
            config,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            in_flight_requests: Arc::new(AtomicU64::new(0)),
            shutdown_start: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(ShutdownConfig::default())
    }

    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        let mut start = self.shutdown_start.lock().unwrap();
        if start.is_none() {
            *start = Some(Instant::now());
        }
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    pub fn start_request(&self) -> bool {
        if self.is_shutdown_requested() {
            return false;
        }
        self.in_flight_requests.fetch_add(1, Ordering::SeqCst);
        true
    }

    pub fn end_request(&self) {
        let current = self.in_flight_requests.load(Ordering::SeqCst);
        if current > 0 {
            self.in_flight_requests.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn in_flight_count(&self) -> u64 {
        self.in_flight_requests.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> ShutdownState {
        if !self.is_shutdown_requested() {
            return ShutdownState::Running;
        }

        let start = self.shutdown_start.lock().unwrap();
        if let Some(start_time) = *start {
            let elapsed = start_time.elapsed();

            if elapsed >= self.config.force_shutdown_timeout {
                return ShutdownState::Terminated;
            }

            if self.in_flight_count() == 0 {
                return ShutdownState::Terminated;
            }

            if elapsed >= self.config.graceful_timeout {
                return ShutdownState::Draining;
            }

            return ShutdownState::ShuttingDown;
        }

        ShutdownState::ShuttingDown
    }

    pub fn can_accept_requests(&self) -> bool {
        matches!(self.state(), ShutdownState::Running)
    }

    pub fn is_terminated(&self) -> bool {
        matches!(self.state(), ShutdownState::Terminated)
    }

    pub fn wait_for_drain(&self) -> bool {
        let start = Instant::now();
        let total_timeout = self.config.graceful_timeout + self.config.drain_timeout;

        while start.elapsed() < total_timeout {
            if self.in_flight_count() == 0 {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        false
    }

    pub fn time_until_force_shutdown(&self) -> Option<Duration> {
        let start = self.shutdown_start.lock().unwrap();
        if let Some(start_time) = *start {
            let elapsed = start_time.elapsed();
            if elapsed < self.config.force_shutdown_timeout {
                return Some(self.config.force_shutdown_timeout - elapsed);
            }
            return Some(Duration::ZERO);
        }
        None
    }

    pub fn config(&self) -> &ShutdownConfig {
        &self.config
    }

    pub fn reset(&self) {
        self.shutdown_requested.store(false, Ordering::SeqCst);
        self.in_flight_requests.store(0, Ordering::SeqCst);
        let mut start = self.shutdown_start.lock().unwrap();
        *start = None;
    }
}

impl Clone for GracefulShutdown {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            shutdown_requested: self.shutdown_requested.clone(),
            in_flight_requests: self.in_flight_requests.clone(),
            shutdown_start: self.shutdown_start.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_config_default() {
        let config = ShutdownConfig::default();
        assert_eq!(config.graceful_timeout, Duration::from_secs(30));
        assert_eq!(config.drain_timeout, Duration::from_secs(15));
    }

    #[test]
    fn test_shutdown_config_builder() {
        let config = ShutdownConfig::new()
            .with_graceful_timeout(Duration::from_secs(60))
            .with_drain_timeout(Duration::from_secs(30));

        assert_eq!(config.graceful_timeout, Duration::from_secs(60));
        assert_eq!(config.drain_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_graceful_shutdown_initial_state() {
        let shutdown = GracefulShutdown::with_default_config();

        assert_eq!(shutdown.state(), ShutdownState::Running);
        assert!(shutdown.can_accept_requests());
        assert!(!shutdown.is_shutdown_requested());
    }

    #[test]
    fn test_graceful_shutdown_request() {
        let shutdown = GracefulShutdown::with_default_config();

        shutdown.request_shutdown();

        assert!(shutdown.is_shutdown_requested());
        assert!(!shutdown.can_accept_requests());
    }

    #[test]
    fn test_graceful_shutdown_in_flight_tracking() {
        let shutdown = GracefulShutdown::with_default_config();

        assert!(shutdown.start_request());
        assert!(shutdown.start_request());
        assert_eq!(shutdown.in_flight_count(), 2);

        shutdown.end_request();
        assert_eq!(shutdown.in_flight_count(), 1);

        shutdown.end_request();
        assert_eq!(shutdown.in_flight_count(), 0);
    }

    #[test]
    fn test_graceful_shutdown_reject_new_requests() {
        let shutdown = GracefulShutdown::with_default_config();

        assert!(shutdown.start_request());

        shutdown.request_shutdown();

        assert!(!shutdown.start_request());
        assert_eq!(shutdown.in_flight_count(), 1);
    }

    #[test]
    fn test_graceful_shutdown_state_transitions() {
        let config = ShutdownConfig::new()
            .with_graceful_timeout(Duration::from_millis(50))
            .with_force_timeout(Duration::from_millis(100));

        let shutdown = GracefulShutdown::new(config);

        assert_eq!(shutdown.state(), ShutdownState::Running);

        shutdown.start_request();
        shutdown.request_shutdown();

        assert_eq!(shutdown.state(), ShutdownState::ShuttingDown);
    }

    #[test]
    fn test_graceful_shutdown_terminated_when_drained() {
        let shutdown = GracefulShutdown::with_default_config();

        shutdown.request_shutdown();

        assert_eq!(shutdown.state(), ShutdownState::Terminated);
        assert!(shutdown.is_terminated());
    }

    #[test]
    fn test_graceful_shutdown_reset() {
        let shutdown = GracefulShutdown::with_default_config();

        shutdown.start_request();
        shutdown.request_shutdown();

        shutdown.reset();

        assert_eq!(shutdown.state(), ShutdownState::Running);
        assert_eq!(shutdown.in_flight_count(), 0);
    }

    #[test]
    fn test_graceful_shutdown_clone() {
        let shutdown1 = GracefulShutdown::with_default_config();
        shutdown1.start_request();

        let shutdown2 = shutdown1.clone();

        assert_eq!(shutdown2.in_flight_count(), 1);

        shutdown1.request_shutdown();
        assert!(shutdown2.is_shutdown_requested());
    }
}
