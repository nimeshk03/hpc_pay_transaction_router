use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub request_id: Uuid,
    pub selected_route: Option<String>,
    pub decision_time_us: u64,
    pub scores: HashMap<String, f64>,
    pub reason: String,
    pub fallback_routes: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl RoutingDecision {
    pub fn new(request_id: Uuid) -> Self {
        Self {
            request_id,
            selected_route: None,
            decision_time_us: 0,
            scores: HashMap::new(),
            reason: String::new(),
            fallback_routes: Vec::new(),
            timestamp: Utc::now(),
        }
    }

    pub fn with_selected_route(mut self, route: String) -> Self {
        self.selected_route = Some(route);
        self
    }

    pub fn with_decision_time(mut self, time_us: u64) -> Self {
        self.decision_time_us = time_us;
        self
    }

    pub fn with_scores(mut self, scores: HashMap<String, f64>) -> Self {
        self.scores = scores;
        self
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = reason;
        self
    }

    pub fn add_fallback_route(mut self, route: String) -> Self {
        self.fallback_routes.push(route);
        self
    }

    pub fn is_successful(&self) -> bool {
        self.selected_route.is_some()
    }

    pub fn meets_latency_target(&self, target_us: u64) -> bool {
        self.decision_time_us <= target_us
    }
}
