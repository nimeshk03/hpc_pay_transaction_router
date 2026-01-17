use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    #[error("Missing API key")]
    MissingApiKey,
    #[error("Invalid API key")]
    InvalidApiKey,
    #[error("API key expired")]
    ExpiredApiKey,
    #[error("API key revoked")]
    RevokedApiKey,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub key_id: String,
    pub key_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub revoked: bool,
    pub rate_limit: Option<u32>,
}

impl ApiKey {
    pub fn new(key_id: &str, key_hash: &str, name: &str) -> Self {
        Self {
            key_id: key_id.to_string(),
            key_hash: key_hash.to_string(),
            name: name.to_string(),
            permissions: vec!["read".to_string(), "route".to_string()],
            created_at: chrono::Utc::now().timestamp(),
            expires_at: None,
            revoked: false,
            rate_limit: None,
        }
    }

    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn with_expiry(mut self, expires_at: i64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn with_rate_limit(mut self, limit: u32) -> Self {
        self.rate_limit = Some(limit);
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now().timestamp() > expires_at
        } else {
            false
        }
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
            || self.permissions.contains(&"admin".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub header_name: String,
    pub require_https: bool,
    pub default_rate_limit: u32,
    pub rate_limit_window: Duration,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            header_name: "X-API-Key".to_string(),
            require_https: true,
            default_rate_limit: 1000,
            rate_limit_window: Duration::from_secs(60),
        }
    }
}

impl AuthConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    pub fn with_header_name(mut self, name: &str) -> Self {
        self.header_name = name.to_string();
        self
    }

    pub fn with_rate_limit(mut self, limit: u32, window: Duration) -> Self {
        self.default_rate_limit = limit;
        self.rate_limit_window = window;
        self
    }
}

#[derive(Debug)]
struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

pub struct ApiAuthenticator {
    config: AuthConfig,
    keys: Arc<Mutex<HashMap<String, ApiKey>>>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimitEntry>>>,
}

impl ApiAuthenticator {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            keys: Arc::new(Mutex::new(HashMap::new())),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(AuthConfig::default())
    }

    pub fn register_key(&self, api_key: ApiKey) {
        let mut keys = self.keys.lock().unwrap();
        keys.insert(api_key.key_hash.clone(), api_key);
    }

    pub fn revoke_key(&self, key_hash: &str) -> bool {
        let mut keys = self.keys.lock().unwrap();
        if let Some(key) = keys.get_mut(key_hash) {
            key.revoked = true;
            true
        } else {
            false
        }
    }

    pub fn authenticate(&self, key_hash: Option<&str>) -> Result<ApiKey, AuthError> {
        if !self.config.enabled {
            return Ok(ApiKey::new("anonymous", "anonymous", "Anonymous")
                .with_permissions(vec!["read".to_string(), "route".to_string()]));
        }

        let key_hash = key_hash.ok_or(AuthError::MissingApiKey)?;

        let keys = self.keys.lock().unwrap();
        let api_key = keys.get(key_hash).ok_or(AuthError::InvalidApiKey)?;

        if api_key.revoked {
            return Err(AuthError::RevokedApiKey);
        }

        if api_key.is_expired() {
            return Err(AuthError::ExpiredApiKey);
        }

        self.check_rate_limit(key_hash, api_key.rate_limit)?;

        Ok(api_key.clone())
    }

    pub fn authenticate_with_permission(
        &self,
        key_hash: Option<&str>,
        required_permission: &str,
    ) -> Result<ApiKey, AuthError> {
        let api_key = self.authenticate(key_hash)?;

        if !api_key.has_permission(required_permission) {
            return Err(AuthError::InsufficientPermissions);
        }

        Ok(api_key)
    }

    fn check_rate_limit(&self, key_hash: &str, custom_limit: Option<u32>) -> Result<(), AuthError> {
        let mut rate_limits = self.rate_limits.lock().unwrap();
        let limit = custom_limit.unwrap_or(self.config.default_rate_limit);

        let entry = rate_limits
            .entry(key_hash.to_string())
            .or_insert(RateLimitEntry {
                count: 0,
                window_start: Instant::now(),
            });

        if entry.window_start.elapsed() >= self.config.rate_limit_window {
            entry.count = 0;
            entry.window_start = Instant::now();
        }

        if entry.count >= limit {
            return Err(AuthError::RateLimitExceeded);
        }

        entry.count += 1;
        Ok(())
    }

    pub fn get_key_info(&self, key_hash: &str) -> Option<ApiKey> {
        let keys = self.keys.lock().unwrap();
        keys.get(key_hash).cloned()
    }

    pub fn list_keys(&self) -> Vec<ApiKey> {
        let keys = self.keys.lock().unwrap();
        keys.values().cloned().collect()
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn config(&self) -> &AuthConfig {
        &self.config
    }
}

impl Clone for ApiAuthenticator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            keys: self.keys.clone(),
            rate_limits: self.rate_limits.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_creation() {
        let key = ApiKey::new("key-1", "hash123", "Test Key")
            .with_permissions(vec!["read".to_string(), "write".to_string()])
            .with_rate_limit(100);

        assert_eq!(key.key_id, "key-1");
        assert_eq!(key.name, "Test Key");
        assert!(key.has_permission("read"));
        assert!(key.has_permission("write"));
        assert!(!key.has_permission("admin"));
    }

    #[test]
    fn test_api_key_expiry() {
        let expired_key = ApiKey::new("key-1", "hash123", "Expired Key")
            .with_expiry(chrono::Utc::now().timestamp() - 3600);

        assert!(expired_key.is_expired());

        let valid_key = ApiKey::new("key-2", "hash456", "Valid Key")
            .with_expiry(chrono::Utc::now().timestamp() + 3600);

        assert!(!valid_key.is_expired());
    }

    #[test]
    fn test_authenticator_disabled() {
        let auth = ApiAuthenticator::new(AuthConfig::disabled());

        let result = auth.authenticate(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_authenticator_missing_key() {
        let auth = ApiAuthenticator::with_default_config();

        let result = auth.authenticate(None);
        assert_eq!(result.err(), Some(AuthError::MissingApiKey));
    }

    #[test]
    fn test_authenticator_invalid_key() {
        let auth = ApiAuthenticator::with_default_config();

        let result = auth.authenticate(Some("invalid_key"));
        assert_eq!(result.err(), Some(AuthError::InvalidApiKey));
    }

    #[test]
    fn test_authenticator_valid_key() {
        let auth = ApiAuthenticator::with_default_config();
        let key = ApiKey::new("key-1", "valid_hash", "Test Key");
        auth.register_key(key);

        let result = auth.authenticate(Some("valid_hash"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().key_id, "key-1");
    }

    #[test]
    fn test_authenticator_revoked_key() {
        let auth = ApiAuthenticator::with_default_config();
        let key = ApiKey::new("key-1", "revoked_hash", "Revoked Key");
        auth.register_key(key);
        auth.revoke_key("revoked_hash");

        let result = auth.authenticate(Some("revoked_hash"));
        assert_eq!(result.err(), Some(AuthError::RevokedApiKey));
    }

    #[test]
    fn test_authenticator_permission_check() {
        let auth = ApiAuthenticator::with_default_config();
        let key = ApiKey::new("key-1", "hash123", "Limited Key")
            .with_permissions(vec!["read".to_string()]);
        auth.register_key(key);

        let result = auth.authenticate_with_permission(Some("hash123"), "read");
        assert!(result.is_ok());

        let result = auth.authenticate_with_permission(Some("hash123"), "write");
        assert_eq!(result.err(), Some(AuthError::InsufficientPermissions));
    }

    #[test]
    fn test_authenticator_rate_limit() {
        let config = AuthConfig::default()
            .with_rate_limit(5, Duration::from_secs(60));
        let auth = ApiAuthenticator::new(config);
        let key = ApiKey::new("key-1", "hash123", "Test Key");
        auth.register_key(key);

        for _ in 0..5 {
            let result = auth.authenticate(Some("hash123"));
            assert!(result.is_ok());
        }

        let result = auth.authenticate(Some("hash123"));
        assert_eq!(result.err(), Some(AuthError::RateLimitExceeded));
    }
}
