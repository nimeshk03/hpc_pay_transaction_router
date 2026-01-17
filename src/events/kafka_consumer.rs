use super::event_types::Event;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsumeError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    #[error("Timeout")]
    Timeout,
    #[error("Consumer error: {0}")]
    ConsumerError(String),
}

#[derive(Debug, Clone)]
pub struct KafkaConsumerConfig {
    pub brokers: String,
    pub group_id: String,
    pub topics: Vec<String>,
    pub auto_offset_reset: String,
    pub enable_auto_commit: bool,
    pub session_timeout_ms: u64,
    pub max_poll_interval_ms: u64,
}

impl Default for KafkaConsumerConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            group_id: "transaction_router_consumer".to_string(),
            topics: vec!["transaction.routed".to_string()],
            auto_offset_reset: "earliest".to_string(),
            enable_auto_commit: true,
            session_timeout_ms: 30000,
            max_poll_interval_ms: 300000,
        }
    }
}

impl KafkaConsumerConfig {
    pub fn new(brokers: &str, group_id: &str) -> Self {
        Self {
            brokers: brokers.to_string(),
            group_id: group_id.to_string(),
            ..Default::default()
        }
    }

    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    pub fn with_auto_offset_reset(mut self, reset: &str) -> Self {
        self.auto_offset_reset = reset.to_string();
        self
    }

    pub fn with_auto_commit(mut self, enable: bool) -> Self {
        self.enable_auto_commit = enable;
        self
    }
}

#[cfg(feature = "kafka")]
pub struct KafkaConsumer {
    config: KafkaConsumerConfig,
    consumer: rdkafka::consumer::StreamConsumer,
}

#[cfg(feature = "kafka")]
impl KafkaConsumer {
    pub fn new(config: KafkaConsumerConfig) -> Result<Self, ConsumeError> {
        use rdkafka::ClientConfig;
        use rdkafka::consumer::{Consumer, StreamConsumer};
        
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &config.group_id)
            .set("auto.offset.reset", &config.auto_offset_reset)
            .set("enable.auto.commit", config.enable_auto_commit.to_string())
            .set("session.timeout.ms", config.session_timeout_ms.to_string())
            .set("max.poll.interval.ms", config.max_poll_interval_ms.to_string())
            .create()
            .map_err(|e| ConsumeError::ConnectionFailed(e.to_string()))?;
        
        let topics: Vec<&str> = config.topics.iter().map(|s| s.as_str()).collect();
        consumer
            .subscribe(&topics)
            .map_err(|e| ConsumeError::ConsumerError(e.to_string()))?;
        
        Ok(Self { config, consumer })
    }

    pub async fn poll(&self, timeout: Duration) -> Result<Option<Event>, ConsumeError> {
        use rdkafka::consumer::Consumer;
        use rdkafka::Message;
        
        match tokio::time::timeout(timeout, self.consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    let payload_str = std::str::from_utf8(payload)
                        .map_err(|e| ConsumeError::DeserializationError(e.to_string()))?;
                    
                    let event: Event = serde_json::from_str(payload_str)
                        .map_err(|e| ConsumeError::DeserializationError(e.to_string()))?;
                    
                    Ok(Some(event))
                } else {
                    Ok(None)
                }
            }
            Ok(Err(e)) => Err(ConsumeError::ConsumerError(e.to_string())),
            Err(_) => Err(ConsumeError::Timeout),
        }
    }

    pub async fn poll_batch(&self, max_count: usize, timeout: Duration) -> Result<Vec<Event>, ConsumeError> {
        let mut events = Vec::with_capacity(max_count);
        let start = std::time::Instant::now();
        
        while events.len() < max_count && start.elapsed() < timeout {
            let remaining = timeout - start.elapsed();
            match self.poll(remaining).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => continue,
                Err(ConsumeError::Timeout) => break,
                Err(e) => return Err(e),
            }
        }
        
        Ok(events)
    }

    pub fn config(&self) -> &KafkaConsumerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consumer_config_default() {
        let config = KafkaConsumerConfig::default();
        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.auto_offset_reset, "earliest");
        assert!(config.enable_auto_commit);
    }

    #[test]
    fn test_consumer_config_builder() {
        let config = KafkaConsumerConfig::new("kafka:9092", "my_group")
            .with_topics(vec!["topic1".to_string(), "topic2".to_string()])
            .with_auto_offset_reset("latest")
            .with_auto_commit(false);
        
        assert_eq!(config.brokers, "kafka:9092");
        assert_eq!(config.group_id, "my_group");
        assert_eq!(config.topics.len(), 2);
        assert_eq!(config.auto_offset_reset, "latest");
        assert!(!config.enable_auto_commit);
    }
}
