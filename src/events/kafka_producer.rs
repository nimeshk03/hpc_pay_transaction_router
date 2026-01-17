use super::event_types::Event;
use super::event_publisher::{EventPublisher, PublishError};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct KafkaProducerConfig {
    pub brokers: String,
    pub client_id: String,
    pub acks: String,
    pub retries: u32,
    pub retry_backoff_ms: u64,
    pub delivery_timeout_ms: u64,
    pub batch_size: usize,
    pub linger_ms: u64,
}

impl Default for KafkaProducerConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            client_id: "transaction_router".to_string(),
            acks: "all".to_string(),
            retries: 3,
            retry_backoff_ms: 100,
            delivery_timeout_ms: 30000,
            batch_size: 16384,
            linger_ms: 5,
        }
    }
}

impl KafkaProducerConfig {
    pub fn new(brokers: &str) -> Self {
        Self {
            brokers: brokers.to_string(),
            ..Default::default()
        }
    }

    pub fn with_client_id(mut self, client_id: &str) -> Self {
        self.client_id = client_id.to_string();
        self
    }

    pub fn with_acks(mut self, acks: &str) -> Self {
        self.acks = acks.to_string();
        self
    }

    pub fn with_retries(mut self, retries: u32, backoff_ms: u64) -> Self {
        self.retries = retries;
        self.retry_backoff_ms = backoff_ms;
        self
    }

    pub fn with_batching(mut self, batch_size: usize, linger_ms: u64) -> Self {
        self.batch_size = batch_size;
        self.linger_ms = linger_ms;
        self
    }
}

#[cfg(feature = "kafka")]
pub struct KafkaProducer {
    config: KafkaProducerConfig,
    producer: rdkafka::producer::FutureProducer,
}

#[cfg(feature = "kafka")]
impl KafkaProducer {
    pub fn new(config: KafkaProducerConfig) -> Result<Self, PublishError> {
        use rdkafka::ClientConfig;
        use rdkafka::producer::FutureProducer;
        
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("client.id", &config.client_id)
            .set("acks", &config.acks)
            .set("retries", config.retries.to_string())
            .set("retry.backoff.ms", config.retry_backoff_ms.to_string())
            .set("delivery.timeout.ms", config.delivery_timeout_ms.to_string())
            .set("batch.size", config.batch_size.to_string())
            .set("linger.ms", config.linger_ms.to_string())
            .create()
            .map_err(|e| PublishError::ConnectionFailed(e.to_string()))?;
        
        Ok(Self { config, producer })
    }

    pub async fn send(&self, event: &Event) -> Result<(), PublishError> {
        use rdkafka::producer::FutureRecord;
        
        let topic = event.event_type.topic_name();
        let payload = serde_json::to_string(event)
            .map_err(|e| PublishError::SerializationError(e.to_string()))?;
        
        let record = FutureRecord::to(topic)
            .payload(&payload)
            .key(&event.id);
        
        let timeout = Duration::from_millis(self.config.delivery_timeout_ms);
        
        self.producer
            .send(record, timeout)
            .await
            .map_err(|(e, _)| PublishError::DeliveryFailed(e.to_string()))?;
        
        Ok(())
    }

    pub async fn send_batch(&self, events: &[Event]) -> Result<(), PublishError> {
        for event in events {
            self.send(event).await?;
        }
        Ok(())
    }

    pub fn config(&self) -> &KafkaProducerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_config_default() {
        let config = KafkaProducerConfig::default();
        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.acks, "all");
        assert_eq!(config.retries, 3);
    }

    #[test]
    fn test_kafka_config_builder() {
        let config = KafkaProducerConfig::new("kafka1:9092,kafka2:9092")
            .with_client_id("test_client")
            .with_acks("1")
            .with_retries(5, 200)
            .with_batching(32768, 10);
        
        assert_eq!(config.brokers, "kafka1:9092,kafka2:9092");
        assert_eq!(config.client_id, "test_client");
        assert_eq!(config.acks, "1");
        assert_eq!(config.retries, 5);
        assert_eq!(config.batch_size, 32768);
    }
}
