use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("Lock not acquired")]
    LockNotAcquired,
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub connection_timeout: Duration,
    pub operation_timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            connection_timeout: Duration::from_secs(5),
            operation_timeout: Duration::from_secs(2),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

impl RedisConfig {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            ..Default::default()
        }
    }

    pub fn with_timeouts(mut self, connection: Duration, operation: Duration) -> Self {
        self.connection_timeout = connection;
        self.operation_timeout = operation;
        self
    }

    pub fn with_retries(mut self, max_retries: u32, delay: Duration) -> Self {
        self.max_retries = max_retries;
        self.retry_delay = delay;
        self
    }
}

#[derive(Clone)]
pub struct RedisClient {
    config: RedisConfig,
    #[cfg(feature = "redis")]
    client: redis::Client,
}

impl RedisClient {
    #[cfg(feature = "redis")]
    pub fn new(config: RedisConfig) -> Result<Self, RedisError> {
        let client = redis::Client::open(config.url.as_str())
            .map_err(|e| RedisError::ConnectionFailed(e.to_string()))?;
        
        Ok(Self { config, client })
    }

    #[cfg(feature = "redis")]
    pub async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, RedisError> {
        let conn = self.client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| RedisError::ConnectionFailed(e.to_string()))?;
        
        Ok(conn)
    }

    #[cfg(feature = "redis")]
    pub async fn set(&self, key: &str, value: &str) -> Result<(), RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        conn.set(key, value)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(())
    }

    #[cfg(feature = "redis")]
    pub async fn get(&self, key: &str) -> Result<Option<String>, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        let value: Option<String> = conn.get(key)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(value)
    }

    #[cfg(feature = "redis")]
    pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        conn.set_ex(key, value, ttl_secs)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(())
    }

    #[cfg(feature = "redis")]
    pub async fn del(&self, key: &str) -> Result<bool, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        let deleted: i32 = conn.del(key)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(deleted > 0)
    }

    #[cfg(feature = "redis")]
    pub async fn incr(&self, key: &str) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        let value: i64 = conn.incr(key, 1)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(value)
    }

    #[cfg(feature = "redis")]
    pub async fn decr(&self, key: &str) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        let value: i64 = conn.decr(key, 1)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(value)
    }

    #[cfg(feature = "redis")]
    pub async fn publish(&self, channel: &str, message: &str) -> Result<i32, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        let receivers: i32 = conn.publish(channel, message)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(receivers)
    }

    #[cfg(feature = "redis")]
    pub async fn xadd(&self, stream: &str, fields: &[(&str, &str)]) -> Result<String, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.get_connection().await?;
        let id: String = conn.xadd(stream, "*", fields)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(id)
    }

    pub fn config(&self) -> &RedisConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config_default() {
        let config = RedisConfig::default();
        assert_eq!(config.url, "redis://127.0.0.1:6379");
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_redis_config_custom() {
        let config = RedisConfig::new("redis://localhost:6380")
            .with_timeouts(Duration::from_secs(10), Duration::from_secs(5))
            .with_retries(5, Duration::from_millis(200));
        
        assert_eq!(config.url, "redis://localhost:6380");
        assert_eq!(config.connection_timeout, Duration::from_secs(10));
        assert_eq!(config.max_retries, 5);
    }
}
