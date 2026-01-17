use super::redis_client::{RedisClient, RedisError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    pub psp_id: String,
    pub state: String,
    pub failure_count: u32,
    pub last_failure_time: Option<i64>,
    pub last_success_time: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: i64,
    pub event_type: String,
    pub psp_id: String,
    pub details: HashMap<String, String>,
}

pub struct StateSync {
    client: Arc<RedisClient>,
    prefix: String,
}

impl StateSync {
    pub fn new(client: RedisClient, prefix: &str) -> Self {
        Self {
            client: Arc::new(client),
            prefix: prefix.to_string(),
        }
    }

    fn key(&self, suffix: &str) -> String {
        format!("{}:{}", self.prefix, suffix)
    }

    #[cfg(feature = "redis")]
    pub async fn publish_circuit_breaker_state(
        &self,
        state: &CircuitBreakerState,
    ) -> Result<(), RedisError> {
        let channel = self.key("circuit_breaker");
        let message = serde_json::to_string(state)
            .map_err(|e| RedisError::SerializationError(e.to_string()))?;
        
        self.client.publish(&channel, &message).await?;
        
        let state_key = self.key(&format!("cb_state:{}", state.psp_id));
        self.client.set(&state_key, &message).await?;
        
        Ok(())
    }

    #[cfg(feature = "redis")]
    pub async fn get_circuit_breaker_state(
        &self,
        psp_id: &str,
    ) -> Result<Option<CircuitBreakerState>, RedisError> {
        let state_key = self.key(&format!("cb_state:{}", psp_id));
        
        if let Some(data) = self.client.get(&state_key).await? {
            let state: CircuitBreakerState = serde_json::from_str(&data)
                .map_err(|e| RedisError::SerializationError(e.to_string()))?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    #[cfg(feature = "redis")]
    pub async fn append_audit_log(&self, entry: &AuditLogEntry) -> Result<String, RedisError> {
        let stream_key = self.key("audit_log");
        let data = serde_json::to_string(entry)
            .map_err(|e| RedisError::SerializationError(e.to_string()))?;
        
        self.client.xadd(&stream_key, &[("data", &data)]).await
    }

    #[cfg(feature = "redis")]
    pub async fn get_audit_logs(&self, count: usize) -> Result<Vec<AuditLogEntry>, RedisError> {
        use redis::AsyncCommands;
        
        let stream_key = self.key("audit_log");
        let mut conn = self.client.get_connection().await?;
        
        let results: Vec<redis::streams::StreamReadReply> = redis::cmd("XREVRANGE")
            .arg(&stream_key)
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(count)
            .query_async(&mut conn)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        let mut entries = Vec::new();
        for reply in results {
            for key in reply.keys {
                for id in key.ids {
                    if let Some(data) = id.map.get("data") {
                        if let redis::Value::BulkString(bytes) = data {
                            if let Ok(s) = String::from_utf8(bytes.clone()) {
                                if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(&s) {
                                    entries.push(entry);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(entries)
    }

    #[cfg(feature = "redis")]
    pub async fn set_psp_volume(&self, psp_id: &str, volume: i64) -> Result<(), RedisError> {
        use redis::AsyncCommands;
        
        let key = self.key(&format!("volume:{}", psp_id));
        let mut conn = self.client.get_connection().await?;
        
        conn.set_ex(&key, volume, 3600)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(())
    }

    #[cfg(feature = "redis")]
    pub async fn get_psp_volume(&self, psp_id: &str) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let key = self.key(&format!("volume:{}", psp_id));
        let mut conn = self.client.get_connection().await?;
        
        let volume: Option<i64> = conn.get(&key)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(volume.unwrap_or(0))
    }

    #[cfg(feature = "redis")]
    pub async fn increment_psp_volume(&self, psp_id: &str) -> Result<i64, RedisError> {
        use redis::AsyncCommands;
        
        let key = self.key(&format!("volume:{}", psp_id));
        let mut conn = self.client.get_connection().await?;
        
        let volume: i64 = conn.incr(&key, 1)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        let _: () = conn.expire(&key, 3600)
            .await
            .map_err(|e| RedisError::OperationFailed(e.to_string()))?;
        
        Ok(volume)
    }

    pub fn client(&self) -> Arc<RedisClient> {
        self.client.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_state_serialization() {
        let state = CircuitBreakerState {
            psp_id: "stripe".to_string(),
            state: "open".to_string(),
            failure_count: 5,
            last_failure_time: Some(1234567890),
            last_success_time: None,
        };
        
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: CircuitBreakerState = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.psp_id, "stripe");
        assert_eq!(deserialized.state, "open");
        assert_eq!(deserialized.failure_count, 5);
    }

    #[test]
    fn test_audit_log_entry_serialization() {
        let mut details = HashMap::new();
        details.insert("route".to_string(), "stripe".to_string());
        
        let entry = AuditLogEntry {
            id: "test-123".to_string(),
            timestamp: 1234567890,
            event_type: "routing_decision".to_string(),
            psp_id: "stripe".to_string(),
            details,
        };
        
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditLogEntry = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.id, "test-123");
        assert_eq!(deserialized.event_type, "routing_decision");
    }
}
