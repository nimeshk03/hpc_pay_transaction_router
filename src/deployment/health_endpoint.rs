use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: i64,
    pub details: HashMap<String, String>,
}

impl ComponentHealth {
    pub fn healthy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            message: None,
            last_check: chrono::Utc::now().timestamp_millis(),
            details: HashMap::new(),
        }
    }

    pub fn degraded(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Degraded,
            message: Some(message.to_string()),
            last_check: chrono::Utc::now().timestamp_millis(),
            details: HashMap::new(),
        }
    }

    pub fn unhealthy(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(message.to_string()),
            last_check: chrono::Utc::now().timestamp_millis(),
            details: HashMap::new(),
        }
    }

    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: u64,
    pub timestamp: i64,
    pub components: Vec<ComponentHealth>,
}

impl HealthResponse {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

pub struct HealthEndpoint {
    version: String,
    start_time: Instant,
    components: Arc<Mutex<HashMap<String, ComponentHealth>>>,
}

impl HealthEndpoint {
    pub fn new(version: &str) -> Self {
        Self {
            version: version.to_string(),
            start_time: Instant::now(),
            components: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_component(&self, name: &str) {
        let mut components = self.components.lock().unwrap();
        components.insert(name.to_string(), ComponentHealth::healthy(name));
    }

    pub fn update_component(&self, health: ComponentHealth) {
        let mut components = self.components.lock().unwrap();
        components.insert(health.name.clone(), health);
    }

    pub fn set_component_healthy(&self, name: &str) {
        self.update_component(ComponentHealth::healthy(name));
    }

    pub fn set_component_degraded(&self, name: &str, message: &str) {
        self.update_component(ComponentHealth::degraded(name, message));
    }

    pub fn set_component_unhealthy(&self, name: &str, message: &str) {
        self.update_component(ComponentHealth::unhealthy(name, message));
    }

    pub fn get_health(&self) -> HealthResponse {
        let components = self.components.lock().unwrap();
        let component_list: Vec<ComponentHealth> = components.values().cloned().collect();

        let overall_status = self.calculate_overall_status(&component_list);

        HealthResponse {
            status: overall_status,
            version: self.version.clone(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            components: component_list,
        }
    }

    fn calculate_overall_status(&self, components: &[ComponentHealth]) -> HealthStatus {
        if components.is_empty() {
            return HealthStatus::Healthy;
        }

        let has_unhealthy = components.iter().any(|c| c.status == HealthStatus::Unhealthy);
        let has_degraded = components.iter().any(|c| c.status == HealthStatus::Degraded);

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.get_health().status.is_healthy()
    }

    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

impl Clone for HealthEndpoint {
    fn clone(&self) -> Self {
        Self {
            version: self.version.clone(),
            start_time: self.start_time,
            components: self.components.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Degraded.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_healthy());
    }

    #[test]
    fn test_component_health_creation() {
        let healthy = ComponentHealth::healthy("database");
        assert_eq!(healthy.status, HealthStatus::Healthy);

        let degraded = ComponentHealth::degraded("cache", "High latency");
        assert_eq!(degraded.status, HealthStatus::Degraded);
        assert_eq!(degraded.message, Some("High latency".to_string()));

        let unhealthy = ComponentHealth::unhealthy("queue", "Connection failed");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_endpoint_basic() {
        let endpoint = HealthEndpoint::new("1.0.0");

        let health = endpoint.get_health();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.version, "1.0.0");
    }

    #[test]
    fn test_health_endpoint_with_components() {
        let endpoint = HealthEndpoint::new("1.0.0");

        endpoint.register_component("database");
        endpoint.register_component("cache");
        endpoint.register_component("queue");

        let health = endpoint.get_health();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.components.len(), 3);
    }

    #[test]
    fn test_health_endpoint_degraded() {
        let endpoint = HealthEndpoint::new("1.0.0");

        endpoint.register_component("database");
        endpoint.set_component_degraded("database", "High latency");

        let health = endpoint.get_health();
        assert_eq!(health.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_endpoint_unhealthy() {
        let endpoint = HealthEndpoint::new("1.0.0");

        endpoint.register_component("database");
        endpoint.register_component("cache");
        endpoint.set_component_unhealthy("database", "Connection failed");

        let health = endpoint.get_health();
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_response_json() {
        let endpoint = HealthEndpoint::new("1.0.0");
        endpoint.register_component("database");

        let health = endpoint.get_health();
        let json = health.to_json();

        assert!(json.contains("Healthy") || json.contains("healthy"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("database"));
    }

    #[test]
    fn test_component_health_with_details() {
        let health = ComponentHealth::healthy("database")
            .with_detail("connections", "10")
            .with_detail("latency_ms", "5");

        assert_eq!(health.details.get("connections"), Some(&"10".to_string()));
        assert_eq!(health.details.get("latency_ms"), Some(&"5".to_string()));
    }
}
