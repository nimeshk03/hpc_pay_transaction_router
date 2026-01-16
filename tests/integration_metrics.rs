use std::thread;
use std::time::Duration;
use transaction_router::MetricsCollector;

#[test]
fn test_metrics_collector_basic_usage() {
    let collector = MetricsCollector::new();
    
    collector.record_latency("stripe", Duration::from_millis(45));
    collector.record_latency("stripe", Duration::from_millis(55));
    collector.record_success("stripe");
    collector.record_failure("stripe");

    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.successful_requests, 1);
    assert_eq!(stats.failed_requests, 1);
    assert_eq!(stats.success_rate, 0.5);
    assert_eq!(stats.avg_latency_ms, 50.0);
}

#[test]
fn test_metrics_collector_percentiles() {
    let collector = MetricsCollector::new();
    
    for i in 1..=100 {
        collector.record_latency("stripe", Duration::from_millis(i));
    }

    let stats = collector.get_stats("stripe").unwrap();
    assert!(stats.p50_latency_ms >= 49 && stats.p50_latency_ms <= 51);
    assert!(stats.p95_latency_ms >= 94 && stats.p95_latency_ms <= 96);
    assert!(stats.p99_latency_ms >= 98 && stats.p99_latency_ms <= 100);
}

#[test]
fn test_metrics_collector_rolling_window() {
    let collector = MetricsCollector::with_config(100, 0.2);
    
    for i in 0..200 {
        collector.record_request(
            "stripe",
            i % 2 == 0,
            Duration::from_millis(10 + i),
        );
    }

    assert_eq!(collector.total_samples("stripe"), 100);
    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.total_requests, 100);
}

#[test]
fn test_metrics_collector_multiple_routes() {
    let collector = MetricsCollector::new();
    
    for _ in 0..70 {
        collector.record_success("stripe");
    }
    for _ in 0..30 {
        collector.record_failure("stripe");
    }
    
    for _ in 0..90 {
        collector.record_success("adyen");
    }
    for _ in 0..10 {
        collector.record_failure("adyen");
    }

    let stripe_stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stripe_stats.success_rate, 0.7);
    
    let adyen_stats = collector.get_stats("adyen").unwrap();
    assert_eq!(adyen_stats.success_rate, 0.9);
    
    let all_stats = collector.get_all_stats();
    assert_eq!(all_stats.len(), 2);
}

#[test]
fn test_metrics_collector_exponential_moving_average() {
    let collector = MetricsCollector::with_config(1000, 0.2);
    
    for _ in 0..10 {
        collector.record_latency("stripe", Duration::from_millis(100));
    }
    
    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.ema_latency_ms, 100.0);
    
    collector.record_latency("stripe", Duration::from_millis(200));
    let stats = collector.get_stats("stripe").unwrap();
    assert!(stats.ema_latency_ms > 100.0 && stats.ema_latency_ms < 200.0);
}

#[test]
fn test_metrics_collector_concurrent_updates() {
    let collector = MetricsCollector::new();
    let mut handles = vec![];

    for i in 0..10 {
        let collector_clone = collector.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                collector_clone.record_request(
                    "stripe",
                    i % 2 == 0,
                    Duration::from_millis(50 + i),
                );
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.total_requests, 1000);
}

#[test]
fn test_metrics_collector_reset() {
    let collector = MetricsCollector::new();
    
    collector.record_success("stripe");
    collector.record_success("adyen");
    assert!(collector.get_stats("stripe").is_some());
    assert!(collector.get_stats("adyen").is_some());
    
    collector.reset("stripe");
    assert!(collector.get_stats("stripe").is_none());
    assert!(collector.get_stats("adyen").is_some());
    
    collector.reset_all();
    assert!(collector.get_stats("stripe").is_none());
    assert!(collector.get_stats("adyen").is_none());
}

#[test]
fn test_metrics_collector_min_max_tracking() {
    let collector = MetricsCollector::new();
    
    collector.record_latency("stripe", Duration::from_millis(10));
    collector.record_latency("stripe", Duration::from_millis(100));
    collector.record_latency("stripe", Duration::from_millis(50));
    collector.record_latency("stripe", Duration::from_millis(75));

    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.min_latency_ms, 10);
    assert_eq!(stats.max_latency_ms, 100);
}

#[test]
fn test_metrics_collector_high_volume() {
    let collector = MetricsCollector::with_config(10000, 0.1);
    
    for i in 1..=5000 {
        collector.record_request(
            "stripe",
            i % 10 != 0,
            Duration::from_millis(i % 200),
        );
    }

    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.total_requests, 5000);
    assert!(stats.success_rate > 0.85 && stats.success_rate < 0.95);
}

#[test]
fn test_metrics_collector_edge_cases() {
    let collector = MetricsCollector::new();
    
    assert!(collector.get_stats("nonexistent").is_none());
    assert_eq!(collector.total_samples("nonexistent"), 0);
    
    collector.record_success("stripe");
    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.p50_latency_ms, 0);
    assert_eq!(stats.avg_latency_ms, 0.0);
}

#[test]
fn test_metrics_collector_window_size_boundary() {
    let collector = MetricsCollector::with_config(10, 0.2);
    
    for i in 0..15 {
        collector.record_latency("stripe", Duration::from_millis(i * 10));
    }

    assert_eq!(collector.total_samples("stripe"), 10);
    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.min_latency_ms, 50);
    assert_eq!(stats.max_latency_ms, 140);
}

#[test]
fn test_metrics_collector_combined_operations() {
    let collector = MetricsCollector::new();
    
    for i in 0..50 {
        let success = i % 3 != 0;
        let latency = Duration::from_millis(20 + (i % 80));
        collector.record_request("stripe", success, latency);
    }

    let stats = collector.get_stats("stripe").unwrap();
    assert_eq!(stats.total_requests, 50);
    assert!(stats.success_rate > 0.6 && stats.success_rate < 0.7);
    assert!(stats.avg_latency_ms > 0.0);
    assert!(stats.p50_latency_ms > 0);
}
