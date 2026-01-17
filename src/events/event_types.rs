use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    RoutingDecision,
    CircuitBreakerStateChange,
    ConfigurationUpdate,
    HealthStatusChange,
    MetricsSnapshot,
}

impl EventType {
    pub fn topic_name(&self) -> &'static str {
        match self {
            EventType::RoutingDecision => "transaction.routed",
            EventType::CircuitBreakerStateChange => "circuit.state.changed",
            EventType::ConfigurationUpdate => "config.updated",
            EventType::HealthStatusChange => "health.status.changed",
            EventType::MetricsSnapshot => "metrics.snapshot",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub timestamp: i64,
    pub source: String,
    pub payload: String,
    pub metadata: HashMap<String, String>,
}

impl Event {
    pub fn new(event_type: EventType, payload: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: "transaction_router".to_string(),
            payload,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_string();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEvent {
    pub request_id: String,
    pub transaction_id: String,
    pub selected_psp: String,
    pub fallback_psps: Vec<String>,
    pub routing_time_ms: u64,
    pub score: f64,
    pub reason: String,
}

impl RoutingEvent {
    pub fn new(
        request_id: String,
        transaction_id: String,
        selected_psp: String,
        routing_time_ms: u64,
    ) -> Self {
        Self {
            request_id,
            transaction_id,
            selected_psp,
            fallback_psps: Vec::new(),
            routing_time_ms,
            score: 0.0,
            reason: String::new(),
        }
    }

    pub fn with_fallbacks(mut self, fallbacks: Vec<String>) -> Self {
        self.fallback_psps = fallbacks;
        self
    }

    pub fn with_score(mut self, score: f64, reason: &str) -> Self {
        self.score = score;
        self.reason = reason.to_string();
        self
    }

    pub fn to_event(&self) -> Event {
        let payload = serde_json::to_string(self).unwrap_or_default();
        Event::new(EventType::RoutingDecision, payload)
            .with_metadata("psp_id", &self.selected_psp)
            .with_metadata("request_id", &self.request_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerEvent {
    pub psp_id: String,
    pub previous_state: String,
    pub new_state: String,
    pub failure_count: u32,
    pub success_count: u32,
    pub trigger_reason: String,
}

impl CircuitBreakerEvent {
    pub fn new(psp_id: String, previous_state: String, new_state: String) -> Self {
        Self {
            psp_id,
            previous_state,
            new_state,
            failure_count: 0,
            success_count: 0,
            trigger_reason: String::new(),
        }
    }

    pub fn with_counts(mut self, failures: u32, successes: u32) -> Self {
        self.failure_count = failures;
        self.success_count = successes;
        self
    }

    pub fn with_reason(mut self, reason: &str) -> Self {
        self.trigger_reason = reason.to_string();
        self
    }

    pub fn to_event(&self) -> Event {
        let payload = serde_json::to_string(self).unwrap_or_default();
        Event::new(EventType::CircuitBreakerStateChange, payload)
            .with_metadata("psp_id", &self.psp_id)
            .with_metadata("new_state", &self.new_state)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEvent {
    pub version: u32,
    pub previous_version: Option<u32>,
    pub change_type: String,
    pub changed_keys: Vec<String>,
    pub source: String,
}

impl ConfigEvent {
    pub fn new(version: u32, change_type: &str) -> Self {
        Self {
            version,
            previous_version: None,
            change_type: change_type.to_string(),
            changed_keys: Vec::new(),
            source: "config_loader".to_string(),
        }
    }

    pub fn with_previous_version(mut self, version: u32) -> Self {
        self.previous_version = Some(version);
        self
    }

    pub fn with_changed_keys(mut self, keys: Vec<String>) -> Self {
        self.changed_keys = keys;
        self
    }

    pub fn to_event(&self) -> Event {
        let payload = serde_json::to_string(self).unwrap_or_default();
        Event::new(EventType::ConfigurationUpdate, payload)
            .with_metadata("version", &self.version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_topic_names() {
        assert_eq!(EventType::RoutingDecision.topic_name(), "transaction.routed");
        assert_eq!(EventType::CircuitBreakerStateChange.topic_name(), "circuit.state.changed");
        assert_eq!(EventType::ConfigurationUpdate.topic_name(), "config.updated");
    }

    #[test]
    fn test_event_creation() {
        let event = Event::new(EventType::RoutingDecision, "test payload".to_string());
        
        assert!(!event.id.is_empty());
        assert_eq!(event.event_type, EventType::RoutingDecision);
        assert_eq!(event.source, "transaction_router");
        assert_eq!(event.payload, "test payload");
    }

    #[test]
    fn test_event_with_metadata() {
        let event = Event::new(EventType::RoutingDecision, "payload".to_string())
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");
        
        assert_eq!(event.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(event.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_routing_event() {
        let routing_event = RoutingEvent::new(
            "req-123".to_string(),
            "tx-456".to_string(),
            "stripe".to_string(),
            50,
        )
        .with_fallbacks(vec!["adyen".to_string(), "checkout".to_string()])
        .with_score(0.95, "best latency");
        
        assert_eq!(routing_event.selected_psp, "stripe");
        assert_eq!(routing_event.fallback_psps.len(), 2);
        assert_eq!(routing_event.score, 0.95);
        
        let event = routing_event.to_event();
        assert_eq!(event.event_type, EventType::RoutingDecision);
    }

    #[test]
    fn test_circuit_breaker_event() {
        let cb_event = CircuitBreakerEvent::new(
            "stripe".to_string(),
            "closed".to_string(),
            "open".to_string(),
        )
        .with_counts(5, 100)
        .with_reason("failure threshold exceeded");
        
        assert_eq!(cb_event.psp_id, "stripe");
        assert_eq!(cb_event.failure_count, 5);
        
        let event = cb_event.to_event();
        assert_eq!(event.event_type, EventType::CircuitBreakerStateChange);
    }

    #[test]
    fn test_config_event() {
        let config_event = ConfigEvent::new(2, "reload")
            .with_previous_version(1)
            .with_changed_keys(vec!["routes".to_string(), "limits".to_string()]);
        
        assert_eq!(config_event.version, 2);
        assert_eq!(config_event.previous_version, Some(1));
        assert_eq!(config_event.changed_keys.len(), 2);
        
        let event = config_event.to_event();
        assert_eq!(event.event_type, EventType::ConfigurationUpdate);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(EventType::RoutingDecision, "test".to_string());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.event_type, event.event_type);
    }
}
