use transaction_router::{
    ApiAuthenticator, ApiKey, AuthConfig, AuthError,
    AuditLogger, AuditLogConfig, AuditEntry, AuditAction,
    DataProtector, SensitiveDataMasker, EncryptionConfig,
};
use std::time::Duration;
use std::collections::HashMap;

#[test]
fn test_api_authentication_workflow() {
    let auth = ApiAuthenticator::with_default_config();
    
    let key = ApiKey::new("key-1", "valid_hash_123", "Production Key")
        .with_permissions(vec!["read".to_string(), "route".to_string()]);
    
    auth.register_key(key);
    
    let result = auth.authenticate(Some("valid_hash_123"));
    assert!(result.is_ok());
    
    let api_key = result.unwrap();
    assert_eq!(api_key.key_id, "key-1");
    assert!(api_key.has_permission("read"));
    assert!(api_key.has_permission("route"));
}

#[test]
fn test_api_authentication_missing_key() {
    let auth = ApiAuthenticator::with_default_config();
    
    let result = auth.authenticate(None);
    assert_eq!(result.err(), Some(AuthError::MissingApiKey));
}

#[test]
fn test_api_authentication_invalid_key() {
    let auth = ApiAuthenticator::with_default_config();
    
    let result = auth.authenticate(Some("nonexistent_key"));
    assert_eq!(result.err(), Some(AuthError::InvalidApiKey));
}

#[test]
fn test_api_authentication_revoked_key() {
    let auth = ApiAuthenticator::with_default_config();
    
    let key = ApiKey::new("key-1", "to_revoke", "Revokable Key");
    auth.register_key(key);
    
    auth.revoke_key("to_revoke");
    
    let result = auth.authenticate(Some("to_revoke"));
    assert_eq!(result.err(), Some(AuthError::RevokedApiKey));
}

#[test]
fn test_api_authentication_permission_check() {
    let auth = ApiAuthenticator::with_default_config();
    
    let key = ApiKey::new("key-1", "limited_key", "Limited Key")
        .with_permissions(vec!["read".to_string()]);
    
    auth.register_key(key);
    
    let result = auth.authenticate_with_permission(Some("limited_key"), "read");
    assert!(result.is_ok());
    
    let result = auth.authenticate_with_permission(Some("limited_key"), "write");
    assert_eq!(result.err(), Some(AuthError::InsufficientPermissions));
}

#[test]
fn test_api_authentication_rate_limiting() {
    let config = AuthConfig::default()
        .with_rate_limit(5, Duration::from_secs(60));
    let auth = ApiAuthenticator::new(config);
    
    let key = ApiKey::new("key-1", "rate_limited", "Rate Limited Key");
    auth.register_key(key);
    
    for _ in 0..5 {
        let result = auth.authenticate(Some("rate_limited"));
        assert!(result.is_ok());
    }
    
    let result = auth.authenticate(Some("rate_limited"));
    assert_eq!(result.err(), Some(AuthError::RateLimitExceeded));
}

#[test]
fn test_api_authentication_disabled() {
    let auth = ApiAuthenticator::new(AuthConfig::disabled());
    
    let result = auth.authenticate(None);
    assert!(result.is_ok());
}

#[test]
fn test_audit_logger_basic() {
    let logger = AuditLogger::with_default_config();
    
    logger.log_route_request("user-123", "stripe", "req-001");
    logger.log_route_success("user-123", "stripe", 45);
    logger.log_route_failure("user-456", "adyen", "timeout");
    
    let entries = logger.get_entries(10);
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_audit_logger_filter_by_action() {
    let logger = AuditLogger::with_default_config();
    
    logger.log_route_request("user-1", "stripe", "req-1");
    logger.log_route_success("user-1", "stripe", 50);
    logger.log_route_failure("user-2", "adyen", "error");
    logger.log_route_failure("user-3", "checkout", "timeout");
    
    let failures = logger.get_entries_by_action(AuditAction::RouteFailure, 10);
    assert_eq!(failures.len(), 2);
}

#[test]
fn test_audit_logger_filter_by_actor() {
    let logger = AuditLogger::with_default_config();
    
    logger.log_route_request("alice", "stripe", "req-1");
    logger.log_route_request("bob", "adyen", "req-2");
    logger.log_route_request("alice", "checkout", "req-3");
    
    let alice_entries = logger.get_entries_by_actor("alice", 10);
    assert_eq!(alice_entries.len(), 2);
}

#[test]
fn test_audit_logger_sensitive_data_masking() {
    let logger = AuditLogger::with_default_config();
    
    let entry = AuditEntry::new(AuditAction::RouteRequest, "user")
        .with_detail("card_number", "4111111111111111")
        .with_detail("amount", "100.00")
        .with_detail("cvv", "123");
    
    logger.log(entry);
    
    let entries = logger.get_entries(1);
    assert_eq!(entries[0].details.get("card_number"), Some(&"[REDACTED]".to_string()));
    assert_eq!(entries[0].details.get("cvv"), Some(&"[REDACTED]".to_string()));
    assert_eq!(entries[0].details.get("amount"), Some(&"100.00".to_string()));
}

#[test]
fn test_audit_logger_search() {
    let logger = AuditLogger::with_default_config();
    
    logger.log_route_request("alice", "stripe", "req-1");
    logger.log_route_request("bob", "adyen", "req-2");
    logger.log_route_request("charlie", "stripe", "req-3");
    
    let results = logger.search("stripe", 10);
    assert_eq!(results.len(), 2);
    
    let results = logger.search("alice", 10);
    assert_eq!(results.len(), 1);
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
fn test_sensitive_data_masker_card_number() {
    let masker = SensitiveDataMasker::new();
    
    let masked = masker.mask("card_number", "4111111111111111");
    assert_eq!(masked, "4111********1111");
}

#[test]
fn test_sensitive_data_masker_cvv() {
    let masker = SensitiveDataMasker::new();
    
    let masked = masker.mask("cvv", "123");
    assert_eq!(masked, "***");
}

#[test]
fn test_sensitive_data_masker_email() {
    let masker = SensitiveDataMasker::new();
    
    let masked = masker.mask("email", "user@example.com");
    assert_eq!(masked, "us**************");
}

#[test]
fn test_sensitive_data_masker_map() {
    let masker = SensitiveDataMasker::new();
    
    let mut data = HashMap::new();
    data.insert("card_number".to_string(), "4111111111111111".to_string());
    data.insert("amount".to_string(), "100.00".to_string());
    data.insert("cvv".to_string(), "123".to_string());
    
    let masked = masker.mask_map(&data);
    
    assert_eq!(masked.get("card_number"), Some(&"4111********1111".to_string()));
    assert_eq!(masked.get("amount"), Some(&"100.00".to_string()));
    assert_eq!(masked.get("cvv"), Some(&"***".to_string()));
}

#[test]
fn test_data_protector_tls_validation() {
    let protector = DataProtector::with_default_config();
    
    assert!(protector.validate_tls_required("https://api.stripe.com"));
    assert!(!protector.validate_tls_required("http://insecure.example.com"));
}

#[test]
fn test_data_protector_sensitive_field_detection() {
    let protector = DataProtector::with_default_config();
    
    assert!(protector.is_sensitive("card_number"));
    assert!(protector.is_sensitive("cvv"));
    assert!(protector.is_sensitive("password"));
    assert!(!protector.is_sensitive("amount"));
    assert!(!protector.is_sensitive("merchant_id"));
}

#[test]
fn test_encryption_config() {
    let config = EncryptionConfig::new()
        .with_algorithm("AES-128-GCM")
        .with_key_rotation(30);
    
    assert_eq!(config.algorithm, "AES-128-GCM");
    assert_eq!(config.key_rotation_days, 30);
    assert!(config.encrypt_at_rest);
    assert!(config.encrypt_in_transit);
}

#[test]
fn test_audit_entry_json_serialization() {
    let entry = AuditEntry::new(AuditAction::RouteRequest, "user-123")
        .with_target("stripe")
        .with_detail("amount", "100.00")
        .with_request_id("req-456");
    
    let json = entry.to_json();
    
    assert!(json.contains("user-123"));
    assert!(json.contains("stripe"));
    assert!(json.contains("100.00"));
}

#[test]
fn test_audit_action_severity() {
    assert_eq!(AuditAction::RouteSuccess.severity(), "INFO");
    assert_eq!(AuditAction::RouteFailure.severity(), "WARN");
    assert_eq!(AuditAction::AuthFailure.severity(), "WARN");
    assert_eq!(AuditAction::ConfigReload.severity(), "INFO");
}
