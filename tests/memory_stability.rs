use transaction_router::{
    PriorityQueue, BackpressureMonitor, BackpressureConfig, MetricsCollector,
    HealthChecker, LatencyPredictor,
};
use transaction_router::queue::priority_queue::Priority;
use std::time::Duration;

#[test]
fn test_priority_queue_memory_stability() {
    let queue = PriorityQueue::with_capacity(100000);
    
    // Fill and drain multiple times
    for _ in 0..10 {
        for i in 0..10000 {
            queue.enqueue(i, Priority::Normal);
        }
        
        for _ in 0..10000 {
            queue.dequeue();
        }
    }
    
    assert!(queue.is_empty());
}

#[test]
fn test_backpressure_monitor_memory_stability() {
    let config = BackpressureConfig::new(100000);
    let monitor = BackpressureMonitor::new(config);
    
    // Simulate high load cycles
    for _ in 0..10 {
        for _ in 0..10000 {
            monitor.enqueue();
        }
        
        for _ in 0..10000 {
            monitor.dequeue();
        }
    }
    
    assert_eq!(monitor.current_depth(), 0);
}

#[test]
fn test_metrics_collector_memory_stability() {
    let collector = MetricsCollector::new();
    
    // Record many metrics
    for cycle in 0..10 {
        for i in 0..1000 {
            let route = format!("route_{}", i % 10);
            collector.record_request(&route, true, Duration::from_millis(50));
        }
        
        // Reset some routes periodically to simulate real usage
        if cycle % 3 == 0 {
            for i in 0..5 {
                collector.reset(&format!("route_{}", i));
            }
        }
    }
    
    // Verify metrics are still accessible for non-reset routes
    let stats = collector.get_stats("route_9");
    assert!(stats.is_some());
}

#[test]
fn test_health_checker_memory_stability() {
    let checker = HealthChecker::with_default_config();
    
    // Record many probes
    for _ in 0..10 {
        for i in 0..1000 {
            let route = format!("route_{}", i % 10);
            checker.record_probe_success(&route, 100);
        }
    }
    
    // Verify all routes are tracked
    let statuses = checker.get_all_statuses();
    assert_eq!(statuses.len(), 10);
}

#[test]
fn test_predictor_memory_stability() {
    let predictor = LatencyPredictor::with_default_config();
    
    // Update many predictions
    for _ in 0..10 {
        for i in 0..1000 {
            let route = format!("route_{}", i % 10);
            predictor.update(&route, Duration::from_millis(50 + i % 100));
        }
    }
    
    // Verify predictions are available
    let predictions = predictor.get_all_predictions();
    assert_eq!(predictions.len(), 10);
}

#[test]
fn test_combined_components_memory_stability() {
    let queue = PriorityQueue::new();
    let monitor = BackpressureMonitor::new(BackpressureConfig::new(10000));
    let collector = MetricsCollector::new();
    let checker = HealthChecker::with_default_config();
    let predictor = LatencyPredictor::with_default_config();
    
    // Simulate realistic workload
    for cycle in 0..100 {
        for i in 0..100 {
            let route = format!("route_{}", i % 5);
            
            if monitor.try_enqueue() {
                queue.enqueue(i, Priority::Normal);
                collector.record_latency(&route, Duration::from_millis(50));
                checker.record_probe_success(&route, 50);
                predictor.update(&route, Duration::from_millis(50));
            }
            
            if let Some(_item) = queue.dequeue() {
                monitor.dequeue();
            }
        }
        
        // Periodic cleanup
        if cycle % 10 == 0 {
            queue.clear();
            monitor.reset();
        }
    }
    
    // Verify system is still operational
    assert_eq!(monitor.current_depth(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_high_concurrency_memory_stability() {
    use std::thread;
    
    let queue = PriorityQueue::new();
    let monitor = BackpressureMonitor::new(BackpressureConfig::new(50000));
    let collector = MetricsCollector::new();
    
    let mut handles = vec![];
    
    // Spawn multiple threads
    for thread_id in 0..10 {
        let queue_clone = queue.clone();
        let monitor_clone = monitor.clone();
        let collector_clone = collector.clone();
        
        let handle = thread::spawn(move || {
            for i in 0..1000 {
                let route = format!("route_{}", thread_id);
                
                if monitor_clone.try_enqueue() {
                    queue_clone.enqueue(i, Priority::Normal);
                    collector_clone.record_latency(&route, Duration::from_millis(50));
                }
                
                if i % 2 == 0 {
                    if queue_clone.dequeue().is_some() {
                        monitor_clone.dequeue();
                    }
                }
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify no memory corruption
    assert!(monitor.current_depth() <= 50000);
}

#[test]
fn test_long_running_stability() {
    let collector = MetricsCollector::new();
    let checker = HealthChecker::with_default_config();
    
    // Simulate long-running service
    for iteration in 0..1000 {
        collector.record_request("stripe", iteration % 2 == 0, Duration::from_millis(50));
        checker.record_probe_success("stripe", 50);
        
        // Periodic stats retrieval
        if iteration % 100 == 0 {
            let _ = collector.get_stats("stripe");
            let _ = checker.get_status("stripe");
        }
    }
    
    // Verify metrics are consistent
    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.total_requests, 1000);
}
