use crate::models::RouteConfig;
use crate::router::{RouteScore, RouteScorer};
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    HighestScore,
    WeightedRandom,
}

pub struct RouteSelector {
    strategy: SelectionStrategy,
}

impl RouteSelector {
    pub fn new(strategy: SelectionStrategy) -> Self {
        Self { strategy }
    }

    pub fn with_highest_score() -> Self {
        Self {
            strategy: SelectionStrategy::HighestScore,
        }
    }

    pub fn with_weighted_random() -> Self {
        Self {
            strategy: SelectionStrategy::WeightedRandom,
        }
    }

    pub fn select<'a>(
        &self,
        routes: &'a [RouteConfig],
        scores: &[RouteScore],
    ) -> Option<&'a RouteConfig> {
        if routes.is_empty() || scores.is_empty() {
            return None;
        }

        match self.strategy {
            SelectionStrategy::HighestScore => self.select_highest_score(routes, scores),
            SelectionStrategy::WeightedRandom => self.select_weighted_random(routes, scores),
        }
    }

    pub fn select_with_fallbacks<'a>(
        &self,
        routes: &'a [RouteConfig],
        scores: &[RouteScore],
        fallback_count: usize,
    ) -> (Option<&'a RouteConfig>, Vec<&'a RouteConfig>) {
        let primary = self.select(routes, scores);
        let fallbacks = self.get_fallback_routes(routes, scores, primary, fallback_count);

        (primary, fallbacks)
    }

    fn select_highest_score<'a>(
        &self,
        routes: &'a [RouteConfig],
        scores: &[RouteScore],
    ) -> Option<&'a RouteConfig> {
        let best_score = scores.iter().max_by(|a, b| {
            a.total_score
                .partial_cmp(&b.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;

        routes.iter().find(|r| r.psp_id == best_score.route_id)
    }

    fn select_weighted_random<'a>(
        &self,
        routes: &'a [RouteConfig],
        scores: &[RouteScore],
    ) -> Option<&'a RouteConfig> {
        let total_score: f64 = scores.iter().map(|s| s.total_score).sum();

        if total_score == 0.0 {
            return routes.first();
        }

        let mut rng = rand::thread_rng();
        let mut random_value = rng.gen::<f64>() * total_score;

        for score in scores {
            random_value -= score.total_score;
            if random_value <= 0.0 {
                return routes.iter().find(|r| r.psp_id == score.route_id);
            }
        }

        routes.last()
    }

    fn get_fallback_routes<'a>(
        &self,
        routes: &'a [RouteConfig],
        scores: &[RouteScore],
        primary: Option<&'a RouteConfig>,
        count: usize,
    ) -> Vec<&'a RouteConfig> {
        let mut sorted_scores = scores.to_vec();
        RouteScorer::sort_by_score(&mut sorted_scores);

        let primary_id = primary.map(|r| &r.psp_id);

        sorted_scores
            .iter()
            .filter(|score| Some(&score.route_id) != primary_id)
            .filter_map(|score| routes.iter().find(|r| r.psp_id == score.route_id))
            .take(count)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PaymentMethod;

    fn create_test_route(psp_id: &str) -> RouteConfig {
        let mut route = RouteConfig::new(
            psp_id.to_string(),
            format!("{} PSP", psp_id),
            format!("https://api.{}.com", psp_id),
        )
        .unwrap();

        route.supported_methods = vec![PaymentMethod::Card];
        route.supported_currencies = vec!["USD".to_string()];
        route.enabled = true;

        route
    }

    fn create_test_scores() -> Vec<RouteScore> {
        vec![
            RouteScore {
                route_id: "stripe".to_string(),
                total_score: 0.9,
                latency_score: 0.9,
                cost_score: 0.9,
                success_rate_score: 0.9,
                availability_score: 1.0,
            },
            RouteScore {
                route_id: "adyen".to_string(),
                total_score: 0.7,
                latency_score: 0.7,
                cost_score: 0.7,
                success_rate_score: 0.7,
                availability_score: 1.0,
            },
            RouteScore {
                route_id: "paypal".to_string(),
                total_score: 0.5,
                latency_score: 0.5,
                cost_score: 0.5,
                success_rate_score: 0.5,
                availability_score: 1.0,
            },
        ]
    }

    #[test]
    fn test_select_highest_score() {
        let selector = RouteSelector::with_highest_score();
        let routes = vec![
            create_test_route("stripe"),
            create_test_route("adyen"),
            create_test_route("paypal"),
        ];
        let scores = create_test_scores();

        let selected = selector.select(&routes, &scores);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().psp_id, "stripe");
    }

    #[test]
    fn test_select_weighted_random() {
        let selector = RouteSelector::with_weighted_random();
        let routes = vec![
            create_test_route("stripe"),
            create_test_route("adyen"),
            create_test_route("paypal"),
        ];
        let scores = create_test_scores();

        let mut selections = std::collections::HashMap::new();
        for _ in 0..1000 {
            let selected = selector.select(&routes, &scores);
            if let Some(route) = selected {
                *selections.entry(route.psp_id.clone()).or_insert(0) += 1;
            }
        }

        assert!(selections.contains_key("stripe"));
        assert!(selections["stripe"] > selections.get("paypal").copied().unwrap_or(0));
    }

    #[test]
    fn test_select_with_fallbacks() {
        let selector = RouteSelector::with_highest_score();
        let routes = vec![
            create_test_route("stripe"),
            create_test_route("adyen"),
            create_test_route("paypal"),
        ];
        let scores = create_test_scores();

        let (primary, fallbacks) = selector.select_with_fallbacks(&routes, &scores, 2);

        assert!(primary.is_some());
        assert_eq!(primary.unwrap().psp_id, "stripe");
        assert_eq!(fallbacks.len(), 2);
        assert_eq!(fallbacks[0].psp_id, "adyen");
        assert_eq!(fallbacks[1].psp_id, "paypal");
    }

    #[test]
    fn test_select_empty_routes() {
        let selector = RouteSelector::with_highest_score();
        let routes: Vec<RouteConfig> = vec![];
        let scores: Vec<RouteScore> = vec![];

        let selected = selector.select(&routes, &scores);
        assert!(selected.is_none());
    }

    #[test]
    fn test_select_single_route() {
        let selector = RouteSelector::with_highest_score();
        let routes = vec![create_test_route("stripe")];
        let scores = vec![RouteScore {
            route_id: "stripe".to_string(),
            total_score: 0.8,
            latency_score: 0.8,
            cost_score: 0.8,
            success_rate_score: 0.8,
            availability_score: 1.0,
        }];

        let selected = selector.select(&routes, &scores);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().psp_id, "stripe");
    }

    #[test]
    fn test_fallbacks_exclude_primary() {
        let selector = RouteSelector::with_highest_score();
        let routes = vec![
            create_test_route("stripe"),
            create_test_route("adyen"),
        ];
        let scores = vec![
            RouteScore {
                route_id: "stripe".to_string(),
                total_score: 0.9,
                latency_score: 0.9,
                cost_score: 0.9,
                success_rate_score: 0.9,
                availability_score: 1.0,
            },
            RouteScore {
                route_id: "adyen".to_string(),
                total_score: 0.7,
                latency_score: 0.7,
                cost_score: 0.7,
                success_rate_score: 0.7,
                availability_score: 1.0,
            },
        ];

        let (primary, fallbacks) = selector.select_with_fallbacks(&routes, &scores, 1);

        assert_eq!(primary.unwrap().psp_id, "stripe");
        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0].psp_id, "adyen");
    }

    #[test]
    fn test_strategy_types() {
        let highest = RouteSelector::with_highest_score();
        assert_eq!(highest.strategy, SelectionStrategy::HighestScore);

        let weighted = RouteSelector::with_weighted_random();
        assert_eq!(weighted.strategy, SelectionStrategy::WeightedRandom);
    }
}
