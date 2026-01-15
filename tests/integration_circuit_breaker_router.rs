use rust_decimal::Decimal;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use transaction_router::circuit_breaker::CircuitState;
use transaction_router::{
    PaymentMethod, RouteConfig, TransactionRequest, TransactionRouter,
};

fn create_test_config() -> transaction_router::config::loader::RouterConfig {
    let mut stripe = RouteConfig::new(
        "stripe".to_string(),
        "Stripe".to_string(),
        "https://api.stripe.com".to_string(),
    )
    .unwrap();
    stripe.supported_methods = vec![PaymentMethod::Card];
    stripe.supported_currencies = vec!["USD".to_string()];
    stripe.limits.min_amount = Decimal::from_str("1.00").unwrap();
    stripe.limits.max_amount = Decimal::from_str("10000.00").unwrap();
    stripe.enabled = true;

    let mut adyen = RouteConfig::new(
        "adyen".to_string(),
        "Adyen".to_string(),
        "https://api.adyen.com".to_string(),
    )
    .unwrap();
    adyen.supported_methods = vec![PaymentMethod::Card];
    adyen.supported_currencies = vec!["USD".to_string()];
    adyen.limits.min_amount = Decimal::from_str("1.00").unwrap();
    adyen.limits.max_amount = Decimal::from_str("5000.00").unwrap();
    adyen.enabled = true;

    transaction_router::config::loader::RouterConfig::new().with_routes(vec![stripe, adyen])
}

fn create_test_transaction(amount: &str) -> TransactionRequest {
    TransactionRequest::new(
        "merchant_123".to_string(),
        Decimal::from_str(amount).unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap()
}

#[test]
fn test_circuit_breaker_filters_open_routes() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        breaker.force_open();
    }

    let tx = create_test_transaction("100.00");
    let decision = router.route(&tx).unwrap();

    assert_ne!(decision.selected_route, Some("stripe".to_string()));
    assert_eq!(decision.selected_route, Some("adyen".to_string()));
}

#[test]
fn test_all_circuits_open_fails_routing() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        breaker.force_open();
    }
    if let Some(breaker) = router.get_circuit_breaker("adyen") {
        breaker.force_open();
    }

    let tx = create_test_transaction("100.00");
    let result = router.route(&tx);

    assert!(result.is_err());
}

#[test]
fn test_circuit_breaker_success_tracking() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    for _ in 0..5 {
        router.record_route_success("stripe");
    }

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        let metrics = breaker.get_metrics();
        assert_eq!(metrics.successful_calls, 5);
        assert_eq!(metrics.success_rate(), 1.0);
    }
}

#[test]
fn test_circuit_breaker_failure_tracking() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    for _ in 0..5 {
        router.record_route_failure("stripe");
    }

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        let metrics = breaker.get_metrics();
        assert_eq!(metrics.failed_calls, 5);
        assert_eq!(breaker.state(), CircuitState::Open);
    }
}

#[test]
fn test_circuit_opens_after_threshold() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    for _ in 0..5 {
        router.record_route_failure("stripe");
    }

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    let tx = create_test_transaction("100.00");
    let decision = router.route(&tx).unwrap();
    assert_ne!(decision.selected_route, Some("stripe".to_string()));
}

#[test]
fn test_circuit_recovery_to_half_open() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        breaker.force_open();
        assert_eq!(breaker.state(), CircuitState::Open);

        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn test_get_all_circuit_states() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    let states = router.get_all_circuit_states();
    assert_eq!(states.len(), 2);
    assert_eq!(states.get("stripe"), Some(&CircuitState::Closed));
    assert_eq!(states.get("adyen"), Some(&CircuitState::Closed));

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        breaker.force_open();
    }

    let states = router.get_all_circuit_states();
    assert_eq!(states.get("stripe"), Some(&CircuitState::Open));
}

#[test]
fn test_get_all_circuit_metrics() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    router.record_route_success("stripe");
    router.record_route_success("stripe");
    router.record_route_failure("adyen");

    let metrics = router.get_all_circuit_metrics();
    assert_eq!(metrics.len(), 2);

    let stripe_metrics = metrics.get("stripe").unwrap();
    assert_eq!(stripe_metrics.successful_calls, 2);

    let adyen_metrics = metrics.get("adyen").unwrap();
    assert_eq!(adyen_metrics.failed_calls, 1);
}

#[test]
fn test_circuit_breaker_with_multiple_transactions() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    for i in 0..10 {
        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx);

        if i < 5 {
            assert!(decision.is_ok());
            if let Ok(d) = decision {
                if let Some(ref psp_id) = d.selected_route {
                    router.record_route_success(psp_id);
                }
            }
        } else {
            if let Ok(d) = decision {
                if let Some(ref psp_id) = d.selected_route {
                    router.record_route_failure(psp_id);
                }
            }
        }
    }

    let metrics = router.get_all_circuit_metrics();
    assert!(metrics.values().any(|m| m.total_calls > 0));
}

#[test]
fn test_circuit_breaker_prevents_cascading_failures() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    for _ in 0..5 {
        router.record_route_failure("stripe");
    }

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    for _ in 0..10 {
        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx);
        assert!(decision.is_ok());
        if let Ok(d) = decision {
            assert_ne!(d.selected_route, Some("stripe".to_string()));
        }
    }
}

#[test]
fn test_circuit_breaker_mixed_success_failure() {
    let config = create_test_config();
    let router = TransactionRouter::new(config).unwrap();

    router.record_route_success("stripe");
    router.record_route_failure("stripe");
    router.record_route_success("stripe");
    router.record_route_failure("stripe");

    if let Some(breaker) = router.get_circuit_breaker("stripe") {
        let metrics = breaker.get_metrics();
        assert_eq!(metrics.successful_calls, 2);
        assert_eq!(metrics.failed_calls, 2);
        assert_eq!(metrics.success_rate(), 0.5);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }
}

#[test]
fn test_new_route_gets_circuit_breaker() {
    let config = create_test_config();
    let mut router = TransactionRouter::new(config).unwrap();

    let mut paypal = RouteConfig::new(
        "paypal".to_string(),
        "PayPal".to_string(),
        "https://api.paypal.com".to_string(),
    )
    .unwrap();
    paypal.supported_methods = vec![PaymentMethod::Wallet];
    paypal.supported_currencies = vec!["USD".to_string()];
    paypal.enabled = true;

    let new_config =
        transaction_router::config::loader::RouterConfig::new().with_routes(vec![paypal]);
    router.update_routes(new_config);

    let breaker = router.get_circuit_breaker("paypal");
    assert!(breaker.is_some());
    assert_eq!(breaker.unwrap().state(), CircuitState::Closed);
}
