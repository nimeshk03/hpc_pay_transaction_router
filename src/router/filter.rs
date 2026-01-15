use crate::models::{PaymentMethod, RouteConfig, TransactionRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct RouteFilter;

impl RouteFilter {
    pub fn filter_eligible(
        routes: &[RouteConfig],
        transaction: &TransactionRequest,
    ) -> Vec<RouteConfig> {
        routes
            .iter()
            .filter(|route| Self::is_eligible(route, transaction))
            .cloned()
            .collect()
    }

    pub fn is_eligible(route: &RouteConfig, transaction: &TransactionRequest) -> bool {
        Self::is_enabled(route)
            && Self::supports_payment_method(route, &transaction.payment_method)
            && Self::supports_currency(route, &transaction.currency)
            && Self::within_amount_limits(route, transaction)
    }

    pub fn by_payment_method(
        routes: &[RouteConfig],
        method: &PaymentMethod,
    ) -> Vec<RouteConfig> {
        routes
            .iter()
            .filter(|route| Self::supports_payment_method(route, method))
            .cloned()
            .collect()
    }

    pub fn by_currency(routes: &[RouteConfig], currency: &str) -> Vec<RouteConfig> {
        routes
            .iter()
            .filter(|route| Self::supports_currency(route, currency))
            .cloned()
            .collect()
    }

    pub fn by_enabled(routes: &[RouteConfig]) -> Vec<RouteConfig> {
        routes
            .iter()
            .filter(|route| Self::is_enabled(route))
            .cloned()
            .collect()
    }

    fn is_enabled(route: &RouteConfig) -> bool {
        route.enabled
    }

    fn supports_payment_method(route: &RouteConfig, method: &PaymentMethod) -> bool {
        route.supported_methods.contains(method)
    }

    fn supports_currency(route: &RouteConfig, currency: &str) -> bool {
        route.supports_currency(currency)
    }

    fn within_amount_limits(route: &RouteConfig, transaction: &TransactionRequest) -> bool {
        transaction.amount >= route.limits.min_amount
            && transaction.amount <= route.limits.max_amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_test_route(psp_id: &str, methods: Vec<PaymentMethod>) -> RouteConfig {
        let mut route = RouteConfig::new(
            psp_id.to_string(),
            format!("{} PSP", psp_id),
            format!("https://api.{}.com", psp_id),
        )
        .unwrap();

        route.supported_methods = methods;
        route.supported_currencies = vec!["USD".to_string(), "EUR".to_string()];
        route.limits.min_amount = Decimal::from_str("1.00").unwrap();
        route.limits.max_amount = Decimal::from_str("10000.00").unwrap();
        route.enabled = true;

        route
    }

    fn create_test_transaction(
        amount: &str,
        currency: &str,
        method: PaymentMethod,
    ) -> TransactionRequest {
        TransactionRequest::new(
            "merchant_123".to_string(),
            Decimal::from_str(amount).unwrap(),
            currency.to_string(),
            method,
        )
        .unwrap()
    }

    #[test]
    fn test_filter_by_payment_method() {
        let routes = vec![
            create_test_route("stripe", vec![PaymentMethod::Card, PaymentMethod::Wallet]),
            create_test_route("paypal", vec![PaymentMethod::Wallet]),
            create_test_route("adyen", vec![PaymentMethod::Card, PaymentMethod::BankTransfer]),
        ];

        let filtered = RouteFilter::by_payment_method(&routes, &PaymentMethod::Card);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|r| r.psp_id == "stripe"));
        assert!(filtered.iter().any(|r| r.psp_id == "adyen"));

        let wallet_filtered = RouteFilter::by_payment_method(&routes, &PaymentMethod::Wallet);
        assert_eq!(wallet_filtered.len(), 2);
        assert!(wallet_filtered.iter().any(|r| r.psp_id == "stripe"));
        assert!(wallet_filtered.iter().any(|r| r.psp_id == "paypal"));
    }

    #[test]
    fn test_filter_by_currency() {
        let routes = vec![
            create_test_route("stripe", vec![PaymentMethod::Card]),
            create_test_route("paypal", vec![PaymentMethod::Wallet]),
        ];

        let filtered = RouteFilter::by_currency(&routes, "USD");
        assert_eq!(filtered.len(), 2);

        let filtered_eur = RouteFilter::by_currency(&routes, "EUR");
        assert_eq!(filtered_eur.len(), 2);

        let filtered_gbp = RouteFilter::by_currency(&routes, "GBP");
        assert_eq!(filtered_gbp.len(), 0);
    }

    #[test]
    fn test_filter_by_enabled() {
        let mut routes = vec![
            create_test_route("stripe", vec![PaymentMethod::Card]),
            create_test_route("paypal", vec![PaymentMethod::Wallet]),
        ];

        routes[1].enabled = false;

        let filtered = RouteFilter::by_enabled(&routes);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].psp_id, "stripe");
    }

    #[test]
    fn test_filter_eligible_routes() {
        let routes = vec![
            create_test_route("stripe", vec![PaymentMethod::Card, PaymentMethod::Wallet]),
            create_test_route("paypal", vec![PaymentMethod::Wallet]),
            create_test_route("adyen", vec![PaymentMethod::Card]),
        ];

        let tx = create_test_transaction("100.00", "USD", PaymentMethod::Card);
        let eligible = RouteFilter::filter_eligible(&routes, &tx);

        assert_eq!(eligible.len(), 2);
        assert!(eligible.iter().any(|r| r.psp_id == "stripe"));
        assert!(eligible.iter().any(|r| r.psp_id == "adyen"));
    }

    #[test]
    fn test_filter_amount_limits() {
        let mut routes = vec![create_test_route("stripe", vec![PaymentMethod::Card])];
        routes[0].limits.min_amount = Decimal::from_str("10.00").unwrap();
        routes[0].limits.max_amount = Decimal::from_str("1000.00").unwrap();

        let tx_too_low = create_test_transaction("5.00", "USD", PaymentMethod::Card);
        let eligible = RouteFilter::filter_eligible(&routes, &tx_too_low);
        assert_eq!(eligible.len(), 0);

        let tx_too_high = create_test_transaction("2000.00", "USD", PaymentMethod::Card);
        let eligible = RouteFilter::filter_eligible(&routes, &tx_too_high);
        assert_eq!(eligible.len(), 0);

        let tx_valid = create_test_transaction("500.00", "USD", PaymentMethod::Card);
        let eligible = RouteFilter::filter_eligible(&routes, &tx_valid);
        assert_eq!(eligible.len(), 1);
    }

    #[test]
    fn test_no_eligible_routes() {
        let routes = vec![create_test_route("stripe", vec![PaymentMethod::Card])];

        let tx = create_test_transaction("100.00", "USD", PaymentMethod::UPI);
        let eligible = RouteFilter::filter_eligible(&routes, &tx);
        assert_eq!(eligible.len(), 0);
    }

    #[test]
    fn test_disabled_route_filtered_out() {
        let mut routes = vec![create_test_route("stripe", vec![PaymentMethod::Card])];
        routes[0].enabled = false;

        let tx = create_test_transaction("100.00", "USD", PaymentMethod::Card);
        let eligible = RouteFilter::filter_eligible(&routes, &tx);
        assert_eq!(eligible.len(), 0);
    }
}
