use crate::models::{RouteConfig, TransactionRequest};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ScoringWeights {
    pub latency_weight: f64,
    pub cost_weight: f64,
    pub success_rate_weight: f64,
    pub availability_weight: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            latency_weight: 0.3,
            cost_weight: 0.3,
            success_rate_weight: 0.25,
            availability_weight: 0.15,
        }
    }
}

impl ScoringWeights {
    pub fn new(latency: f64, cost: f64, success_rate: f64, availability: f64) -> Self {
        let total = latency + cost + success_rate + availability;
        Self {
            latency_weight: latency / total,
            cost_weight: cost / total,
            success_rate_weight: success_rate / total,
            availability_weight: availability / total,
        }
    }

    pub fn validate(&self) -> bool {
        let sum = self.latency_weight
            + self.cost_weight
            + self.success_rate_weight
            + self.availability_weight;
        (sum - 1.0).abs() < 0.001
    }
}

#[derive(Debug, Clone)]
pub struct RouteScore {
    pub route_id: String,
    pub total_score: f64,
    pub latency_score: f64,
    pub cost_score: f64,
    pub success_rate_score: f64,
    pub availability_score: f64,
}

#[derive(Clone)]
pub struct RouteScorer {
    weights: ScoringWeights,
}

impl RouteScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    pub fn with_default_weights() -> Self {
        Self {
            weights: ScoringWeights::default(),
        }
    }

    pub fn score_routes(
        &self,
        routes: &[RouteConfig],
        transaction: &TransactionRequest,
    ) -> Vec<RouteScore> {
        if routes.is_empty() {
            return Vec::new();
        }

        let costs = self.calculate_costs(routes, transaction);
        let max_cost = costs.values().cloned().fold(0.0, f64::max);

        routes
            .iter()
            .map(|route| self.score_route(route, transaction, &costs, max_cost))
            .collect()
    }

    fn score_route(
        &self,
        route: &RouteConfig,
        _transaction: &TransactionRequest,
        costs: &HashMap<String, f64>,
        max_cost: f64,
    ) -> RouteScore {
        let latency_score = self.calculate_latency_score(route);
        let cost_score = self.calculate_cost_score(route, costs, max_cost);
        let success_rate_score = self.calculate_success_rate_score(route);
        let availability_score = self.calculate_availability_score(route);

        let total_score = (latency_score * self.weights.latency_weight)
            + (cost_score * self.weights.cost_weight)
            + (success_rate_score * self.weights.success_rate_weight)
            + (availability_score * self.weights.availability_weight);

        RouteScore {
            route_id: route.psp_id.clone(),
            total_score,
            latency_score,
            cost_score,
            success_rate_score,
            availability_score,
        }
    }

    fn calculate_costs(
        &self,
        routes: &[RouteConfig],
        transaction: &TransactionRequest,
    ) -> HashMap<String, f64> {
        routes
            .iter()
            .map(|route| {
                let cost = self.calculate_transaction_cost(route, transaction);
                (route.psp_id.clone(), cost)
            })
            .collect()
    }

    fn calculate_transaction_cost(
        &self,
        route: &RouteConfig,
        transaction: &TransactionRequest,
    ) -> f64 {
        let fixed_fee = route.cost_structure.fixed_fee.to_string().parse::<f64>().unwrap_or(0.0);
        let percentage_fee = route.cost_structure.percentage_fee.to_string().parse::<f64>().unwrap_or(0.0);
        let amount = transaction.amount.to_string().parse::<f64>().unwrap_or(0.0);

        fixed_fee + (amount * percentage_fee / 100.0)
    }

    fn calculate_latency_score(&self, route: &RouteConfig) -> f64 {
        let base_score = 1.0 - (route.priority as f64 * 0.05);
        base_score.clamp(0.5, 1.0)
    }

    fn calculate_cost_score(
        &self,
        route: &RouteConfig,
        costs: &HashMap<String, f64>,
        max_cost: f64,
    ) -> f64 {
        if max_cost == 0.0 {
            return 1.0;
        }

        let cost = costs.get(&route.psp_id).copied().unwrap_or(0.0);
        1.0 - (cost / max_cost)
    }

    fn calculate_success_rate_score(&self, _route: &RouteConfig) -> f64 {
        0.95
    }

    fn calculate_availability_score(&self, route: &RouteConfig) -> f64 {
        if route.enabled {
            1.0
        } else {
            0.0
        }
    }

    pub fn sort_by_score(scores: &mut [RouteScore]) {
        scores.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn get_top_routes(scores: &[RouteScore], count: usize) -> Vec<RouteScore> {
        scores.iter().take(count).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PaymentMethod;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_test_route(psp_id: &str, priority: i32) -> RouteConfig {
        let mut route = RouteConfig::new(
            psp_id.to_string(),
            format!("{} PSP", psp_id),
            format!("https://api.{}.com", psp_id),
        )
        .unwrap();

        route.supported_methods = vec![PaymentMethod::Card];
        route.supported_currencies = vec!["USD".to_string()];
        route.priority = priority;
        route.cost_structure.fixed_fee = Decimal::from_str("0.30").unwrap();
        route.cost_structure.percentage_fee = Decimal::from_str("2.9").unwrap();
        route.enabled = true;

        route
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
    fn test_default_weights() {
        let weights = ScoringWeights::default();
        assert!(weights.validate());
        assert_eq!(weights.latency_weight, 0.3);
        assert_eq!(weights.cost_weight, 0.3);
    }

    #[test]
    fn test_custom_weights() {
        let weights = ScoringWeights::new(1.0, 1.0, 1.0, 1.0);
        assert!(weights.validate());
        assert_eq!(weights.latency_weight, 0.25);
    }

    #[test]
    fn test_score_single_route() {
        let scorer = RouteScorer::with_default_weights();
        let routes = vec![create_test_route("stripe", 1)];
        let tx = create_test_transaction("100.00");

        let scores = scorer.score_routes(&routes, &tx);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].route_id, "stripe");
        assert!(scores[0].total_score > 0.0);
        assert!(scores[0].total_score <= 1.0);
    }

    #[test]
    fn test_score_multiple_routes() {
        let scorer = RouteScorer::with_default_weights();
        let routes = vec![
            create_test_route("stripe", 1),
            create_test_route("adyen", 2),
            create_test_route("paypal", 3),
        ];
        let tx = create_test_transaction("100.00");

        let scores = scorer.score_routes(&routes, &tx);
        assert_eq!(scores.len(), 3);

        for score in &scores {
            assert!(score.total_score > 0.0);
            assert!(score.total_score <= 1.0);
        }
    }

    #[test]
    fn test_cost_calculation() {
        let scorer = RouteScorer::with_default_weights();
        let route = create_test_route("stripe", 1);
        let tx = create_test_transaction("100.00");

        let cost = scorer.calculate_transaction_cost(&route, &tx);
        assert!((cost - 3.20).abs() < 0.01);
    }

    #[test]
    fn test_sort_by_score() {
        let mut scores = vec![
            RouteScore {
                route_id: "low".to_string(),
                total_score: 0.5,
                latency_score: 0.5,
                cost_score: 0.5,
                success_rate_score: 0.5,
                availability_score: 0.5,
            },
            RouteScore {
                route_id: "high".to_string(),
                total_score: 0.9,
                latency_score: 0.9,
                cost_score: 0.9,
                success_rate_score: 0.9,
                availability_score: 0.9,
            },
            RouteScore {
                route_id: "medium".to_string(),
                total_score: 0.7,
                latency_score: 0.7,
                cost_score: 0.7,
                success_rate_score: 0.7,
                availability_score: 0.7,
            },
        ];

        RouteScorer::sort_by_score(&mut scores);
        assert_eq!(scores[0].route_id, "high");
        assert_eq!(scores[1].route_id, "medium");
        assert_eq!(scores[2].route_id, "low");
    }

    #[test]
    fn test_get_top_routes() {
        let scores = vec![
            RouteScore {
                route_id: "first".to_string(),
                total_score: 0.9,
                latency_score: 0.9,
                cost_score: 0.9,
                success_rate_score: 0.9,
                availability_score: 0.9,
            },
            RouteScore {
                route_id: "second".to_string(),
                total_score: 0.8,
                latency_score: 0.8,
                cost_score: 0.8,
                success_rate_score: 0.8,
                availability_score: 0.8,
            },
            RouteScore {
                route_id: "third".to_string(),
                total_score: 0.7,
                latency_score: 0.7,
                cost_score: 0.7,
                success_rate_score: 0.7,
                availability_score: 0.7,
            },
        ];

        let top = RouteScorer::get_top_routes(&scores, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].route_id, "first");
        assert_eq!(top[1].route_id, "second");
    }

    #[test]
    fn test_empty_routes() {
        let scorer = RouteScorer::with_default_weights();
        let routes = vec![];
        let tx = create_test_transaction("100.00");

        let scores = scorer.score_routes(&routes, &tx);
        assert_eq!(scores.len(), 0);
    }

    #[test]
    fn test_disabled_route_score() {
        let scorer = RouteScorer::with_default_weights();
        let mut route = create_test_route("stripe", 1);
        route.enabled = false;

        let routes = vec![route];
        let tx = create_test_transaction("100.00");

        let scores = scorer.score_routes(&routes, &tx);
        assert_eq!(scores[0].availability_score, 0.0);
    }
}
