use transaction_router::{BackpressureMonitor, BackpressureConfig, BackpressureStatus};
use std::time::Duration;

#[test]
fn test_basic_backpressure_monitoring() {
    let config = BackpressureConfig::new(100);
    let monitor = BackpressureMonitor::new(config);
    
    assert_eq!(monitor.status(), BackpressureStatus::Normal);
    assert!(!monitor.is_under_backpressure());
    
    for _ in 0..60 {
        monitor.enqueue();
    }
    
    assert_eq!(monitor.status(), BackpressureStatus::Warning);
    assert!(monitor.is_under_backpressure());
}

#[test]
fn test_load_shedding_trigger() {
    let config = BackpressureConfig::new(100);
    let monitor = BackpressureMonitor::new(config);
    
    assert!(!monitor.should_shed_load());
    
    for _ in 0..80 {
        monitor.enqueue();
    }
    
    assert!(monitor.should_shed_load());
}

#[test]
fn test_queue_depth_limits() {
    let config = BackpressureConfig::new(50);
    let monitor = BackpressureMonitor::new(config);
    
    for _ in 0..50 {
        assert!(monitor.try_enqueue());
    }
    
    assert!(!monitor.try_enqueue());
    assert_eq!(monitor.total_rejected(), 1);
    
    for _ in 0..10 {
        assert!(!monitor.try_enqueue());
    }
    
    assert_eq!(monitor.total_rejected(), 11);
}

#[test]
fn test_status_progression() {
    let config = BackpressureConfig::new(1000)
        .with_thresholds(0.6, 0.8, 0.95);
    let monitor = BackpressureMonitor::new(config);
    
    assert_eq!(monitor.status(), BackpressureStatus::Normal);
    
    for _ in 0..600 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Warning);
    
    for _ in 0..200 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Critical);
    
    for _ in 0..150 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Overload);
}

#[test]
fn test_utilization_calculation() {
    let config = BackpressureConfig::new(200);
    let monitor = BackpressureMonitor::new(config);
    
    for _ in 0..100 {
        monitor.enqueue();
    }
    
    assert_eq!(monitor.utilization(), 0.5);
    
    for _ in 0..50 {
        monitor.enqueue();
    }
    
    assert_eq!(monitor.utilization(), 0.75);
}

#[test]
fn test_peak_depth_tracking() {
    let monitor = BackpressureMonitor::with_default_config();
    
    for _ in 0..100 {
        monitor.enqueue();
    }
    assert_eq!(monitor.peak_depth(), 100);
    
    for _ in 0..50 {
        monitor.dequeue();
    }
    assert_eq!(monitor.peak_depth(), 100);
    assert_eq!(monitor.current_depth(), 50);
    
    for _ in 0..75 {
        monitor.enqueue();
    }
    assert_eq!(monitor.peak_depth(), 125);
}

#[test]
fn test_rejection_rate_calculation() {
    let config = BackpressureConfig::new(100);
    let monitor = BackpressureMonitor::new(config);
    
    for _ in 0..100 {
        monitor.try_enqueue();
    }
    
    for _ in 0..50 {
        monitor.try_enqueue();
    }
    
    let rejection_rate = monitor.rejection_rate();
    assert!((rejection_rate - 0.333).abs() < 0.01);
}

#[test]
fn test_throughput_measurement() {
    let monitor = BackpressureMonitor::with_default_config();
    
    for _ in 0..1000 {
        monitor.enqueue();
        monitor.dequeue();
    }
    
    let throughput = monitor.throughput(Duration::from_secs(1));
    assert_eq!(throughput, 1000.0);
    
    let throughput_10s = monitor.throughput(Duration::from_secs(10));
    assert_eq!(throughput_10s, 100.0);
}

#[test]
fn test_concurrent_enqueue_operations() {
    use std::thread;
    
    let config = BackpressureConfig::new(10000);
    let monitor = BackpressureMonitor::new(config);
    let mut handles = vec![];
    
    for _ in 0..50 {
        let monitor_clone = monitor.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                monitor_clone.enqueue();
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(monitor.current_depth(), 5000);
    assert_eq!(monitor.total_enqueued(), 5000);
}

#[test]
fn test_concurrent_try_enqueue() {
    use std::thread;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    let config = BackpressureConfig::new(1000);
    let monitor = BackpressureMonitor::new(config);
    let accepted = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    
    for _ in 0..20 {
        let monitor_clone = monitor.clone();
        let accepted_clone = accepted.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if monitor_clone.try_enqueue() {
                    accepted_clone.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(accepted.load(Ordering::SeqCst), 1000);
    assert_eq!(monitor.current_depth(), 1000);
    assert_eq!(monitor.total_rejected(), 1000);
}

#[test]
fn test_mixed_concurrent_operations() {
    use std::thread;
    
    let monitor = BackpressureMonitor::with_default_config();
    let mut handles = vec![];
    
    for _ in 0..10 {
        let monitor_clone = monitor.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                monitor_clone.enqueue();
            }
        });
        handles.push(handle);
    }
    
    for _ in 0..10 {
        let monitor_clone = monitor.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                monitor_clone.dequeue();
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(monitor.current_depth(), 0);
    assert_eq!(monitor.total_enqueued(), 1000);
    assert_eq!(monitor.total_dequeued(), 1000);
}

#[test]
fn test_reset_functionality() {
    let monitor = BackpressureMonitor::with_default_config();
    
    for _ in 0..100 {
        monitor.enqueue();
    }
    
    assert_eq!(monitor.current_depth(), 100);
    assert_eq!(monitor.total_enqueued(), 100);
    
    monitor.reset();
    
    assert_eq!(monitor.current_depth(), 0);
    assert_eq!(monitor.total_enqueued(), 0);
    assert_eq!(monitor.peak_depth(), 0);
}

#[test]
fn test_custom_thresholds() {
    let config = BackpressureConfig::new(1000)
        .with_thresholds(0.5, 0.7, 0.9);
    let monitor = BackpressureMonitor::new(config);
    
    for _ in 0..500 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Warning);
    
    for _ in 0..200 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Critical);
    
    for _ in 0..200 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Overload);
}

#[test]
fn test_high_load_scenario() {
    let config = BackpressureConfig::new(1000);
    let monitor = BackpressureMonitor::new(config);
    
    for i in 0..5000 {
        if monitor.try_enqueue() {
            if i % 5 == 0 {
                monitor.dequeue();
            }
        }
    }
    
    assert!(monitor.current_depth() <= 1000);
    assert!(monitor.total_rejected() > 0);
}

#[test]
fn test_gradual_load_increase() {
    let config = BackpressureConfig::new(1000);
    let monitor = BackpressureMonitor::new(config);
    
    for i in 0..1000 {
        monitor.enqueue();
        
        if i == 599 {
            assert_eq!(monitor.status(), BackpressureStatus::Warning);
        } else if i == 799 {
            assert_eq!(monitor.status(), BackpressureStatus::Critical);
        } else if i == 949 {
            assert_eq!(monitor.status(), BackpressureStatus::Overload);
        }
    }
}

#[test]
fn test_recovery_from_overload() {
    let config = BackpressureConfig::new(100);
    let monitor = BackpressureMonitor::new(config);
    
    for _ in 0..95 {
        monitor.enqueue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Overload);
    
    for _ in 0..15 {
        monitor.dequeue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Critical);
    
    for _ in 0..15 {
        monitor.dequeue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Warning);
    
    for _ in 0..10 {
        monitor.dequeue();
    }
    assert_eq!(monitor.status(), BackpressureStatus::Normal);
}
