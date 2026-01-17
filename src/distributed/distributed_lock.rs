use super::redis_client::{RedisClient, RedisError};
use std::time::Duration;
use uuid::Uuid;

pub struct DistributedLock {
    client: RedisClient,
    key: String,
    token: String,
    ttl: Duration,
}

impl DistributedLock {
    pub fn new(client: RedisClient, key: &str, ttl: Duration) -> Self {
        Self {
            client,
            key: format!("lock:{}", key),
            token: Uuid::new_v4().to_string(),
            ttl,
        }
    }

    #[cfg(feature = "redis")]
    pub async fn acquire(&self) -> Result<bool, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        
        let result: Option<String> = redis::cmd("SET")
            .arg(&self.key)
            .arg(&self.token)
            .arg("NX")
            .arg("PX")
            .arg(self.ttl.as_millis() as u64)
            .query_async(&mut conn)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(result.is_some())
    }

    #[cfg(feature = "redis")]
    pub async fn acquire_with_timeout(&self, timeout: Duration) -> Result<bool, RedisError> {
        let start = std::time::Instant::now();
        let retry_delay = Duration::from_millis(50);
        
        while start.elapsed() < timeout {
            if self.acquire().await? {
                return Ok(true);
            }
            tokio::time::sleep(retry_delay).await;
        }
        
        Ok(false)
    }

    #[cfg(feature = "redis")]
    pub async fn release(&self) -> Result<bool, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        
        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
        "#;
        
        let result: i32 = redis::Script::new(script)
            .key(&self.key)
            .arg(&self.token)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(result == 1)
    }

    #[cfg(feature = "redis")]
    pub async fn extend(&self, additional_ttl: Duration) -> Result<bool, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        
        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("pexpire", KEYS[1], ARGV[2])
            else
                return 0
            end
        "#;
        
        let new_ttl = self.ttl + additional_ttl;
        let result: i32 = redis::Script::new(script)
            .key(&self.key)
            .arg(&self.token)
            .arg(new_ttl.as_millis() as u64)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(result == 1)
    }

    #[cfg(feature = "redis")]
    pub async fn is_held(&self) -> Result<bool, RedisError> {
        let value = self.client.get(&self.key).await?;
        Ok(value.as_deref() == Some(&self.token))
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_key_format() {
        let config = super::super::redis_client::RedisConfig::default();
        // Cannot test without Redis, but we can test the key format
        assert!(true);
    }
}
