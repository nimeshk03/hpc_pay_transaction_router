use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    RouteRequest,
    RouteSuccess,
    RouteFailure,
    CircuitBreakerOpen,
    CircuitBreakerClose,
    ConfigReload,
    ApiKeyCreated,
    ApiKeyRevoked,
    AuthSuccess,
    AuthFailure,
    RateLimitExceeded,
}

impl AuditAction {
    pub fn severity(&self) -> &'static str {
        match self {
            AuditAction::RouteRequest | AuditAction::RouteSuccess => "INFO",
            AuditAction::RouteFailure | AuditAction::CircuitBreakerOpen => "WARN",
            AuditAction::AuthFailure | AuditAction::RateLimitExceeded => "WARN",
            AuditAction::ApiKeyRevoked => "WARN",
            AuditAction::CircuitBreakerClose | AuditAction::ConfigReload => "INFO",
            AuditAction::ApiKeyCreated | AuditAction::AuthSuccess => "INFO",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: i64,
    pub action: AuditAction,
    pub actor: String,
    pub target: Option<String>,
    pub details: std::collections::HashMap<String, String>,
    pub ip_address: Option<String>,
    pub request_id: Option<String>,
}

impl AuditEntry {
    pub fn new(action: AuditAction, actor: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            action,
            actor: actor.to_string(),
            target: None,
            details: std::collections::HashMap::new(),
            ip_address: None,
            request_id: None,
        }
    }

    pub fn with_target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_ip(mut self, ip: &str) -> Self {
        self.ip_address = Some(ip.to_string());
        self
    }

    pub fn with_request_id(mut self, request_id: &str) -> Self {
        self.request_id = Some(request_id.to_string());
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn contains_sensitive_data(&self) -> bool {
        let sensitive_keys = ["card_number", "cvv", "pin", "password", "secret", "token"];
        
        for key in &sensitive_keys {
            if self.details.contains_key(*key) {
                return true;
            }
            for value in self.details.values() {
                if value.to_lowercase().contains(key) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct AuditLogConfig {
    pub max_entries: usize,
    pub retention_period: Duration,
    pub include_request_details: bool,
    pub mask_sensitive_data: bool,
}

impl Default for AuditLogConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            retention_period: Duration::from_secs(86400 * 30),
            include_request_details: true,
            mask_sensitive_data: true,
        }
    }
}

impl AuditLogConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    pub fn with_retention(mut self, retention: Duration) -> Self {
        self.retention_period = retention;
        self
    }
}

pub struct AuditLogger {
    config: AuditLogConfig,
    entries: Arc<Mutex<VecDeque<AuditEntry>>>,
}

impl AuditLogger {
    pub fn new(config: AuditLogConfig) -> Self {
        Self {
            config,
            entries: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(AuditLogConfig::default())
    }

    pub fn log(&self, mut entry: AuditEntry) {
        if self.config.mask_sensitive_data {
            entry = self.mask_sensitive_fields(entry);
        }

        let mut entries = self.entries.lock().unwrap();

        if entries.len() >= self.config.max_entries {
            entries.pop_front();
        }

        entries.push_back(entry);
    }

    pub fn log_route_request(&self, actor: &str, psp_id: &str, request_id: &str) {
        let entry = AuditEntry::new(AuditAction::RouteRequest, actor)
            .with_target(psp_id)
            .with_request_id(request_id);
        self.log(entry);
    }

    pub fn log_route_success(&self, actor: &str, psp_id: &str, latency_ms: u64) {
        let entry = AuditEntry::new(AuditAction::RouteSuccess, actor)
            .with_target(psp_id)
            .with_detail("latency_ms", &latency_ms.to_string());
        self.log(entry);
    }

    pub fn log_route_failure(&self, actor: &str, psp_id: &str, error: &str) {
        let entry = AuditEntry::new(AuditAction::RouteFailure, actor)
            .with_target(psp_id)
            .with_detail("error", error);
        self.log(entry);
    }

    pub fn log_auth_attempt(&self, actor: &str, success: bool, ip: Option<&str>) {
        let action = if success {
            AuditAction::AuthSuccess
        } else {
            AuditAction::AuthFailure
        };

        let mut entry = AuditEntry::new(action, actor);
        if let Some(ip) = ip {
            entry = entry.with_ip(ip);
        }
        self.log(entry);
    }

    pub fn log_circuit_breaker_change(&self, psp_id: &str, opened: bool) {
        let action = if opened {
            AuditAction::CircuitBreakerOpen
        } else {
            AuditAction::CircuitBreakerClose
        };

        let entry = AuditEntry::new(action, "system")
            .with_target(psp_id);
        self.log(entry);
    }

    fn mask_sensitive_fields(&self, mut entry: AuditEntry) -> AuditEntry {
        let sensitive_keys = ["card_number", "cvv", "pin", "password", "secret", "token"];

        for key in &sensitive_keys {
            if entry.details.contains_key(*key) {
                entry.details.insert(key.to_string(), "[REDACTED]".to_string());
            }
        }

        entry
    }

    pub fn get_entries(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().unwrap();
        entries.iter().rev().take(limit).cloned().collect()
    }

    pub fn get_entries_by_action(&self, action: AuditAction, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .rev()
            .filter(|e| e.action == action)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn get_entries_by_actor(&self, actor: &str, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .rev()
            .filter(|e| e.actor == actor)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().unwrap();
        let query_lower = query.to_lowercase();

        entries
            .iter()
            .rev()
            .filter(|e| {
                e.actor.to_lowercase().contains(&query_lower)
                    || e.target
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || e.details
                        .values()
                        .any(|v| v.to_lowercase().contains(&query_lower))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn count(&self) -> usize {
        let entries = self.entries.lock().unwrap();
        entries.len()
    }

    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
    }

    pub fn export_json(&self) -> String {
        let entries = self.entries.lock().unwrap();
        let vec: Vec<&AuditEntry> = entries.iter().collect();
        serde_json::to_string_pretty(&vec).unwrap_or_default()
    }
}

impl Clone for AuditLogger {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            entries: self.entries.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::new(AuditAction::RouteRequest, "user-123")
            .with_target("stripe")
            .with_detail("amount", "100.00")
            .with_request_id("req-456");

        assert_eq!(entry.action, AuditAction::RouteRequest);
        assert_eq!(entry.actor, "user-123");
        assert_eq!(entry.target, Some("stripe".to_string()));
        assert_eq!(entry.details.get("amount"), Some(&"100.00".to_string()));
    }

    #[test]
    fn test_audit_entry_sensitive_data_detection() {
        let entry = AuditEntry::new(AuditAction::RouteRequest, "user")
            .with_detail("card_number", "4111111111111111");

        assert!(entry.contains_sensitive_data());

        let safe_entry = AuditEntry::new(AuditAction::RouteRequest, "user")
            .with_detail("amount", "100.00");

        assert!(!safe_entry.contains_sensitive_data());
    }

    #[test]
    fn test_audit_logger_log_and_retrieve() {
        let logger = AuditLogger::with_default_config();

        logger.log_route_request("user-1", "stripe", "req-1");
        logger.log_route_success("user-1", "stripe", 50);
        logger.log_route_failure("user-2", "adyen", "timeout");

        let entries = logger.get_entries(10);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_audit_logger_max_entries() {
        let config = AuditLogConfig::default().with_max_entries(5);
        let logger = AuditLogger::new(config);

        for i in 0..10 {
            logger.log_route_request(&format!("user-{}", i), "stripe", &format!("req-{}", i));
        }

        assert_eq!(logger.count(), 5);
    }

    #[test]
    fn test_audit_logger_filter_by_action() {
        let logger = AuditLogger::with_default_config();

        logger.log_route_request("user-1", "stripe", "req-1");
        logger.log_route_success("user-1", "stripe", 50);
        logger.log_route_failure("user-2", "adyen", "timeout");

        let failures = logger.get_entries_by_action(AuditAction::RouteFailure, 10);
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn test_audit_logger_filter_by_actor() {
        let logger = AuditLogger::with_default_config();

        logger.log_route_request("user-1", "stripe", "req-1");
        logger.log_route_request("user-2", "adyen", "req-2");
        logger.log_route_request("user-1", "checkout", "req-3");

        let user1_entries = logger.get_entries_by_actor("user-1", 10);
        assert_eq!(user1_entries.len(), 2);
    }

    #[test]
    fn test_audit_logger_sensitive_data_masking() {
        let logger = AuditLogger::with_default_config();

        let entry = AuditEntry::new(AuditAction::RouteRequest, "user")
            .with_detail("card_number", "4111111111111111")
            .with_detail("amount", "100.00");

        logger.log(entry);

        let entries = logger.get_entries(1);
        assert_eq!(entries[0].details.get("card_number"), Some(&"[REDACTED]".to_string()));
        assert_eq!(entries[0].details.get("amount"), Some(&"100.00".to_string()));
    }

    #[test]
    fn test_audit_logger_search() {
        let logger = AuditLogger::with_default_config();

        logger.log_route_request("alice", "stripe", "req-1");
        logger.log_route_request("bob", "adyen", "req-2");
        logger.log_route_request("alice", "checkout", "req-3");

        let results = logger.search("alice", 10);
        assert_eq!(results.len(), 2);

        let results = logger.search("stripe", 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_audit_action_severity() {
        assert_eq!(AuditAction::RouteSuccess.severity(), "INFO");
        assert_eq!(AuditAction::RouteFailure.severity(), "WARN");
        assert_eq!(AuditAction::AuthFailure.severity(), "WARN");
    }
}
