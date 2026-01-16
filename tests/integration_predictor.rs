use std::time::Duration;
use transaction_router::{LatencyPredictor, LatencyPredictorConfig};

#[test]
fn test_predictor_basic_usage() {
    let predictor = LatencyPredictor::with_default_config();
    
    predictor.update("stripe", Duration::from_millis(100));
    let prediction = predictor.predict("stripe").unwrap();
    assert_eq!(prediction.as_millis(), 100);
}

#[test]
fn test_ema_prediction() {
    let config = LatencyPredictorConfig::new(0.3);
    let predictor = LatencyPredictor::new(config);
    
    predictor.update("stripe", Duration::from_millis(100));
    predictor.update("stripe", Duration::from_millis(120));
    
    let predicted = predictor.predict("stripe").unwrap();
    assert!(predicted.as_millis() > 100 && predicted.as_millis() < 120);
}

#[test]
fn test_ema_smoothing() {
    let config = LatencyPredictorConfig::new(0.2);
    let predictor = LatencyPredictor::new(config);
    
    for _ in 0..100 {
        predictor.update("stripe", Duration::from_millis(50));
    }
    
    let predicted = predictor.predict("stripe").unwrap();
    assert_eq!(predicted.as_millis(), 50);
}

#[test]
fn test_anomaly_detection_spike() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..30 {
        let latency = 50 + (i % 5);
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(predictor.is_anomaly("stripe", Duration::from_millis(500)));
}

#[test]
fn test_no_anomaly_normal_variance() {
    let config = LatencyPredictorConfig::new(0.3)
        .with_anomaly_threshold(5.0);
    let predictor = LatencyPredictor::new(config);
    
    for i in 0..30 {
        let latency = 50 + (i % 10);
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(!predictor.is_anomaly("stripe", Duration::from_millis(60)));
}

#[test]
fn test_degrading_performance() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..30 {
        let latency = if i < 15 { 50 } else { 100 + (i - 15) * 5 };
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(predictor.is_degrading("stripe"));
}

#[test]
fn test_stable_performance() {
    let predictor = LatencyPredictor::with_default_config();
    
    for _ in 0..30 {
        predictor.update("stripe", Duration::from_millis(50));
    }
    
    assert!(!predictor.is_degrading("stripe"));
}

#[test]
fn test_improving_performance() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..30 {
        let latency = 100 - i * 2;
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(!predictor.is_degrading("stripe"));
}

#[test]
fn test_trend_positive() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..20 {
        let latency = 50 + i * 2;
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    let trend = predictor.get_trend("stripe");
    assert!(trend > 0.0);
}

#[test]
fn test_trend_negative() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..20 {
        let latency = 100 - i * 2;
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    let trend = predictor.get_trend("stripe");
    assert!(trend < 0.0);
}

#[test]
fn test_trend_stable() {
    let predictor = LatencyPredictor::with_default_config();
    
    for _ in 0..20 {
        predictor.update("stripe", Duration::from_millis(50));
    }
    
    let trend = predictor.get_trend("stripe");
    assert_eq!(trend, 0.0);
}

#[test]
fn test_multiple_routes_prediction() {
    let predictor = LatencyPredictor::with_default_config();
    
    for _ in 0..10 {
        predictor.update("stripe", Duration::from_millis(100));
        predictor.update("adyen", Duration::from_millis(50));
    }
    
    let stripe_pred = predictor.predict("stripe").unwrap();
    let adyen_pred = predictor.predict("adyen").unwrap();
    
    assert_eq!(stripe_pred.as_millis(), 100);
    assert_eq!(adyen_pred.as_millis(), 50);
}

#[test]
fn test_std_dev_calculation() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..20 {
        let latency = 50 + (i % 10) * 5;
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    let std_dev = predictor.get_std_dev("stripe");
    assert!(std_dev > 0.0);
}

#[test]
fn test_sample_count_tracking() {
    let predictor = LatencyPredictor::with_default_config();
    
    for _ in 0..50 {
        predictor.update("stripe", Duration::from_millis(100));
    }
    
    assert_eq!(predictor.get_sample_count("stripe"), 50);
}

#[test]
fn test_last_latency_tracking() {
    let predictor = LatencyPredictor::with_default_config();
    
    predictor.update("stripe", Duration::from_millis(100));
    predictor.update("stripe", Duration::from_millis(150));
    predictor.update("stripe", Duration::from_millis(200));
    
    let last = predictor.get_last_latency("stripe").unwrap();
    assert_eq!(last.as_millis(), 200);
}

#[test]
fn test_all_predictions() {
    let predictor = LatencyPredictor::with_default_config();
    
    predictor.update("stripe", Duration::from_millis(100));
    predictor.update("adyen", Duration::from_millis(50));
    predictor.update("paypal", Duration::from_millis(75));
    
    let all_preds = predictor.get_all_predictions();
    assert_eq!(all_preds.len(), 3);
    assert!(all_preds.contains_key("stripe"));
    assert!(all_preds.contains_key("adyen"));
    assert!(all_preds.contains_key("paypal"));
}

#[test]
fn test_reset_route() {
    let predictor = LatencyPredictor::with_default_config();
    
    predictor.update("stripe", Duration::from_millis(100));
    predictor.update("adyen", Duration::from_millis(50));
    
    predictor.reset("stripe");
    
    assert!(predictor.predict("stripe").is_none());
    assert!(predictor.predict("adyen").is_some());
}

#[test]
fn test_reset_all() {
    let predictor = LatencyPredictor::with_default_config();
    
    predictor.update("stripe", Duration::from_millis(100));
    predictor.update("adyen", Duration::from_millis(50));
    
    predictor.reset_all();
    
    assert!(predictor.predict("stripe").is_none());
    assert!(predictor.predict("adyen").is_none());
}

#[test]
fn test_custom_config() {
    let config = LatencyPredictorConfig::new(0.5)
        .with_anomaly_threshold(2.0)
        .with_degradation_config(30, 2.0);
    
    let predictor = LatencyPredictor::new(config);
    
    for _ in 0..20 {
        predictor.update("stripe", Duration::from_millis(100));
    }
    
    let predicted = predictor.predict("stripe").unwrap();
    assert_eq!(predicted.as_millis(), 100);
}

#[test]
fn test_high_volume_updates() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..1000 {
        let latency = 50 + (i % 50);
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert_eq!(predictor.get_sample_count("stripe"), 1000);
    let predicted = predictor.predict("stripe").unwrap();
    assert!(predicted.as_millis() > 0);
}

#[test]
fn test_concurrent_predictions() {
    use std::thread;
    
    let predictor = LatencyPredictor::with_default_config();
    let mut handles = vec![];
    
    for i in 0..10 {
        let predictor_clone = predictor.clone();
        let handle = thread::spawn(move || {
            for _ in 0..50 {
                predictor_clone.update("stripe", Duration::from_millis(100 + i * 5));
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(predictor.get_sample_count("stripe"), 500);
}

#[test]
fn test_gradual_degradation() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..40 {
        let latency = if i < 20 { 50 } else { 50 + (i - 20) * 5 };
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(predictor.is_degrading("stripe"));
}

#[test]
fn test_recovery_from_degradation() {
    let predictor = LatencyPredictor::with_default_config();
    
    for i in 0..40 {
        let latency = if i < 20 { 100 } else { 50 };
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(!predictor.is_degrading("stripe"));
}

#[test]
fn test_anomaly_with_custom_threshold() {
    let config = LatencyPredictorConfig::new(0.3)
        .with_anomaly_threshold(2.0);
    
    let predictor = LatencyPredictor::new(config);
    
    for i in 0..30 {
        let latency = 50 + (i % 10);
        predictor.update("stripe", Duration::from_millis(latency));
    }
    
    assert!(predictor.is_anomaly("stripe", Duration::from_millis(300)));
}

#[test]
fn test_no_prediction_for_unknown_route() {
    let predictor = LatencyPredictor::with_default_config();
    
    assert!(predictor.predict("nonexistent").is_none());
    assert_eq!(predictor.get_sample_count("nonexistent"), 0);
    assert!(predictor.get_last_latency("nonexistent").is_none());
}
