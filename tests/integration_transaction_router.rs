use rust_decimal::Decimal;
use std::str::FromStr;
use transaction_router::{
    MerchantPreferences, PaymentMethod, RouteConfig, ScoringWeights,
    SelectionStrategy, TransactionRequest, TransactionRouter,
};

fn create_comprehensive_config() -> transaction_router::config::loader::RouterConfig {
    let mut stripe = RouteConfig::new(
        "stripe".to_string(),
        "Stripe".to_string(),
        "https://api.stripe.com".to_string(),
    )
    .unwrap();
    stripe.supported_methods = vec![PaymentMethod::Card, PaymentMethod::Wallet];
    stripe.supported_currencies = vec!["USD".to_string(), "EUR".to_string()];
    stripe.cost_structure.fixed_fee = Decimal::from_str("0.30").unwrap();
    stripe.cost_structure.percentage_fee = Decimal::from_str("2.9").unwrap();
    stripe.limits.min_amount = Decimal::from_str("0.50").unwrap();
    stripe.limits.max_amount = Decimal::from_str("10000.00").unwrap();
    stripe.priority = 1;
    stripe.enabled = true;

    let mut adyen = RouteConfig::new(
        "adyen".to_string(),
        "Adyen".to_string(),
        "https://api.adyen.com".to_string(),
    )
    .unwrap();
    adyen.supported_methods = vec![PaymentMethod::Card, PaymentMethod::BankTransfer];
    adyen.supported_currencies = vec!["USD".to_string(), "EUR".to_string(), "GBP".to_string()];
    adyen.cost_structure.fixed_fee = Decimal::from_str("0.10").unwrap();
    adyen.cost_structure.percentage_fee = Decimal::from_str("2.5").unwrap();
    adyen.limits.min_amount = Decimal::from_str("1.00").unwrap();
    adyen.limits.max_amount = Decimal::from_str("5000.00").unwrap();
    adyen.priority = 2;
    adyen.enabled = true;

    let mut paypal = RouteConfig::new(
        "paypal".to_string(),
        "PayPal".to_string(),
        "https://api.paypal.com".to_string(),
    )
    .unwrap();
    paypal.supported_methods = vec![PaymentMethod::Wallet];
    paypal.supported_currencies = vec!["USD".to_string()];
    paypal.cost_structure.fixed_fee = Decimal::from_str("0.49").unwrap();
    paypal.cost_structure.percentage_fee = Decimal::from_str("3.49").unwrap();
    paypal.limits.min_amount = Decimal::from_str("0.01").unwrap();
    paypal.limits.max_amount = Decimal::from_str("1000.00").unwrap();
    paypal.priority = 3;
    paypal.enabled = true;

    transaction_router::config::loader::RouterConfig::new()
        .with_routes(vec![stripe, adyen, paypal])
}

#[test]
fn test_end_to_end_transaction_routing() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let transaction = TransactionRequest::new(
        "merchant_123".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let decision = router.route(&transaction).unwrap();

    assert!(decision.is_successful());
    assert!(decision.selected_route.is_some());
    assert!(!decision.fallback_routes.is_empty());
    assert!(decision.decision_time_us > 0);
    assert!(!decision.scores.is_empty());
}

#[test]
fn test_merchant_preferences_integration() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let mut preferences = MerchantPreferences::default();
    preferences.preferred_psps.push("adyen".to_string());
    preferences.blocked_psps.push("paypal".to_string());

    router.set_merchant_preferences("merchant_vip".to_string(), preferences);

    let transaction = TransactionRequest::new(
        "merchant_vip".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let decision = router.route(&transaction).unwrap();
    assert!(decision.is_successful());
    assert_ne!(decision.selected_route, Some("paypal".to_string()));
}

#[test]
fn test_custom_scoring_weights_integration() {
    let config = create_comprehensive_config();
    let cost_focused_weights = ScoringWeights::new(0.1, 0.7, 0.1, 0.1);
    let router = TransactionRouter::new(config)
        .unwrap()
        .with_scoring_weights(cost_focused_weights);

    let transaction = TransactionRequest::new(
        "merchant_cost_conscious".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let decision = router.route(&transaction).unwrap();
    assert!(decision.is_successful());
}

#[test]
fn test_weighted_random_strategy_integration() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config)
        .unwrap()
        .with_selection_strategy(SelectionStrategy::WeightedRandom);

    let mut selections = std::collections::HashMap::new();

    for _ in 0..50 {
        let transaction = TransactionRequest::new(
            "merchant_random".to_string(),
            Decimal::from_str("100.00").unwrap(),
            "USD".to_string(),
            PaymentMethod::Card,
        )
        .unwrap();

        if let Ok(decision) = router.route(&transaction) {
            if let Some(ref route_id) = decision.selected_route {
                *selections.entry(route_id.clone()).or_insert(0) += 1;
            }
        }
    }

    assert!(selections.len() > 0);
}

#[test]
fn test_metrics_collection_integration() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    for i in 0..10 {
        let transaction = TransactionRequest::new(
            format!("merchant_{}", i),
            Decimal::from_str("100.00").unwrap(),
            "USD".to_string(),
            PaymentMethod::Card,
        )
        .unwrap();

        let _ = router.route(&transaction);
    }

    let metrics = router.get_metrics();
    assert_eq!(metrics.total_requests, 10);
    assert!(metrics.successful_routes > 0);
    assert!(metrics.average_latency_us() > 0.0);
    assert!(metrics.success_rate() > 0.0);
}

#[test]
fn test_audit_log_integration() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let tx1 = TransactionRequest::new(
        "merchant_audit_1".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let tx2 = TransactionRequest::new(
        "merchant_audit_2".to_string(),
        Decimal::from_str("200.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let _ = router.route(&tx1);
    let _ = router.route(&tx2);

    let audit_log = router.get_audit_log();
    assert_eq!(audit_log.len(), 2);
    assert_eq!(audit_log[0].request_id, tx1.id);
    assert_eq!(audit_log[1].request_id, tx2.id);

    router.clear_audit_log();
    let cleared_log = router.get_audit_log();
    assert_eq!(cleared_log.len(), 0);
}

#[test]
fn test_no_eligible_routes_error_handling() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let transaction = TransactionRequest::new(
        "merchant_unsupported".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "JPY".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let result = router.route(&transaction);
    assert!(result.is_err());

    let metrics = router.get_metrics();
    assert_eq!(metrics.failed_routes, 1);
}

#[test]
fn test_amount_limits_routing() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let large_transaction = TransactionRequest::new(
        "merchant_large".to_string(),
        Decimal::from_str("8000.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let decision = router.route(&large_transaction).unwrap();
    assert!(decision.is_successful());
    assert_eq!(decision.selected_route, Some("stripe".to_string()));
}

#[test]
fn test_fallback_routes_populated() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let transaction = TransactionRequest::new(
        "merchant_fallback".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let decision = router.route(&transaction).unwrap();
    assert!(!decision.fallback_routes.is_empty());
    
    if let Some(ref primary) = decision.selected_route {
        for fallback in &decision.fallback_routes {
            assert_ne!(primary, fallback);
        }
    }
}

#[test]
fn test_route_update_integration() {
    let config = create_comprehensive_config();
    let mut router = TransactionRouter::new(config).unwrap();

    assert_eq!(router.get_routes().len(), 3);

    let new_config = transaction_router::config::loader::RouterConfig::new().with_routes(vec![]);
    router.update_routes(new_config);

    assert_eq!(router.get_routes().len(), 0);

    let transaction = TransactionRequest::new(
        "merchant_no_routes".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let result = router.route(&transaction);
    assert!(result.is_err());
}

#[test]
fn test_merchant_custom_weights() {
    let config = create_comprehensive_config();
    let router = TransactionRouter::new(config).unwrap();

    let mut preferences = MerchantPreferences::default();
    preferences.custom_weights = Some(ScoringWeights::new(0.1, 0.8, 0.05, 0.05));

    router.set_merchant_preferences("merchant_custom".to_string(), preferences);

    let transaction = TransactionRequest::new(
        "merchant_custom".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let decision = router.route(&transaction).unwrap();
    assert!(decision.is_successful());
}
