#[cfg(feature = "kafka")]
pub mod kafka_producer;
#[cfg(feature = "kafka")]
pub mod kafka_consumer;
pub mod event_types;
pub mod event_publisher;

#[cfg(feature = "kafka")]
pub use kafka_producer::{KafkaProducer, KafkaProducerConfig};
#[cfg(feature = "kafka")]
pub use kafka_consumer::{KafkaConsumer, KafkaConsumerConfig};
pub use event_types::{Event, EventType, RoutingEvent, CircuitBreakerEvent, ConfigEvent};
pub use event_publisher::{EventPublisher, InMemoryEventPublisher};
