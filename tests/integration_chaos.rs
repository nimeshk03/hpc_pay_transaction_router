use transaction_router::{
    FailureInjector, FailureType, FailureConfig,
    NetworkSimulator, NetworkCondition, LatencyProfile,
    RecoveryVerifier, RecoveryResult,
};
use std::time::Duration;

#[test]
fn test_failure_injection_workflow() {
    let injector = FailureInjector::new();
    
    let config = FailureConfig::new(FailureType::CompleteOutage, "stripe")
        .with_duration(Duration::from_secs(60))
        .with_failure_rate(1.0);
    
    injector.inject(config);
    
    assert!(injector.is_failure_active("stripe"));
    assert!(!injector.is_failure_active("adyen"));
    
    let failure = injector.should_fail("stripe");
    assert_eq!(failure, Some(FailureType::CompleteOutage));
    
    injector.remove("stripe");
    assert!(!injector.is_failure_active("stripe"));
}

#[test]
fn test_partial_failure_rate() {
    let injector = FailureInjector::new();
    
    let config = FailureConfig::new(FailureType::IntermittentFailure, "stripe")
        .with_failure_rate(0.5);
    
    injector.inject(config);
    
    let mut failures = 0;
    let mut successes = 0;
    
    for _ in 0..100 {
        if injector.should_fail("stripe").is_some() {
            failures += 1;
        } else {
            successes += 1;
        }
    }
    
    assert!(failures > 20);
    assert!(successes > 20);
}

#[test]
fn test_latency_injection() {
    let injector = FailureInjector::new();
    
    let config = FailureConfig::new(FailureType::HighLatency, "stripe")
        .with_latency(500);
    
    injector.inject(config);
    
    let latency = injector.get_injected_latency("stripe");
    assert_eq!(latency, Some(Duration::from_millis(500)));
    
    let no_latency = injector.get_injected_latency("adyen");
    assert_eq!(no_latency, None);
}

#[test]
fn test_multiple_failure_targets() {
    let injector = FailureInjector::new();
    
    injector.inject(FailureConfig::new(FailureType::CompleteOutage, "stripe"));
    injector.inject(FailureConfig::new(FailureType::Timeout, "adyen"));
    injector.inject(FailureConfig::new(FailureType::RateLimitExceeded, "checkout"));
    
    let active = injector.get_active_failures();
    assert_eq!(active.len(), 3);
    
    injector.clear_all();
    let active = injector.get_active_failures();
    assert_eq!(active.len(), 0);
}

#[test]
fn test_network_simulator_conditions() {
    let simulator = NetworkSimulator::new();
    
    simulator.set_condition("stripe", NetworkCondition::HighLatency, LatencyProfile::high_latency());
    
    assert_eq!(simulator.get_condition("stripe"), NetworkCondition::HighLatency);
    assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Normal);
}

#[test]
fn test_network_simulator_global_condition() {
    let simulator = NetworkSimulator::new();
    
    simulator.set_global_condition(NetworkCondition::Throttled, LatencyProfile::degraded());
    
    assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Throttled);
    assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Throttled);
    assert_eq!(simulator.get_condition("checkout"), NetworkCondition::Throttled);
}

#[test]
fn test_network_partition_simulation() {
    let simulator = NetworkSimulator::new();
    
    simulator.simulate_partition(&["stripe", "adyen"]);
    
    assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Partition);
    assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Partition);
    assert_eq!(simulator.get_condition("checkout"), NetworkCondition::Normal);
    
    assert!(simulator.should_drop("stripe"));
    
    simulator.heal_partition(&["stripe", "adyen"]);
    
    assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Normal);
    assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Normal);
}

#[test]
fn test_latency_profile_calculation() {
    let profile = LatencyProfile::normal();
    
    for _ in 0..10 {
        let latency = profile.calculate_latency();
        assert!(latency.as_millis() <= 20);
    }
    
    let high_latency = LatencyProfile::high_latency();
    for _ in 0..10 {
        let latency = high_latency.calculate_latency();
        assert!(latency.as_millis() >= 300);
    }
}

#[test]
fn test_recovery_verifier_basic() {
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
fn test_recovery_verifier_recovery_success() {
    let verifier = RecoveryVerifier::new()
        .with_threshold(0.9);
    
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
    let verifier = RecoveryVerifier::new()
        .with_threshold(0.99);
    
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
fn test_recovery_verifier_report() {
    let verifier = RecoveryVerifier::new();
    
    verifier.start_failure_tracking("stripe");
    verifier.record_request("stripe", false);
    verifier.start_recovery("stripe");
    verifier.record_request("stripe", true);
    verifier.complete_recovery("stripe", RecoveryResult::Success);
    
    let report = verifier.generate_report("stripe");
    assert!(report.is_some());
    let report_text = report.unwrap();
    assert!(report_text.contains("stripe"));
    assert!(report_text.contains("Success"));
}

#[test]
fn test_chaos_testing_full_scenario() {
    let injector = FailureInjector::new();
    let simulator = NetworkSimulator::new();
    let verifier = RecoveryVerifier::new();
    
    verifier.start_failure_tracking("stripe");
    injector.inject(FailureConfig::new(FailureType::CompleteOutage, "stripe"));
    simulator.set_condition("stripe", NetworkCondition::Partition, LatencyProfile::severe());
    
    for _ in 0..5 {
        let should_fail = injector.should_fail("stripe").is_some();
        verifier.record_request("stripe", !should_fail);
    }
    
    injector.remove("stripe");
    simulator.clear_condition("stripe");
    verifier.start_recovery("stripe");
    
    for _ in 0..15 {
        verifier.record_request("stripe", true);
    }
    
    let result = verifier.verify_recovery("stripe");
    assert!(result.is_successful());
}
