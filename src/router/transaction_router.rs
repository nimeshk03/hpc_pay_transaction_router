use crate::config::loader::RouterConfig;
use crate::errors::ConfigError;
use crate::models::{RouteConfig, RoutingDecision, TransactionRequest};
use crate::router::{RouteFilter, RouteScore, RouteScorer, RouteSelector, ScoringWeights, SelectionStrategy};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct MerchantPreferences {
    pub preferred_psps: Vec<String>,
    pub blocked_psps: Vec<String>,
    pub custom_weights: Option<ScoringWeights>,
}

#[derive(Debug, Clone, Default)]
pub struct RoutingMetrics {
    pub total_requests: u64,
    pub successful_routes: u64,
    pub failed_routes: u64,
    pub total_latency_us: u64,
    pub routes_selected: HashMap<String, u64>,
}

impl RoutingMetrics {
    pub fn average_latency_us(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_latency_us as f64 / self.total_requests as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_routes as f64 / self.total_requests as f64
        }
    }
}

pub struct TransactionRouter {
    routes: Vec<RouteConfig>,
    scorer: RouteScorer,
    selector: RouteSelector,
    merchant_preferences: Arc<Mutex<HashMap<String, MerchantPreferences>>>,
    metrics: Arc<Mutex<RoutingMetrics>>,
    audit_log: Arc<Mutex<Vec<RoutingDecision>>>,
}

impl TransactionRouter {
    pub fn new(config: RouterConfig) -> Result<Self, ConfigError> {
        Ok(Self {
            routes: config.routes,
            scorer: RouteScorer::with_default_weights(),
            selector: RouteSelector::with_highest_score(),
            merchant_preferences: Arc::new(Mutex::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(RoutingMetrics::default())),
            audit_log: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn with_scoring_weights(mut self, weights: ScoringWeights) -> Self {
        self.scorer = RouteScorer::new(weights);
        self
    }

    pub fn with_selection_strategy(mut self, strategy: SelectionStrategy) -> Self {
        self.selector = RouteSelector::new(strategy);
        self
    }

    pub fn set_merchant_preferences(&self, merchant_id: String, preferences: MerchantPreferences) {
        let mut prefs = self.merchant_preferences.lock().unwrap();
        prefs.insert(merchant_id, preferences);
    }

    pub fn route(&self, transaction: &TransactionRequest) -> Result<RoutingDecision, ConfigError> {
        let start = Instant::now();

        let mut eligible = RouteFilter::filter_eligible(&self.routes, transaction);

        if eligible.is_empty() {
            return self.create_failed_decision(
                transaction,
                "No eligible routes found".to_string(),
                start,
            );
        }

        eligible = self.apply_merchant_preferences(&transaction.merchant_id, eligible);

        if eligible.is_empty() {
            return self.create_failed_decision(
                transaction,
                "No routes after applying merchant preferences".to_string(),
                start,
            );
        }

        let weights = self.get_merchant_weights(&transaction.merchant_id);
        let scorer = if let Some(w) = weights {
            RouteScorer::new(w)
        } else {
            self.scorer.clone()
        };

        let scores = scorer.score_routes(&eligible, transaction);

        let (primary, fallbacks) = self.selector.select_with_fallbacks(&eligible, &scores, 2);

        let decision = self.create_decision(transaction, primary, &fallbacks, &scores, start);

        self.record_decision(&decision);
        self.update_metrics(&decision, start.elapsed().as_micros() as u64);

        Ok(decision)
    }

    fn apply_merchant_preferences(
        &self,
        merchant_id: &str,
        mut routes: Vec<RouteConfig>,
    ) -> Vec<RouteConfig> {
        let prefs = self.merchant_preferences.lock().unwrap();
        
        if let Some(preferences) = prefs.get(merchant_id) {
            routes.retain(|route| !preferences.blocked_psps.contains(&route.psp_id));

            if !preferences.preferred_psps.is_empty() {
                routes.sort_by(|a, b| {
                    let a_preferred = preferences.preferred_psps.contains(&a.psp_id);
                    let b_preferred = preferences.preferred_psps.contains(&b.psp_id);
                    b_preferred.cmp(&a_preferred)
                });
            }
        }

        routes
    }

    fn get_merchant_weights(&self, merchant_id: &str) -> Option<ScoringWeights> {
        let prefs = self.merchant_preferences.lock().unwrap();
        prefs.get(merchant_id).and_then(|p| p.custom_weights.clone())
    }

    fn create_decision(
        &self,
        transaction: &TransactionRequest,
        primary: Option<&RouteConfig>,
        fallbacks: &[&RouteConfig],
        scores: &[RouteScore],
        start: Instant,
    ) -> RoutingDecision {
        let decision_time_us = start.elapsed().as_micros() as u64;

        let selected_route = primary.map(|r| r.psp_id.clone());
        let fallback_routes: Vec<String> = fallbacks.iter().map(|r| r.psp_id.clone()).collect();

        let scores_map: HashMap<String, f64> = scores
            .iter()
            .map(|s| (s.route_id.clone(), s.total_score))
            .collect();

        let reason = if let Some(route) = primary {
            format!("Selected {} based on scoring", route.name)
        } else {
            "No route selected".to_string()
        };

        RoutingDecision::new(transaction.id)
            .with_selected_route(selected_route.unwrap_or_default())
            .with_decision_time(decision_time_us)
            .with_scores(scores_map)
            .with_reason(reason)
            .add_fallback_route(fallback_routes.first().cloned().unwrap_or_default())
    }

    fn create_failed_decision(
        &self,
        transaction: &TransactionRequest,
        reason: String,
        start: Instant,
    ) -> Result<RoutingDecision, ConfigError> {
        let decision_time_us = start.elapsed().as_micros() as u64;

        let decision = RoutingDecision::new(transaction.id)
            .with_decision_time(decision_time_us)
            .with_reason(reason.clone());

        self.record_decision(&decision);
        
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_requests += 1;
        metrics.failed_routes += 1;

        Err(ConfigError::ValidationError(reason))
    }

    fn record_decision(&self, decision: &RoutingDecision) {
        let mut audit_log = self.audit_log.lock().unwrap();
        audit_log.push(decision.clone());
    }

    fn update_metrics(&self, decision: &RoutingDecision, latency_us: u64) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_requests += 1;
        
        if decision.is_successful() {
            metrics.successful_routes += 1;
            
            if let Some(ref route_id) = decision.selected_route {
                *metrics.routes_selected.entry(route_id.clone()).or_insert(0) += 1;
            }
        } else {
            metrics.failed_routes += 1;
        }
        
        metrics.total_latency_us += latency_us;
    }

    pub fn get_metrics(&self) -> RoutingMetrics {
        self.metrics.lock().unwrap().clone()
    }

    pub fn get_audit_log(&self) -> Vec<RoutingDecision> {
        self.audit_log.lock().unwrap().clone()
    }

    pub fn clear_audit_log(&self) {
        self.audit_log.lock().unwrap().clear();
    }

    pub fn get_routes(&self) -> &[RouteConfig] {
        &self.routes
    }

    pub fn update_routes(&mut self, config: RouterConfig) {
        self.routes = config.routes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PaymentMethod;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_test_config() -> RouterConfig {
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

        RouterConfig::new().with_routes(vec![stripe, adyen])
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
    fn test_router_creation() {
        let config = create_test_config();
        let router = TransactionRouter::new(config);
        assert!(router.is_ok());
    }

    #[test]
    fn test_successful_routing() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();
        let tx = create_test_transaction("100.00");

        let decision = router.route(&tx);
        assert!(decision.is_ok());
        
        let decision = decision.unwrap();
        assert!(decision.is_successful());
        assert!(decision.selected_route.is_some());
        assert!(decision.decision_time_us > 0);
    }

    #[test]
    fn test_no_eligible_routes() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();
        
        let tx = TransactionRequest::new(
            "merchant_123".to_string(),
            Decimal::from_str("100.00").unwrap(),
            "EUR".to_string(),
            PaymentMethod::Card,
        )
        .unwrap();

        let result = router.route(&tx);
        assert!(result.is_err());
    }

    #[test]
    fn test_merchant_preferences_blocking() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let mut prefs = MerchantPreferences::default();
        prefs.blocked_psps.push("stripe".to_string());
        router.set_merchant_preferences("merchant_123".to_string(), prefs);

        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx).unwrap();

        assert_eq!(decision.selected_route, Some("adyen".to_string()));
    }

    #[test]
    fn test_merchant_preferences_preferred() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let mut prefs = MerchantPreferences::default();
        prefs.preferred_psps.push("adyen".to_string());
        router.set_merchant_preferences("merchant_123".to_string(), prefs);

        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx).unwrap();

        assert!(decision.is_successful());
    }

    #[test]
    fn test_metrics_collection() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let tx1 = create_test_transaction("100.00");
        let tx2 = create_test_transaction("200.00");

        let _ = router.route(&tx1);
        let _ = router.route(&tx2);

        let metrics = router.get_metrics();
        assert_eq!(metrics.total_requests, 2);
        assert!(metrics.successful_routes > 0);
        assert!(metrics.average_latency_us() > 0.0);
    }

    #[test]
    fn test_audit_log() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let tx = create_test_transaction("100.00");
        let _ = router.route(&tx);

        let audit_log = router.get_audit_log();
        assert_eq!(audit_log.len(), 1);
        assert_eq!(audit_log[0].request_id, tx.id);
    }

    #[test]
    fn test_clear_audit_log() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let tx = create_test_transaction("100.00");
        let _ = router.route(&tx);

        router.clear_audit_log();
        let audit_log = router.get_audit_log();
        assert_eq!(audit_log.len(), 0);
    }

    #[test]
    fn test_custom_scoring_weights() {
        let config = create_test_config();
        let weights = ScoringWeights::new(0.1, 0.7, 0.1, 0.1);
        let router = TransactionRouter::new(config).unwrap().with_scoring_weights(weights);

        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx);
        assert!(decision.is_ok());
    }

    #[test]
    fn test_weighted_random_strategy() {
        let config = create_test_config();
        let router = TransactionRouter::new(config)
            .unwrap()
            .with_selection_strategy(SelectionStrategy::WeightedRandom);

        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx);
        assert!(decision.is_ok());
    }

    #[test]
    fn test_fallback_routes() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let tx = create_test_transaction("100.00");
        let decision = router.route(&tx).unwrap();

        assert!(!decision.fallback_routes.is_empty());
    }

    #[test]
    fn test_update_routes() {
        let config = create_test_config();
        let mut router = TransactionRouter::new(config).unwrap();

        assert_eq!(router.get_routes().len(), 2);

        let new_config = RouterConfig::new().with_routes(vec![]);
        router.update_routes(new_config);

        assert_eq!(router.get_routes().len(), 0);
    }

    #[test]
    fn test_metrics_success_rate() {
        let config = create_test_config();
        let router = TransactionRouter::new(config).unwrap();

        let tx = create_test_transaction("100.00");
        let _ = router.route(&tx);

        let metrics = router.get_metrics();
        assert!(metrics.success_rate() > 0.0);
        assert!(metrics.success_rate() <= 1.0);
    }
}
