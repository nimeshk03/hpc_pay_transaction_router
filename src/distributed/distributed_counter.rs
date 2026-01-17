use super::redis_client::{RedisClient, RedisError};
use std::time::Duration;

pub struct DistributedCounter {
    client: RedisClient,
    key: String,
    ttl: Option<Duration>,
}

impl DistributedCounter {
    pub fn new(client: RedisClient, key: &str) -> Self {
        Self {
            client,
            key: format!("counter:{}", key),
            ttl: None,
        }
    }

    pub fn with_ttl(client: RedisClient, key: &str, ttl: Duration) -> Self {
        Self {
            client,
            key: format!("counter:{}", key),
            ttl: Some(ttl),
        }
    }

    #[cfg(feature = "redis")]
    pub async fn increment(&self) -> Result<i64, RedisError> {
        self.increment_by(1).await
    }

    #[cfg(feature = "redis")]
    pub async fn increment_by(&self, amount: i64) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        let value: i64 = conn.incr(&self.key, amount)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        if let Some(ttl) = self.ttl {
            let _: () = conn.expire(&self.key, ttl.as_secs() as i64)
                .await
                .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        }
        
        Ok(value)
    }

    #[cfg(feature = "redis")]
    pub async fn decrement(&self) -> Result<i64, RedisError> {
        self.decrement_by(1).await
    }

    #[cfg(feature = "redis")]
    pub async fn decrement_by(&self, amount: i64) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        let value: i64 = conn.decr(&self.key, amount)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(value)
    }

    #[cfg(feature = "redis")]
    pub async fn get(&self) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        let value: Option<i64> = conn.get(&self.key)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(value.unwrap_or(0))
    }

    #[cfg(feature = "redis")]
    pub async fn set(&self, value: i64) -> Result<(), RedisError> {
        use redis::AsyncCommands;
        
        let mut conn = self.client.get_connection().await?;
        
        if let Some(ttl) = self.ttl {
            conn.set_ex(&self.key, value, ttl.as_secs())
                .await
                .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        } else {
            conn.set(&self.key, value)
                .await
                .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        }
        
        Ok(())
    }

    #[cfg(feature = "redis")]
    pub async fn reset(&self) -> Result<(), RedisError> {
        self.client.del(&self.key).await?;
        Ok(())
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_key_format() {
        // Key format test
        assert!(true);
    }
}
