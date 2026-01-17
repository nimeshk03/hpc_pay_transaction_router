use transaction_router::{
    Event, EventType, RoutingEvent, CircuitBreakerEvent, ConfigEvent,
    EventPublisher, InMemoryEventPublisher,
};

#[test]
fn test_event_publishing_workflow() {
    let publisher = InMemoryEventPublisher::new(100);
    
    let routing_event = RoutingEvent::new(
        "req-001".to_string(),
        "tx-001".to_string(),
        "stripe".to_string(),
        45,
    )
    .with_fallbacks(vec!["adyen".to_string()])
    .with_score(0.95, "lowest latency");
    
    publisher.publish(routing_event.to_event()).unwrap();
    
    assert_eq!(publisher.len(), 1);
    
    let events = publisher.get_events_by_type(EventType::RoutingDecision);
    assert_eq!(events.len(), 1);
    assert!(events[0].payload.contains("stripe"));
}

#[test]
fn test_circuit_breaker_event_publishing() {
    let publisher = InMemoryEventPublisher::new(100);
    
    let cb_event = CircuitBreakerEvent::new(
        "stripe".to_string(),
        "closed".to_string(),
        "open".to_string(),
    )
    .with_counts(5, 95)
    .with_reason("failure threshold exceeded");
    
    publisher.publish(cb_event.to_event()).unwrap();
    
    let events = publisher.get_events_by_type(EventType::CircuitBreakerStateChange);
    assert_eq!(events.len(), 1);
    
    let event = &events[0];
    assert_eq!(event.metadata.get("psp_id"), Some(&"stripe".to_string()));
    assert_eq!(event.metadata.get("new_state"), Some(&"open".to_string()));
}

#[test]
fn test_config_event_publishing() {
    let publisher = InMemoryEventPublisher::new(100);
    
    let config_event = ConfigEvent::new(2, "reload")
        .with_previous_version(1)
        .with_changed_keys(vec!["routes".to_string(), "limits".to_string()]);
    
    publisher.publish(config_event.to_event()).unwrap();
    
    let events = publisher.get_events_by_type(EventType::ConfigurationUpdate);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].metadata.get("version"), Some(&"2".to_string()));
}

#[test]
fn test_mixed_event_types() {
    let publisher = InMemoryEventPublisher::new(100);
    
    publisher.publish(RoutingEvent::new(
        "req-1".to_string(),
        "tx-1".to_string(),
        "stripe".to_string(),
        50,
    ).to_event()).unwrap();
    
    publisher.publish(CircuitBreakerEvent::new(
        "adyen".to_string(),
        "closed".to_string(),
        "half_open".to_string(),
    ).to_event()).unwrap();
    
    publisher.publish(ConfigEvent::new(3, "update").to_event()).unwrap();
    
    publisher.publish(RoutingEvent::new(
        "req-2".to_string(),
        "tx-2".to_string(),
        "checkout".to_string(),
        30,
    ).to_event()).unwrap();
    
    assert_eq!(publisher.len(), 4);
    assert_eq!(publisher.get_events_by_type(EventType::RoutingDecision).len(), 2);
    assert_eq!(publisher.get_events_by_type(EventType::CircuitBreakerStateChange).len(), 1);
    assert_eq!(publisher.get_events_by_type(EventType::ConfigurationUpdate).len(), 1);
}

#[test]
fn test_event_batch_publishing() {
    let publisher = InMemoryEventPublisher::new(100);
    
    let events: Vec<Event> = (0..10)
        .map(|i| {
            RoutingEvent::new(
                format!("req-{}", i),
                format!("tx-{}", i),
                "stripe".to_string(),
                50 + i,
            ).to_event()
        })
        .collect();
    
    publisher.publish_batch(events).unwrap();
    
    assert_eq!(publisher.len(), 10);
}

#[test]
fn test_event_max_size_eviction() {
    let publisher = InMemoryEventPublisher::new(5);
    
    for i in 0..10 {
        let event = Event::new(EventType::RoutingDecision, format!("payload-{}", i));
        publisher.publish(event).unwrap();
    }
    
    assert_eq!(publisher.len(), 5);
    
    let events = publisher.get_events();
    assert!(events[0].payload.contains("payload-5"));
    assert!(events[4].payload.contains("payload-9"));
}

#[test]
fn test_event_get_last_n() {
    let publisher = InMemoryEventPublisher::new(100);
    
    for i in 0..20 {
        let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
        publisher.publish(event).unwrap();
    }
    
    let last_5 = publisher.get_last_n(5);
    assert_eq!(last_5.len(), 5);
    assert!(last_5[0].payload.contains("event-19"));
    assert!(last_5[4].payload.contains("event-15"));
}

#[test]
fn test_event_clear() {
    let publisher = InMemoryEventPublisher::new(100);
    
    for i in 0..10 {
        let event = Event::new(EventType::RoutingDecision, format!("event-{}", i));
        publisher.publish(event).unwrap();
    }
    
    assert_eq!(publisher.len(), 10);
    
    publisher.clear();
    
    assert!(publisher.is_empty());
    assert_eq!(publisher.len(), 0);
}

#[test]
fn test_event_metadata() {
    let event = Event::new(EventType::RoutingDecision, "test".to_string())
        .with_metadata("psp_id", "stripe")
        .with_metadata("region", "us-east-1")
        .with_source("router_instance_1");
    
    assert_eq!(event.metadata.get("psp_id"), Some(&"stripe".to_string()));
    assert_eq!(event.metadata.get("region"), Some(&"us-east-1".to_string()));
    assert_eq!(event.source, "router_instance_1");
}

#[test]
fn test_concurrent_event_publishing() {
    use std::thread;
    
    let publisher = InMemoryEventPublisher::new(1000);
    let mut handles = vec![];
    
    for thread_id in 0..10 {
        let pub_clone = publisher.clone();
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let event = Event::new(
                    EventType::RoutingDecision,
                    format!("thread-{}-event-{}", thread_id, i),
                );
                pub_clone.publish(event).unwrap();
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(publisher.len(), 1000);
}

#[test]
fn test_event_type_topic_names() {
    assert_eq!(EventType::RoutingDecision.topic_name(), "transaction.routed");
    assert_eq!(EventType::CircuitBreakerStateChange.topic_name(), "circuit.state.changed");
    assert_eq!(EventType::ConfigurationUpdate.topic_name(), "config.updated");
    assert_eq!(EventType::HealthStatusChange.topic_name(), "health.status.changed");
    assert_eq!(EventType::MetricsSnapshot.topic_name(), "metrics.snapshot");
}
