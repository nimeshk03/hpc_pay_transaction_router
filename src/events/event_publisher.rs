use super::event_types::Event;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PublishError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Delivery failed: {0}")]
    DeliveryFailed(String),
    #[error("Queue full")]
    QueueFull,
}

pub trait EventPublisher: Send + Sync {
    fn publish(&self, event: Event) -> Result<(), PublishError>;
    fn publish_batch(&self, events: Vec<Event>) -> Result<(), PublishError>;
    fn pending_count(&self) -> usize;
    fn flush(&self) -> Result<(), PublishError>;
}

#[derive(Clone)]
pub struct InMemoryEventPublisher {
    events: Arc<Mutex<VecDeque<Event>>>,
    max_size: usize,
}

impl InMemoryEventPublisher {
    pub fn new(max_size: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    pub fn with_default_size() -> Self {
        Self::new(10000)
    }

    pub fn get_events(&self) -> Vec<Event> {
        let events = self.events.lock().unwrap();
        events.iter().cloned().collect()
    }

    pub fn get_events_by_type(&self, event_type: super::event_types::EventType) -> Vec<Event> {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    pub fn get_last_n(&self, n: usize) -> Vec<Event> {
        let events = self.events.lock().unwrap();
        events.iter().rev().take(n).cloned().collect()
    }

    pub fn clear(&self) {
        let mut events = self.events.lock().unwrap();
        events.clear();
    }

    pub fn len(&self) -> usize {
        let events = self.events.lock().unwrap();
        events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl EventPublisher for InMemoryEventPublisher {
    fn publish(&self, event: Event) -> Result<(), PublishError> {
        let mut events = self.events.lock().unwrap();
        
        if events.len() >= self.max_size {
            events.pop_front();
        }
        
        events.push_back(event);
        Ok(())
    }

    fn publish_batch(&self, batch: Vec<Event>) -> Result<(), PublishError> {
        let mut events = self.events.lock().unwrap();
        
        for event in batch {
            if events.len() >= self.max_size {
                events.pop_front();
            }
            events.push_back(event);
        }
        
        Ok(())
    }

    fn pending_count(&self) -> usize {
        self.len()
    }

    fn flush(&self) -> Result<(), PublishError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct BufferedEventPublisher<P: EventPublisher + Clone> {
    inner: P,
    buffer: Arc<Mutex<Vec<Event>>>,
    buffer_size: usize,
}

impl<P: EventPublisher + Clone> BufferedEventPublisher<P> {
    pub fn new(inner: P, buffer_size: usize) -> Self {
        Self {
            inner,
            buffer: Arc::new(Mutex::new(Vec::with_capacity(buffer_size))),
            buffer_size,
        }
    }

    fn flush_buffer(&self) -> Result<(), PublishError> {
        let mut buffer = self.buffer.lock().unwrap();
        if !buffer.is_empty() {
            let events: Vec<Event> = buffer.drain(..).collect();
            self.inner.publish_batch(events)?;
        }
        Ok(())
    }
}

impl<P: EventPublisher + Clone> EventPublisher for BufferedEventPublisher<P> {
    fn publish(&self, event: Event) -> Result<(), PublishError> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.push(event);
        
        if buffer.len() >= self.buffer_size {
            let events: Vec<Event> = buffer.drain(..).collect();
            drop(buffer);
            self.inner.publish_batch(events)?;
        }
        
        Ok(())
    }

    fn publish_batch(&self, events: Vec<Event>) -> Result<(), PublishError> {
        self.inner.publish_batch(events)
    }

    fn pending_count(&self) -> usize {
        let buffer = self.buffer.lock().unwrap();
        buffer.len() + self.inner.pending_count()
    }

    fn flush(&self) -> Result<(), PublishError> {
        self.flush_buffer()?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::event_types::EventType;

    #[test]
    fn test_in_memory_publisher_basic() {
        let publisher = InMemoryEventPublisher::new(100);
        
        let event = Event::new(EventType::RoutingDecision, "test".to_string());
        publisher.publish(event).unwrap();
        
        assert_eq!(publisher.len(), 1);
    }

    #[test]
    fn test_in_memory_publisher_max_size() {
        let publisher = InMemoryEventPublisher::new(5);
        
        for i in 0..10 {
            let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
            publisher.publish(event).unwrap();
        }
        
        assert_eq!(publisher.len(), 5);
        
        let events = publisher.get_events();
        assert!(events[0].payload.contains("event-5"));
    }

    #[test]
    fn test_in_memory_publisher_batch() {
        let publisher = InMemoryEventPublisher::new(100);
        
        let events: Vec<Event> = (0..5)
            .map(|i| Event::new(EventType::RoutingDecision, format!("event-{}", i)))
            .collect();
        
        publisher.publish_batch(events).unwrap();
        
        assert_eq!(publisher.len(), 5);
    }

    #[test]
    fn test_in_memory_publisher_filter_by_type() {
        let publisher = InMemoryEventPublisher::new(100);
        
        publisher.publish(Event::new(EventType::RoutingDecision, "routing".to_string())).unwrap();
        publisher.publish(Event::new(EventType::CircuitBreakerStateChange, "cb".to_string())).unwrap();
        publisher.publish(Event::new(EventType::RoutingDecision, "routing2".to_string())).unwrap();
        
        let routing_events = publisher.get_events_by_type(EventType::RoutingDecision);
        assert_eq!(routing_events.len(), 2);
        
        let cb_events = publisher.get_events_by_type(EventType::CircuitBreakerStateChange);
        assert_eq!(cb_events.len(), 1);
    }

    #[test]
    fn test_in_memory_publisher_get_last_n() {
        let publisher = InMemoryEventPublisher::new(100);
        
        for i in 0..10 {
            let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
            publisher.publish(event).unwrap();
        }
        
        let last_3 = publisher.get_last_n(3);
        assert_eq!(last_3.len(), 3);
        assert!(last_3[0].payload.contains("event-9"));
    }

    #[test]
    fn test_in_memory_publisher_clear() {
        let publisher = InMemoryEventPublisher::new(100);
        
        for i in 0..5 {
            let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
            publisher.publish(event).unwrap();
        }
        
        assert_eq!(publisher.len(), 5);
        publisher.clear();
        assert_eq!(publisher.len(), 0);
        assert!(publisher.is_empty());
    }

    #[test]
    fn test_buffered_publisher() {
        let inner = InMemoryEventPublisher::new(100);
        let buffered = BufferedEventPublisher::new(inner.clone(), 5);
        
        for i in 0..4 {
            let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
            buffered.publish(event).unwrap();
        }
        
        assert_eq!(inner.len(), 0);
        
        let event = Event::new(EventType::RoutingDecision, "event-4".to_string());
        buffered.publish(event).unwrap();
        
        assert_eq!(inner.len(), 5);
    }

    #[test]
    fn test_buffered_publisher_flush() {
        let inner = InMemoryEventPublisher::new(100);
        let buffered = BufferedEventPublisher::new(inner.clone(), 10);
        
        for i in 0..3 {
            let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
            buffered.publish(event).unwrap();
        }
        
        assert_eq!(inner.len(), 0);
        
        buffered.flush().unwrap();
        
        assert_eq!(inner.len(), 3);
    }
}
