use transaction_router::{HealthChecker, HealthCheckConfig, HealthStatus};

#[test]
fn test_health_checker_basic_usage() {
    let checker = HealthChecker::with_default_config();
    
    checker.add_route("stripe");
    assert_eq!(checker.get_status("stripe"), HealthStatus::Unknown);
    
    checker.record_probe_success("stripe", 100);
    assert_eq!(checker.get_probe_count("stripe"), 1);
    assert_eq!(checker.get_availability("stripe"), 1.0);
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
        checker.record_probe_failure("stripe", "Connection error".to_string());
    }
    assert_eq!(checker.get_status("stripe"), HealthStatus::Unhealthy);
}

#[test]
fn test_availability_tracking() {
    let checker = HealthChecker::with_default_config();
    
    for _ in 0..80 {
        checker.record_probe_success("stripe", 100);
    }
    for _ in 0..20 {
        checker.record_probe_failure("stripe", "Timeout".to_string());
    }
    
    let availability = checker.get_availability("stripe");
    assert_eq!(availability, 0.8);
}

#[test]
fn test_degraded_status() {
    let checker = HealthChecker::with_default_config();
    
    for i in 0..20 {
        if i % 3 == 0 {
            checker.record_probe_failure("stripe", "Error".to_string());
        } else {
            checker.record_probe_success("stripe", 100);
        }
    }
    
    let status = checker.get_status("stripe");
    let availability = checker.get_availability("stripe");
    
    assert!(availability > 0.5 && availability < 0.9);
    assert!(status == HealthStatus::Degraded || status == HealthStatus::Healthy);
}

#[test]
fn test_response_time_tracking() {
    let checker = HealthChecker::with_default_config();
    
    checker.record_probe_success("stripe", 50);
    checker.record_probe_success("stripe", 100);
    checker.record_probe_success("stripe", 150);
    
    let avg_time = checker.get_avg_response_time("stripe");
    assert_eq!(avg_time, 100.0);
}

#[test]
fn test_multiple_routes() {
    let checker = HealthChecker::with_default_config();
    
    for _ in 0..5 {
        checker.record_probe_success("stripe", 100);
    }
    
    for _ in 0..3 {
        checker.record_probe_success("adyen", 150);
    }
    for _ in 0..2 {
        checker.record_probe_failure("adyen", "Error".to_string());
    }
    
    let statuses = checker.get_all_statuses();
    assert_eq!(statuses.len(), 2);
    
    let availabilities = checker.get_all_availabilities();
    assert_eq!(availabilities.get("stripe"), Some(&1.0));
    assert_eq!(availabilities.get("adyen"), Some(&0.6));
}

#[test]
fn test_rolling_window_eviction() {
    let config = HealthCheckConfig::new(30, 5000, 3, 3).with_window_size(10);
    let checker = HealthChecker::new(config);
    
    for _ in 0..20 {
        checker.record_probe_success("stripe", 100);
    }
    
    assert_eq!(checker.get_probe_count("stripe"), 10);
}

#[test]
fn test_consecutive_success_threshold() {
    let config = HealthCheckConfig::new(30, 5000, 5, 3);
    let checker = HealthChecker::new(config);
    
    checker.record_probe_failure("stripe", "Error".to_string());
    for _ in 0..4 {
        checker.record_probe_success("stripe", 100);
    }
    
    checker.record_probe_success("stripe", 100);
    assert_eq!(checker.get_status("stripe"), HealthStatus::Healthy);
}

#[test]
fn test_consecutive_failure_threshold() {
    let config = HealthCheckConfig::new(30, 5000, 3, 5);
    let checker = HealthChecker::new(config);
    
    checker.record_probe_success("stripe", 100);
    for _ in 0..4 {
        checker.record_probe_failure("stripe", "Error".to_string());
    }
    
    checker.record_probe_failure("stripe", "Error".to_string());
    assert_eq!(checker.get_status("stripe"), HealthStatus::Unhealthy);
}

#[test]
fn test_slow_response_degradation() {
    let config = HealthCheckConfig::new(30, 1000, 3, 3);
    let checker = HealthChecker::new(config);
    
    checker.record_probe_success("stripe", 1500);
    checker.record_probe_success("stripe", 1200);
    checker.record_probe_success("stripe", 1800);
    
    assert_eq!(checker.get_probe_count("stripe"), 3);
}

#[test]
fn test_reset_functionality() {
    let checker = HealthChecker::with_default_config();
    
    checker.record_probe_success("stripe", 100);
    checker.record_probe_success("adyen", 100);
    
    assert_eq!(checker.get_probe_count("stripe"), 1);
    assert_eq!(checker.get_probe_count("adyen"), 1);
    
    checker.reset("stripe");
    assert_eq!(checker.get_status("stripe"), HealthStatus::Unknown);
    assert_eq!(checker.get_probe_count("adyen"), 1);
    
    checker.reset_all();
    assert_eq!(checker.get_status("adyen"), HealthStatus::Unknown);
}

#[test]
fn test_is_healthy_check() {
    let config = HealthCheckConfig::new(30, 5000, 3, 3);
    let checker = HealthChecker::new(config);
    
    assert!(!checker.is_healthy("stripe"));
    
    for _ in 0..3 {
        checker.record_probe_success("stripe", 100);
    }
    
    assert!(checker.is_healthy("stripe"));
}

#[test]
fn test_last_probe_time_tracking() {
    let checker = HealthChecker::with_default_config();
    
    assert!(checker.get_last_probe_time("stripe").is_none());
    
    checker.record_probe_success("stripe", 100);
    let first_time = checker.get_last_probe_time("stripe");
    assert!(first_time.is_some());
    
    std::thread::sleep(std::time::Duration::from_millis(10));
    
    checker.record_probe_success("stripe", 100);
    let second_time = checker.get_last_probe_time("stripe");
    assert!(second_time.is_some());
    assert!(second_time.unwrap() > first_time.unwrap());
}

#[test]
fn test_high_volume_probes() {
    let checker = HealthChecker::with_default_config();
    
    for i in 0..1000 {
        if i % 10 == 0 {
            checker.record_probe_failure("stripe", "Error".to_string());
        } else {
            checker.record_probe_success("stripe", 100);
        }
    }
    
    let availability = checker.get_availability("stripe");
    assert!(availability > 0.85 && availability < 0.95);
}

#[test]
fn test_concurrent_health_checks() {
    use std::thread;
    
    let config = HealthCheckConfig::new(30, 5000, 3, 3).with_window_size(500);
    let checker = HealthChecker::new(config);
    let mut handles = vec![];
    
    for i in 0..10 {
        let checker_clone = checker.clone();
        let handle = thread::spawn(move || {
            for _ in 0..50 {
                if i % 2 == 0 {
                    checker_clone.record_probe_success("stripe", 100);
                } else {
                    checker_clone.record_probe_failure("stripe", "Error".to_string());
                }
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(checker.get_probe_count("stripe"), 500);
}

#[test]
fn test_recovery_from_unhealthy() {
    let config = HealthCheckConfig::new(30, 5000, 3, 3);
    let checker = HealthChecker::new(config);
    
    for _ in 0..3 {
        checker.record_probe_failure("stripe", "Error".to_string());
    }
    assert_eq!(checker.get_status("stripe"), HealthStatus::Unhealthy);
    
    for _ in 0..3 {
        checker.record_probe_success("stripe", 100);
    }
    assert_eq!(checker.get_status("stripe"), HealthStatus::Healthy);
}

#[test]
fn test_mixed_success_failure_patterns() {
    let checker = HealthChecker::with_default_config();
    
    checker.record_probe_success("stripe", 100);
    checker.record_probe_success("stripe", 100);
    checker.record_probe_failure("stripe", "Error".to_string());
    checker.record_probe_success("stripe", 100);
    checker.record_probe_failure("stripe", "Error".to_string());
    checker.record_probe_success("stripe", 100);
    
    let availability = checker.get_availability("stripe");
    assert!(availability > 0.6 && availability < 0.7);
}
