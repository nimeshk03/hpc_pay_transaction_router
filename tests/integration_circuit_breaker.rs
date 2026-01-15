use std::thread;
use std::time::Duration;
use transaction_router::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

#[test]
fn test_circuit_breaker_full_lifecycle() {
    let config = CircuitBreakerConfig::new(3, 2, 1, 3);
    let breaker = CircuitBreaker::new(config);

    assert_eq!(breaker.state(), CircuitState::Closed);

    for _ in 0..3 {
        breaker.record_failure();
    }
    assert_eq!(breaker.state(), CircuitState::Open);

    thread::sleep(Duration::from_millis(1100));
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    breaker.record_success();
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_prevents_cascading_failures() {
    let config = CircuitBreakerConfig::new(5, 2, 60, 3);
    let breaker = CircuitBreaker::new(config);

    for _ in 0..5 {
        breaker.record_failure();
    }
    assert_eq!(breaker.state(), CircuitState::Open);

    for _ in 0..10 {
        assert!(!breaker.is_call_permitted());
    }

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.rejected_calls, 10);
    assert_eq!(metrics.failed_calls, 5);
}

#[test]
fn test_circuit_breaker_with_call_wrapper() {
    let config = CircuitBreakerConfig::new(2, 2, 60, 3);
    let breaker = CircuitBreaker::new(config);

    let result = breaker.call(|| Ok::<i32, String>(42));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);

    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    let result = breaker.call(|| Ok::<i32, String>(100));
    assert!(result.is_err());

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.successful_calls, 1);
    assert_eq!(metrics.failed_calls, 2);
    assert_eq!(metrics.rejected_calls, 1);
}

#[test]
fn test_circuit_breaker_half_open_recovery() {
    let config = CircuitBreakerConfig::new(2, 3, 1, 5);
    let breaker = CircuitBreaker::new(config);

    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    thread::sleep(Duration::from_millis(1100));
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    for _ in 0..3 {
        assert!(breaker.is_call_permitted());
        breaker.record_success();
    }

    assert_eq!(breaker.state(), CircuitState::Closed);

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.state_transitions, 3);
}

#[test]
fn test_circuit_breaker_half_open_failure_reopens() {
    let config = CircuitBreakerConfig::new(2, 2, 1, 3);
    let breaker = CircuitBreaker::new(config);

    breaker.force_open();
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.state_transitions, 3);
}

#[test]
fn test_circuit_breaker_metrics_accuracy() {
    let config = CircuitBreakerConfig::new(10, 2, 60, 3);
    let breaker = CircuitBreaker::new(config);

    for _ in 0..7 {
        breaker.record_success();
    }

    for _ in 0..3 {
        breaker.record_failure();
    }

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.total_calls, 10);
    assert_eq!(metrics.successful_calls, 7);
    assert_eq!(metrics.failed_calls, 3);
    assert_eq!(metrics.success_rate(), 0.7);
    assert_eq!(metrics.failure_rate(), 0.3);
}

#[test]
fn test_circuit_breaker_reset_functionality() {
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
    assert_eq!(metrics.state_transitions, 2);
}

#[test]
fn test_circuit_breaker_concurrent_operations() {
    use std::sync::Arc;

    let config = CircuitBreakerConfig::new(20, 2, 60, 10);
    let breaker = Arc::new(CircuitBreaker::new(config));
    let mut handles = vec![];

    for i in 0..50 {
        let breaker_clone = Arc::clone(&breaker);
        let handle = thread::spawn(move || {
            if i % 3 == 0 {
                breaker_clone.record_failure();
            } else {
                breaker_clone.record_success();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.total_calls, 50);
    assert!(metrics.successful_calls > 0);
    assert!(metrics.failed_calls > 0);
}

#[test]
fn test_circuit_breaker_half_open_call_limit() {
    let config = CircuitBreakerConfig::new(1, 2, 1, 2);
    let breaker = CircuitBreaker::new(config);

    breaker.force_open();
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    assert!(breaker.is_call_permitted());
    assert!(breaker.is_call_permitted());
    assert!(!breaker.is_call_permitted());
    assert!(!breaker.is_call_permitted());

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.rejected_calls, 2);
}

#[test]
fn test_circuit_breaker_force_state_transitions() {
    let breaker = CircuitBreaker::with_default_config();

    assert_eq!(breaker.state(), CircuitState::Closed);

    breaker.force_open();
    assert_eq!(breaker.state(), CircuitState::Open);

    breaker.force_half_open();
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    breaker.force_closed();
    assert_eq!(breaker.state(), CircuitState::Closed);

    let metrics = breaker.get_metrics();
    assert_eq!(metrics.state_transitions, 3);
}
