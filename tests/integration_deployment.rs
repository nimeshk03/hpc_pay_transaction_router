use transaction_router::{
    HealthEndpoint, ComponentHealth,
    GracefulShutdown, ShutdownConfig, ShutdownState,
    ReadinessChecker, ReadinessState,
};
use std::time::Duration;

#[test]
fn test_health_endpoint_basic() {
    let endpoint = HealthEndpoint::new("1.0.0");
    
    let health = endpoint.get_health();
    assert!(health.status.is_healthy());
    assert_eq!(health.version, "1.0.0");
}

#[test]
fn test_health_endpoint_with_components() {
    let endpoint = HealthEndpoint::new("1.0.0");
    
    endpoint.register_component("database");
    endpoint.register_component("cache");
    endpoint.register_component("queue");
    
    let health = endpoint.get_health();
    assert!(health.status.is_healthy());
    assert_eq!(health.components.len(), 3);
}

#[test]
fn test_health_endpoint_degraded_component() {
    let endpoint = HealthEndpoint::new("1.0.0");
    
    endpoint.register_component("database");
    endpoint.register_component("cache");
    
    endpoint.set_component_degraded("cache", "High latency detected");
    
    let health = endpoint.get_health();
    assert!(health.status.is_healthy());
    
    let cache_health = health.components.iter().find(|c| c.name == "cache").unwrap();
    assert_eq!(cache_health.message, Some("High latency detected".to_string()));
}

#[test]
fn test_health_endpoint_unhealthy_component() {
    let endpoint = HealthEndpoint::new("1.0.0");
    
    endpoint.register_component("database");
    endpoint.register_component("cache");
    
    endpoint.set_component_unhealthy("database", "Connection failed");
    
    let health = endpoint.get_health();
    assert!(!health.status.is_healthy());
}

#[test]
fn test_health_endpoint_component_recovery() {
    let endpoint = HealthEndpoint::new("1.0.0");
    
    endpoint.register_component("database");
    endpoint.set_component_unhealthy("database", "Connection failed");
    
    assert!(!endpoint.is_healthy());
    
    endpoint.set_component_healthy("database");
    
    assert!(endpoint.is_healthy());
}

#[test]
fn test_health_endpoint_uptime() {
    let endpoint = HealthEndpoint::new("1.0.0");
    
    std::thread::sleep(Duration::from_millis(10));
    
    let uptime = endpoint.uptime();
    assert!(uptime.as_millis() >= 10);
}

#[test]
fn test_component_health_with_details() {
    let health = ComponentHealth::healthy("database")
        .with_detail("connections", "10")
        .with_detail("latency_ms", "5");
    
    assert_eq!(health.details.get("connections"), Some(&"10".to_string()));
    assert_eq!(health.details.get("latency_ms"), Some(&"5".to_string()));
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
    assert!(shutdown.start_request());
    
    assert_eq!(shutdown.in_flight_count(), 3);
    
    shutdown.end_request();
    assert_eq!(shutdown.in_flight_count(), 2);
    
    shutdown.end_request();
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
fn test_graceful_shutdown_terminated_when_drained() {
    let shutdown = GracefulShutdown::with_default_config();
    
    shutdown.request_shutdown();
    
    assert_eq!(shutdown.state(), ShutdownState::Terminated);
    assert!(shutdown.is_terminated());
}

#[test]
fn test_graceful_shutdown_config() {
    let config = ShutdownConfig::new()
        .with_graceful_timeout(Duration::from_secs(60))
        .with_drain_timeout(Duration::from_secs(30))
        .with_force_timeout(Duration::from_secs(120));
    
    let shutdown = GracefulShutdown::new(config);
    
    assert_eq!(shutdown.config().graceful_timeout, Duration::from_secs(60));
    assert_eq!(shutdown.config().drain_timeout, Duration::from_secs(30));
}

#[test]
fn test_graceful_shutdown_reset() {
    let shutdown = GracefulShutdown::with_default_config();
    
    shutdown.start_request();
    shutdown.request_shutdown();
    
    shutdown.reset();
    
    assert_eq!(shutdown.state(), ShutdownState::Running);
    assert_eq!(shutdown.in_flight_count(), 0);
    assert!(shutdown.can_accept_requests());
}

#[test]
fn test_readiness_checker_empty() {
    let checker = ReadinessChecker::new()
        .with_grace_period(Duration::ZERO);
    
    assert!(checker.is_ready());
    assert_eq!(checker.state(), ReadinessState::Ready);
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
fn test_readiness_checker_probe_recovery() {
    let checker = ReadinessChecker::new()
        .with_grace_period(Duration::ZERO);
    
    checker.register_probe("database");
    
    checker.set_not_ready("database", "Connection failed");
    assert!(!checker.is_ready());
    
    checker.set_ready("database");
    assert!(checker.is_ready());
    
    let probe = checker.get_probe("database").unwrap();
    assert_eq!(probe.consecutive_failures, 0);
}

#[test]
fn test_deployment_integration_scenario() {
    let health = HealthEndpoint::new("1.0.0");
    let shutdown = GracefulShutdown::with_default_config();
    let readiness = ReadinessChecker::new()
        .with_grace_period(Duration::ZERO);
    
    health.register_component("database");
    health.register_component("cache");
    readiness.register_probe("database");
    readiness.register_probe("cache");
    
    health.set_component_healthy("database");
    health.set_component_healthy("cache");
    readiness.set_ready("database");
    readiness.set_ready("cache");
    
    assert!(health.is_healthy());
    assert!(readiness.is_ready());
    assert!(shutdown.can_accept_requests());
    
    for _ in 0..5 {
        shutdown.start_request();
    }
    
    shutdown.request_shutdown();
    
    assert!(!shutdown.can_accept_requests());
    assert_eq!(shutdown.in_flight_count(), 5);
    
    for _ in 0..5 {
        shutdown.end_request();
    }
    
    assert!(shutdown.is_terminated());
}

#[test]
fn test_concurrent_shutdown_handling() {
    use std::thread;
    
    let shutdown = GracefulShutdown::with_default_config();
    let mut handles = vec![];
    
    for _ in 0..10 {
        let shutdown_clone = shutdown.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if shutdown_clone.start_request() {
                    std::thread::sleep(Duration::from_micros(10));
                    shutdown_clone.end_request();
                }
            }
        });
        handles.push(handle);
    }
    
    std::thread::sleep(Duration::from_millis(5));
    shutdown.request_shutdown();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert!(shutdown.is_shutdown_requested());
}
