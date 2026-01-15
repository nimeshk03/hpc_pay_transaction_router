use rust_decimal::Decimal;
use std::str::FromStr;
use transaction_router::{
    PaymentMethod, RouteConfig, RouteFilter, RouteScorer, RouteSelector, ScoringWeights,
    TransactionRequest,
};

fn create_test_routes() -> Vec<RouteConfig> {
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

    vec![stripe, adyen, paypal]
}

#[test]
fn test_end_to_end_routing_card_payment() {
    let routes = create_test_routes();
    let transaction = TransactionRequest::new(
        "merchant_123".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);
    assert_eq!(eligible.len(), 2);
    assert!(eligible.iter().any(|r| r.psp_id == "stripe"));
    assert!(eligible.iter().any(|r| r.psp_id == "adyen"));

    let scorer = RouteScorer::with_default_weights();
    let scores = scorer.score_routes(&eligible, &transaction);
    assert_eq!(scores.len(), 2);

    let selector = RouteSelector::with_highest_score();
    let selected = selector.select(&eligible, &scores);
    assert!(selected.is_some());
    assert!(selected.unwrap().psp_id == "stripe" || selected.unwrap().psp_id == "adyen");
}

#[test]
fn test_end_to_end_routing_wallet_payment() {
    let routes = create_test_routes();
    let transaction = TransactionRequest::new(
        "merchant_456".to_string(),
        Decimal::from_str("50.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Wallet,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);
    assert_eq!(eligible.len(), 2);
    assert!(eligible.iter().any(|r| r.psp_id == "stripe"));
    assert!(eligible.iter().any(|r| r.psp_id == "paypal"));

    let scorer = RouteScorer::with_default_weights();
    let scores = scorer.score_routes(&eligible, &transaction);

    let selector = RouteSelector::with_highest_score();
    let selected = selector.select(&eligible, &scores);
    assert!(selected.is_some());
}

#[test]
fn test_routing_with_fallbacks() {
    let routes = create_test_routes();
    let transaction = TransactionRequest::new(
        "merchant_789".to_string(),
        Decimal::from_str("200.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);
    let scorer = RouteScorer::with_default_weights();
    let scores = scorer.score_routes(&eligible, &transaction);

    let selector = RouteSelector::with_highest_score();
    let (primary, fallbacks) = selector.select_with_fallbacks(&eligible, &scores, 2);

    assert!(primary.is_some());
    assert!(!fallbacks.is_empty());
    assert_ne!(primary.unwrap().psp_id, fallbacks[0].psp_id);
}

#[test]
fn test_routing_amount_limits() {
    let routes = create_test_routes();
    
    let small_tx = TransactionRequest::new(
        "merchant_small".to_string(),
        Decimal::from_str("0.25").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &small_tx);
    assert_eq!(eligible.len(), 0);

    let large_tx = TransactionRequest::new(
        "merchant_large".to_string(),
        Decimal::from_str("8000.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &large_tx);
    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].psp_id, "stripe");
}

#[test]
fn test_routing_unsupported_currency() {
    let routes = create_test_routes();
    let transaction = TransactionRequest::new(
        "merchant_xyz".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "JPY".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);
    assert_eq!(eligible.len(), 0);
}

#[test]
fn test_routing_disabled_route() {
    let mut routes = create_test_routes();
    routes[0].enabled = false;

    let transaction = TransactionRequest::new(
        "merchant_disabled".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);
    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].psp_id, "adyen");
}

#[test]
fn test_weighted_random_distribution() {
    let routes = create_test_routes();
    let transaction = TransactionRequest::new(
        "merchant_random".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);
    let scorer = RouteScorer::with_default_weights();
    let scores = scorer.score_routes(&eligible, &transaction);

    let selector = RouteSelector::with_weighted_random();
    let mut selections = std::collections::HashMap::new();

    for _ in 0..100 {
        if let Some(selected) = selector.select(&eligible, &scores) {
            *selections.entry(selected.psp_id.clone()).or_insert(0) += 1;
        }
    }

    assert!(selections.len() > 0);
    assert!(selections.values().sum::<i32>() == 100);
}

#[test]
fn test_custom_scoring_weights() {
    let routes = create_test_routes();
    let transaction = TransactionRequest::new(
        "merchant_custom".to_string(),
        Decimal::from_str("100.00").unwrap(),
        "USD".to_string(),
        PaymentMethod::Card,
    )
    .unwrap();

    let eligible = RouteFilter::filter_eligible(&routes, &transaction);

    let cost_focused_weights = ScoringWeights::new(0.1, 0.7, 0.1, 0.1);
    let scorer = RouteScorer::new(cost_focused_weights);
    let scores = scorer.score_routes(&eligible, &transaction);

    assert!(scores.iter().all(|s| s.total_score > 0.0 && s.total_score <= 1.0));
}
