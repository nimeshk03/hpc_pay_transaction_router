#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::errors::TransactionError;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_valid_transaction_creation() {
        let tx = TransactionRequest::new(
            "merchant_123".to_string(),
            Decimal::from_str("100.00").unwrap(),
            "USD".to_string(),
            PaymentMethod::Card,
        );

        assert!(tx.is_ok());
        let tx = tx.unwrap();
        assert_eq!(tx.merchant_id, "merchant_123");
        assert_eq!(tx.amount, Decimal::from_str("100.00").unwrap());
        assert_eq!(tx.currency, "USD");
        assert!(tx.is_valid());
    }

    #[test]
    fn test_invalid_amount_rejection() {
        let result = TransactionRequest::new(
            "merchant_123".to_string(),
            Decimal::from_str("-10.00").unwrap(),
            "USD".to_string(),
            PaymentMethod::Card,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TransactionError::InvalidAmount("Amount must be positive".to_string())
        );
    }

    #[test]
    fn test_invalid_currency_rejection() {
        let result = TransactionRequest::new(
            "merchant_123".to_string(),
            Decimal::from_str("100.00").unwrap(),
            "US".to_string(),
            PaymentMethod::Card,
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TransactionError::InvalidCurrency("Currency must be 3-letter ISO code".to_string())
        );
    }

    #[test]
    fn test_missing_merchant_id() {
        let result = TransactionRequest::new(
            "".to_string(),
            Decimal::from_str("100.00").unwrap(),
            "USD".to_string(),
            PaymentMethod::Card,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_builder_pattern() {
        let tx = TransactionRequest::new(
            "merchant_123".to_string(),
            Decimal::from_str("100.00").unwrap(),
            "USD".to_string(),
            PaymentMethod::Card,
        )
        .unwrap()
        .with_priority(Priority::High)
        .with_card_token("tok_123".to_string())
        .with_timeout(3000);

        assert_eq!(tx.priority, Priority::High);
        assert_eq!(tx.card_token, Some("tok_123".to_string()));
        assert_eq!(tx.timeout_ms, 3000);
    }

    #[test]
    fn test_route_config_creation() {
        let config = RouteConfig::new(
            "stripe".to_string(),
            "Stripe".to_string(),
            "https://api.stripe.com".to_string(),
        );

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.psp_id, "stripe");
        assert!(config.enabled);
    }

    #[test]
    fn test_route_config_validation() {
        let mut config = RouteConfig::new(
            "stripe".to_string(),
            "Stripe".to_string(),
            "https://api.stripe.com".to_string(),
        )
        .unwrap();

        config.limits.min_amount = Decimal::from_str("1000.00").unwrap();
        config.limits.max_amount = Decimal::from_str("100.00").unwrap();

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_route_supports_method() {
        let mut config = RouteConfig::new(
            "stripe".to_string(),
            "Stripe".to_string(),
            "https://api.stripe.com".to_string(),
        )
        .unwrap();

        config.supported_methods = vec![PaymentMethod::Card, PaymentMethod::Wallet];

        assert!(config.supports_method(&PaymentMethod::Card));
        assert!(!config.supports_method(&PaymentMethod::BankTransfer));
    }

    #[test]
    fn test_route_supports_currency() {
        let mut config = RouteConfig::new(
            "stripe".to_string(),
            "Stripe".to_string(),
            "https://api.stripe.com".to_string(),
        )
        .unwrap();

        config.supported_currencies = vec!["USD".to_string(), "EUR".to_string()];

        assert!(config.supports_currency("USD"));
        assert!(config.supports_currency("usd"));
        assert!(!config.supports_currency("GBP"));
    }

    #[test]
    fn test_routing_decision_builder() {
        let request_id = uuid::Uuid::new_v4();
        let mut scores = std::collections::HashMap::new();
        scores.insert("stripe".to_string(), 0.95);
        scores.insert("adyen".to_string(), 0.85);

        let decision = RoutingDecision::new(request_id)
            .with_selected_route("stripe".to_string())
            .with_decision_time(500)
            .with_scores(scores.clone())
            .with_reason("Best score".to_string())
            .add_fallback_route("adyen".to_string());

        assert!(decision.is_successful());
        assert_eq!(decision.selected_route, Some("stripe".to_string()));
        assert_eq!(decision.decision_time_us, 500);
        assert_eq!(decision.scores.len(), 2);
        assert_eq!(decision.fallback_routes.len(), 1);
    }

    #[test]
    fn test_decision_meets_latency_target() {
        let decision = RoutingDecision::new(uuid::Uuid::new_v4()).with_decision_time(800);

        assert!(decision.meets_latency_target(1000));
        assert!(!decision.meets_latency_target(500));
    }
}
