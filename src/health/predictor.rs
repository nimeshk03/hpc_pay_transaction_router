use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LatencyPredictorConfig {
    pub ema_alpha: f64,
    pub anomaly_threshold_multiplier: f64,
    pub degradation_window: usize,
    pub degradation_threshold: f64,
}

impl Default for LatencyPredictorConfig {
    fn default() -> Self {
        Self {
            ema_alpha: 0.3,
            anomaly_threshold_multiplier: 3.0,
            degradation_window: 20,
            degradation_threshold: 1.5,
        }
    }
}

impl LatencyPredictorConfig {
    pub fn new(ema_alpha: f64) -> Self {
        Self {
            ema_alpha,
            ..Default::default()
        }
    }

    pub fn with_anomaly_threshold(mut self, multiplier: f64) -> Self {
        self.anomaly_threshold_multiplier = multiplier;
        self
    }

    pub fn with_degradation_config(mut self, window: usize, threshold: f64) -> Self {
        self.degradation_window = window;
        self.degradation_threshold = threshold;
        self
    }
}

#[derive(Debug)]
struct RouteLatencyData {
    ema_ms: f64,
    variance: f64,
    recent_latencies: VecDeque<f64>,
    sample_count: u64,
    last_latency_ms: f64,
}

impl RouteLatencyData {
    fn new(window_size: usize) -> Self {
        Self {
            ema_ms: 0.0,
            variance: 0.0,
            recent_latencies: VecDeque::with_capacity(window_size),
            sample_count: 0,
            last_latency_ms: 0.0,
        }
    }

    fn update(&mut self, latency_ms: f64, alpha: f64, window_size: usize) {
        self.last_latency_ms = latency_ms;
        self.sample_count += 1;

        if self.sample_count == 1 {
            self.ema_ms = latency_ms;
            self.variance = 0.0;
        } else {
            let delta = latency_ms - self.ema_ms;
            self.ema_ms += alpha * delta;
            self.variance = (1.0 - alpha) * (self.variance + alpha * delta * delta);
        }

        if self.recent_latencies.len() >= window_size {
            self.recent_latencies.pop_front();
        }
        self.recent_latencies.push_back(latency_ms);
    }

    fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    fn is_anomaly(&self, latency_ms: f64, threshold_multiplier: f64) -> bool {
        if self.sample_count < 10 {
            return false;
        }

        let std_dev = self.std_dev();
        let threshold = self.ema_ms + threshold_multiplier * std_dev;
        latency_ms > threshold
    }

    fn is_degrading(&self, threshold: f64) -> bool {
        if self.recent_latencies.len() < 10 {
            return false;
        }

        let mid_point = self.recent_latencies.len() / 2;
        let first_half: Vec<f64> = self.recent_latencies.iter().take(mid_point).copied().collect();
        let second_half: Vec<f64> = self.recent_latencies.iter().skip(mid_point).copied().collect();

        if first_half.is_empty() || second_half.is_empty() {
            return false;
        }

        let first_avg: f64 = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_avg: f64 = second_half.iter().sum::<f64>() / second_half.len() as f64;

        if first_avg == 0.0 {
            return false;
        }

        second_avg / first_avg > threshold
    }

    fn trend(&self) -> f64 {
        if self.recent_latencies.len() < 5 {
            return 0.0;
        }

        let mid_point = self.recent_latencies.len() / 2;
        let first_half: Vec<f64> = self.recent_latencies.iter().take(mid_point).copied().collect();
        let second_half: Vec<f64> = self.recent_latencies.iter().skip(mid_point).copied().collect();

        if first_half.is_empty() || second_half.is_empty() {
            return 0.0;
        }

        let first_avg: f64 = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_avg: f64 = second_half.iter().sum::<f64>() / second_half.len() as f64;

        if first_avg == 0.0 {
            return 0.0;
        }

        (second_avg - first_avg) / first_avg
    }
}

#[derive(Debug, Clone)]
pub struct LatencyPredictor {
    routes: Arc<Mutex<HashMap<String, RouteLatencyData>>>,
    config: LatencyPredictorConfig,
}

impl LatencyPredictor {
    pub fn new(config: LatencyPredictorConfig) -> Self {
        Self {
            routes: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(LatencyPredictorConfig::default())
    }

    fn get_or_create_route_data(&self, route_id: &str) {
        let mut routes = self.routes.lock().unwrap();
        if !routes.contains_key(route_id) {
            routes.insert(
                route_id.to_string(),
                RouteLatencyData::new(self.config.degradation_window),
            );
        }
    }

    pub fn update(&self, route_id: &str, latency: Duration) {
        self.get_or_create_route_data(route_id);
        let mut routes = self.routes.lock().unwrap();

        if let Some(data) = routes.get_mut(route_id) {
            let latency_ms = latency.as_millis() as f64;
            data.update(
                latency_ms,
                self.config.ema_alpha,
                self.config.degradation_window,
            );
        }
    }

    pub fn predict(&self, route_id: &str) -> Option<Duration> {
        let routes = self.routes.lock().unwrap();
        routes.get(route_id).map(|data| {
            if data.sample_count == 0 {
                Duration::from_millis(0)
            } else {
                Duration::from_millis(data.ema_ms.round() as u64)
            }
        })
    }

    pub fn is_anomaly(&self, route_id: &str, latency: Duration) -> bool {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| {
                data.is_anomaly(
                    latency.as_millis() as f64,
                    self.config.anomaly_threshold_multiplier,
                )
            })
            .unwrap_or(false)
    }

    pub fn is_degrading(&self, route_id: &str) -> bool {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.is_degrading(self.config.degradation_threshold))
            .unwrap_or(false)
    }

    pub fn get_trend(&self, route_id: &str) -> f64 {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.trend())
            .unwrap_or(0.0)
    }

    pub fn get_std_dev(&self, route_id: &str) -> f64 {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.std_dev())
            .unwrap_or(0.0)
    }

    pub fn get_sample_count(&self, route_id: &str) -> u64 {
        let routes = self.routes.lock().unwrap();
        routes
            .get(route_id)
            .map(|data| data.sample_count)
            .unwrap_or(0)
    }

    pub fn get_last_latency(&self, route_id: &str) -> Option<Duration> {
        let routes = self.routes.lock().unwrap();
        routes.get(route_id).map(|data| {
            Duration::from_millis(data.last_latency_ms.round() as u64)
        })
    }

    pub fn get_all_predictions(&self) -> HashMap<String, Duration> {
        let routes = self.routes.lock().unwrap();
        routes
            .iter()
            .map(|(id, data)| {
                (
                    id.clone(),
                    Duration::from_millis(data.ema_ms.round() as u64),
                )
            })
            .collect()
    }

    pub fn reset(&self, route_id: &str) {
        let mut routes = self.routes.lock().unwrap();
        routes.remove(route_id);
    }

    pub fn reset_all(&self) {
        let mut routes = self.routes.lock().unwrap();
        routes.clear();
    }

    pub fn get_config(&self) -> LatencyPredictorConfig {
        self.config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictor_creation() {
        let predictor = LatencyPredictor::with_default_config();
        assert!(predictor.predict("stripe").is_none());
    }

    #[test]
    fn test_ema_calculation() {
        let config = LatencyPredictorConfig::new(0.3);
        let predictor = LatencyPredictor::new(config);

        predictor.update("stripe", Duration::from_millis(100));
        let pred1 = predictor.predict("stripe").unwrap();
        assert_eq!(pred1.as_millis(), 100);

        predictor.update("stripe", Duration::from_millis(120));
        let pred2 = predictor.predict("stripe").unwrap();
        assert!(pred2.as_millis() > 100 && pred2.as_millis() < 120);
    }

    #[test]
    fn test_ema_convergence() {
        let config = LatencyPredictorConfig::new(0.2);
        let predictor = LatencyPredictor::new(config);

        for _ in 0..50 {
            predictor.update("stripe", Duration::from_millis(100));
        }

        let predicted = predictor.predict("stripe").unwrap();
        assert_eq!(predicted.as_millis(), 100);
    }

    #[test]
    fn test_anomaly_detection() {
        let predictor = LatencyPredictor::with_default_config();

        for i in 0..20 {
            let latency = 50 + (i % 10);
            predictor.update("stripe", Duration::from_millis(latency));
        }

        assert!(!predictor.is_anomaly("stripe", Duration::from_millis(60)));
        assert!(predictor.is_anomaly("stripe", Duration::from_millis(500)));
    }

    #[test]
    fn test_degradation_detection() {
        let predictor = LatencyPredictor::with_default_config();

        for i in 0..20 {
            let latency = 50 + i * 5;
            predictor.update("stripe", Duration::from_millis(latency));
        }

        assert!(predictor.is_degrading("stripe"));
    }

    #[test]
    fn test_no_degradation_stable() {
        let predictor = LatencyPredictor::with_default_config();

        for _ in 0..20 {
            predictor.update("stripe", Duration::from_millis(50));
        }

        assert!(!predictor.is_degrading("stripe"));
    }

    #[test]
    fn test_trend_calculation() {
        let predictor = LatencyPredictor::with_default_config();

        for i in 0..20 {
            let latency = 50 + i * 2;
            predictor.update("stripe", Duration::from_millis(latency));
        }

        let trend = predictor.get_trend("stripe");
        assert!(trend > 0.0);
    }

    #[test]
    fn test_std_dev_calculation() {
        let predictor = LatencyPredictor::with_default_config();

        predictor.update("stripe", Duration::from_millis(50));
        predictor.update("stripe", Duration::from_millis(100));
        predictor.update("stripe", Duration::from_millis(150));

        let std_dev = predictor.get_std_dev("stripe");
        assert!(std_dev > 0.0);
    }

    #[test]
    fn test_sample_count() {
        let predictor = LatencyPredictor::with_default_config();

        for _ in 0..10 {
            predictor.update("stripe", Duration::from_millis(50));
        }

        assert_eq!(predictor.get_sample_count("stripe"), 10);
    }

    #[test]
    fn test_last_latency() {
        let predictor = LatencyPredictor::with_default_config();

        predictor.update("stripe", Duration::from_millis(100));
        predictor.update("stripe", Duration::from_millis(150));

        let last = predictor.get_last_latency("stripe").unwrap();
        assert_eq!(last.as_millis(), 150);
    }

    #[test]
    fn test_multiple_routes() {
        let predictor = LatencyPredictor::with_default_config();

        predictor.update("stripe", Duration::from_millis(100));
        predictor.update("adyen", Duration::from_millis(50));

        let stripe_pred = predictor.predict("stripe").unwrap();
        let adyen_pred = predictor.predict("adyen").unwrap();

        assert_eq!(stripe_pred.as_millis(), 100);
        assert_eq!(adyen_pred.as_millis(), 50);
    }

    #[test]
    fn test_get_all_predictions() {
        let predictor = LatencyPredictor::with_default_config();

        predictor.update("stripe", Duration::from_millis(100));
        predictor.update("adyen", Duration::from_millis(50));

        let all_preds = predictor.get_all_predictions();
        assert_eq!(all_preds.len(), 2);
        assert!(all_preds.contains_key("stripe"));
        assert!(all_preds.contains_key("adyen"));
    }

    #[test]
    fn test_reset_route() {
        let predictor = LatencyPredictor::with_default_config();

        predictor.update("stripe", Duration::from_millis(100));
        assert!(predictor.predict("stripe").is_some());

        predictor.reset("stripe");
        assert!(predictor.predict("stripe").is_none());
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
    fn test_insufficient_data_no_anomaly() {
        let predictor = LatencyPredictor::with_default_config();

        for _ in 0..5 {
            predictor.update("stripe", Duration::from_millis(50));
        }

        assert!(!predictor.is_anomaly("stripe", Duration::from_millis(500)));
    }

    #[test]
    fn test_insufficient_data_no_degradation() {
        let predictor = LatencyPredictor::with_default_config();

        for _ in 0..5 {
            predictor.update("stripe", Duration::from_millis(50));
        }

        assert!(!predictor.is_degrading("stripe"));
    }

    #[test]
    fn test_concurrent_updates() {
        use std::thread;

        let predictor = LatencyPredictor::with_default_config();
        let predictor_clone = predictor.clone();

        let handle = thread::spawn(move || {
            for _ in 0..50 {
                predictor_clone.update("stripe", Duration::from_millis(100));
            }
        });

        for _ in 0..50 {
            predictor.update("stripe", Duration::from_millis(100));
        }

        handle.join().unwrap();

        assert_eq!(predictor.get_sample_count("stripe"), 100);
    }

    #[test]
    fn test_custom_config() {
        let config = LatencyPredictorConfig::new(0.5)
            .with_anomaly_threshold(2.0)
            .with_degradation_config(30, 2.0);

        let predictor = LatencyPredictor::new(config);

        predictor.update("stripe", Duration::from_millis(100));
        assert!(predictor.predict("stripe").is_some());
    }

    #[test]
    fn test_unknown_route() {
        let predictor = LatencyPredictor::with_default_config();

        assert!(predictor.predict("nonexistent").is_none());
        assert!(!predictor.is_anomaly("nonexistent", Duration::from_millis(100)));
        assert!(!predictor.is_degrading("nonexistent"));
        assert_eq!(predictor.get_trend("nonexistent"), 0.0);
        assert_eq!(predictor.get_std_dev("nonexistent"), 0.0);
    }
}
