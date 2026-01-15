use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
        }
    }
}

impl CircuitBreakerConfig {
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        timeout_secs: u64,
        half_open_max_calls: u32,
    ) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            timeout: Duration::from_secs(timeout_secs),
            half_open_max_calls,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerMetrics {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub rejected_calls: u64,
    pub state_transitions: u64,
    consecutive_failures: u32,
    consecutive_successes: u32,
    half_open_calls: u32,
}

impl CircuitBreakerMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failure_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.failed_calls as f64 / self.total_calls as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.successful_calls as f64 / self.total_calls as f64
        }
    }

    fn record_success(&mut self) {
        self.total_calls += 1;
        self.successful_calls += 1;
        self.consecutive_successes += 1;
        self.consecutive_failures = 0;
    }

    fn record_failure(&mut self) {
        self.total_calls += 1;
        self.failed_calls += 1;
        self.consecutive_failures += 1;
        self.consecutive_successes = 0;
    }

    fn record_rejection(&mut self) {
        self.rejected_calls += 1;
    }

    fn record_transition(&mut self) {
        self.state_transitions += 1;
    }

    fn reset_consecutive_counts(&mut self) {
        self.consecutive_failures = 0;
        self.consecutive_successes = 0;
    }

    fn reset_half_open_calls(&mut self) {
        self.half_open_calls = 0;
    }

    fn increment_half_open_calls(&mut self) {
        self.half_open_calls += 1;
    }
}

struct CircuitBreakerState {
    state: CircuitState,
    last_state_change: Instant,
    metrics: CircuitBreakerMetrics,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            last_state_change: Instant::now(),
            metrics: CircuitBreakerMetrics::new(),
        }
    }

    fn with_state(state: CircuitState) -> Self {
        Self {
            state,
            last_state_change: Instant::now(),
            metrics: CircuitBreakerMetrics::new(),
        }
    }
}

#[derive(Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<Mutex<CircuitBreakerState>>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(CircuitBreakerState::new())),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    pub fn with_state(config: CircuitBreakerConfig, initial_state: CircuitState) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(CircuitBreakerState::with_state(initial_state))),
        }
    }

    pub fn state(&self) -> CircuitState {
        let mut state = self.state.lock().unwrap();
        let current_state = self.check_and_update_state(&state);
        
        if current_state == CircuitState::HalfOpen && state.state == CircuitState::Open {
            self.transition_to(&mut state, CircuitState::HalfOpen);
        }
        
        state.state
    }

    pub fn is_call_permitted(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        let current_state = self.check_and_update_state(&state);

        match current_state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                state.metrics.record_rejection();
                false
            }
            CircuitState::HalfOpen => {
                if state.metrics.half_open_calls < self.config.half_open_max_calls {
                    state.metrics.increment_half_open_calls();
                    true
                } else {
                    state.metrics.record_rejection();
                    false
                }
            }
        }
    }

    pub fn record_success(&self) {
        let mut state = self.state.lock().unwrap();
        state.metrics.record_success();

        match state.state {
            CircuitState::Closed => {},
            CircuitState::Open => {},
            CircuitState::HalfOpen => {
                if state.metrics.consecutive_successes >= self.config.success_threshold {
                    self.transition_to(&mut state, CircuitState::Closed);
                }
            }
        }
    }

    pub fn record_failure(&self) {
        let mut state = self.state.lock().unwrap();
        state.metrics.record_failure();

        match state.state {
            CircuitState::Closed => {
                if state.metrics.consecutive_failures >= self.config.failure_threshold {
                    self.transition_to(&mut state, CircuitState::Open);
                }
            }
            CircuitState::Open => {},
            CircuitState::HalfOpen => {
                self.transition_to(&mut state, CircuitState::Open);
            }
        }
    }

    pub fn force_open(&self) {
        let mut state = self.state.lock().unwrap();
        self.transition_to(&mut state, CircuitState::Open);
    }

    pub fn force_closed(&self) {
        let mut state = self.state.lock().unwrap();
        self.transition_to(&mut state, CircuitState::Closed);
    }

    pub fn force_half_open(&self) {
        let mut state = self.state.lock().unwrap();
        self.transition_to(&mut state, CircuitState::HalfOpen);
    }

    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        let transitions = state.metrics.state_transitions;
        state.metrics = CircuitBreakerMetrics::new();
        state.metrics.state_transitions = transitions;
        self.transition_to(&mut state, CircuitState::Closed);
    }

    pub fn get_metrics(&self) -> CircuitBreakerMetrics {
        let state = self.state.lock().unwrap();
        state.metrics.clone()
    }

    pub fn get_config(&self) -> CircuitBreakerConfig {
        self.config.clone()
    }

    fn check_and_update_state(&self, state: &CircuitBreakerState) -> CircuitState {
        match state.state {
            CircuitState::Open => {
                let elapsed = state.last_state_change.elapsed();
                if elapsed >= self.config.timeout {
                    CircuitState::HalfOpen
                } else {
                    CircuitState::Open
                }
            }
            _ => state.state,
        }
    }

    fn transition_to(&self, state: &mut CircuitBreakerState, new_state: CircuitState) {
        if state.state != new_state {
            state.state = new_state;
            state.last_state_change = Instant::now();
            state.metrics.record_transition();

            match new_state {
                CircuitState::Closed => {
                    state.metrics.reset_consecutive_counts();
                    state.metrics.reset_half_open_calls();
                }
                CircuitState::Open => {
                    state.metrics.reset_half_open_calls();
                }
                CircuitState::HalfOpen => {
                    state.metrics.reset_consecutive_counts();
                    state.metrics.reset_half_open_calls();
                }
            }
        }
    }

    pub fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if !self.is_call_permitted() {
            return Err(CircuitBreakerError::Rejected);
        }

        match f() {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(err) => {
                self.record_failure();
                Err(CircuitBreakerError::CallFailed(err))
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CircuitBreakerError<E> {
    Rejected,
    CallFailed(E),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_initial_state_is_closed() {
        let breaker = CircuitBreaker::with_default_config();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_transition_to_open_after_threshold_failures() {
        let config = CircuitBreakerConfig::new(3, 2, 60, 3);
        let breaker = CircuitBreaker::new(config);

        for _ in 0..2 {
            breaker.record_failure();
            assert_eq!(breaker.state(), CircuitState::Closed);
        }

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[test]
    fn test_call_rejected_when_open() {
        let config = CircuitBreakerConfig::new(1, 2, 60, 3);
        let breaker = CircuitBreaker::new(config);

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        assert!(!breaker.is_call_permitted());

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.rejected_calls, 1);
    }

    #[test]
    fn test_transition_to_half_open_after_timeout() {
        let config = CircuitBreakerConfig::new(1, 2, 1, 3);
        let breaker = CircuitBreaker::new(config);

        breaker.force_open();
        assert_eq!(breaker.state(), CircuitState::Open);

        thread::sleep(Duration::from_millis(1100));

        assert_eq!(breaker.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_half_open_to_closed_on_success() {
        let config = CircuitBreakerConfig::new(3, 2, 60, 3);
        let breaker = CircuitBreaker::with_state(config, CircuitState::HalfOpen);

        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_to_open_on_failure() {
        let config = CircuitBreakerConfig::new(3, 2, 60, 3);
        let breaker = CircuitBreaker::with_state(config, CircuitState::HalfOpen);

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[test]
    fn test_half_open_limits_calls() {
        let config = CircuitBreakerConfig::new(3, 2, 60, 2);
        let breaker = CircuitBreaker::with_state(config, CircuitState::HalfOpen);

        assert!(breaker.is_call_permitted());
        assert!(breaker.is_call_permitted());
        assert!(!breaker.is_call_permitted());

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.rejected_calls, 1);
    }

    #[test]
    fn test_metrics_tracking() {
        let breaker = CircuitBreaker::with_default_config();

        breaker.record_success();
        breaker.record_success();
        breaker.record_failure();

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.total_calls, 3);
        assert_eq!(metrics.successful_calls, 2);
        assert_eq!(metrics.failed_calls, 1);
        assert_eq!(metrics.success_rate(), 2.0 / 3.0);
        assert_eq!(metrics.failure_rate(), 1.0 / 3.0);
    }

    #[test]
    fn test_consecutive_failures_reset_on_success() {
        let config = CircuitBreakerConfig::new(3, 2, 60, 3);
        let breaker = CircuitBreaker::new(config);

        breaker.record_failure();
        breaker.record_failure();
        breaker.record_success();
        breaker.record_failure();
        breaker.record_failure();

        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_force_open() {
        let breaker = CircuitBreaker::with_default_config();
        assert_eq!(breaker.state(), CircuitState::Closed);

        breaker.force_open();
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[test]
    fn test_force_closed() {
        let breaker = CircuitBreaker::with_default_config();
        breaker.force_open();
        assert_eq!(breaker.state(), CircuitState::Open);

        breaker.force_closed();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_force_half_open() {
        let breaker = CircuitBreaker::with_default_config();
        breaker.force_half_open();
        assert_eq!(breaker.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_reset() {
        let config = CircuitBreakerConfig::new(2, 2, 60, 3);
        let breaker = CircuitBreaker::new(config);

        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        breaker.reset();
        assert_eq!(breaker.state(), CircuitState::Closed);

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.total_calls, 0);
        assert_eq!(metrics.failed_calls, 0);
    }

    #[test]
    fn test_call_wrapper_success() {
        let breaker = CircuitBreaker::with_default_config();

        let result = breaker.call(|| Ok::<i32, String>(42));
        assert_eq!(result, Ok(42));

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.successful_calls, 1);
    }

    #[test]
    fn test_call_wrapper_failure() {
        let breaker = CircuitBreaker::with_default_config();

        let result = breaker.call(|| Err::<i32, String>("error".to_string()));
        assert_eq!(result, Err(CircuitBreakerError::CallFailed("error".to_string())));

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.failed_calls, 1);
    }

    #[test]
    fn test_call_wrapper_rejected() {
        let config = CircuitBreakerConfig::new(1, 2, 60, 3);
        let breaker = CircuitBreaker::new(config);

        breaker.force_open();

        let result = breaker.call(|| Ok::<i32, String>(42));
        assert_eq!(result, Err(CircuitBreakerError::Rejected));

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.rejected_calls, 1);
    }

    #[test]
    fn test_state_transitions_counted() {
        let config = CircuitBreakerConfig::new(2, 2, 60, 3);
        let breaker = CircuitBreaker::new(config);

        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        breaker.force_half_open();
        breaker.record_success();
        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.state_transitions, 3);
    }

    #[test]
    fn test_custom_config() {
        let config = CircuitBreakerConfig::new(10, 5, 120, 5);
        let breaker = CircuitBreaker::new(config.clone());

        let retrieved_config = breaker.get_config();
        assert_eq!(retrieved_config.failure_threshold, 10);
        assert_eq!(retrieved_config.success_threshold, 5);
        assert_eq!(retrieved_config.timeout, Duration::from_secs(120));
        assert_eq!(retrieved_config.half_open_max_calls, 5);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;

        let breaker = Arc::new(CircuitBreaker::with_default_config());
        let mut handles = vec![];

        for i in 0..10 {
            let breaker_clone = Arc::clone(&breaker);
            let handle = thread::spawn(move || {
                if i % 2 == 0 {
                    breaker_clone.record_success();
                } else {
                    breaker_clone.record_failure();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let metrics = breaker.get_metrics();
        assert_eq!(metrics.total_calls, 10);
        assert_eq!(metrics.successful_calls, 5);
        assert_eq!(metrics.failed_calls, 5);
    }
}
