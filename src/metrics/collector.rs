use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RouteStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub p99_latency_ms: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub ema_latency_ms: f64,
}

impl Default for RouteStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            success_rate: 0.0,
            avg_latency_ms: 0.0,
            p50_latency_ms: 0,
            p95_latency_ms: 0,
            p99_latency_ms: 0,
            min_latency_ms: 0,
            max_latency_ms: 0,
            ema_latency_ms: 0.0,
        }
    }
}

#[derive(Debug)]
struct RouteMetricsData {
    latencies: VecDeque<u64>,
    successes: VecDeque<bool>,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    ema_latency: f64,
    ema_alpha: f64,
    window_size: usize,
}

impl RouteMetricsData {
    fn new(window_size: usize, ema_alpha: f64) -> Self {
        Self {
            latencies: VecDeque::with_capacity(window_size),
            successes: VecDeque::with_capacity(window_size),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            ema_latency: 0.0,
            ema_alpha,
            window_size,
        }
    }

    fn record_latency(&mut self, latency_ms: u64) {
        if self.latencies.len() >= self.window_size {
            self.latencies.pop_front();
        }
        self.latencies.push_back(latency_ms);

        if self.ema_latency == 0.0 {
            self.ema_latency = latency_ms as f64;
        } else {
            self.ema_latency = self.ema_alpha * latency_ms as f64
                + (1.0 - self.ema_alpha) * self.ema_latency;
        }
    }

    fn record_success(&mut self, success: bool) {
        if self.successes.len() >= self.window_size {
            let old = self.successes.pop_front().unwrap();
            if old {
                self.successful_requests = self.successful_requests.saturating_sub(1);
            } else {
                self.failed_requests = self.failed_requests.saturating_sub(1);
            }
            self.total_requests = self.total_requests.saturating_sub(1);
        }

        self.successes.push_back(success);
        self.total_requests += 1;
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }
    }

    fn calculate_percentile(&self, percentile: f64) -> u64 {
        if self.latencies.is_empty() {
            return 0;
        }

        let mut sorted: Vec<u64> = self.latencies.iter().copied().collect();
        sorted.sort_unstable();

        let rank = (percentile / 100.0) * (sorted.len() - 1) as f64;
        let lower_index = rank.floor() as usize;
        let upper_index = rank.ceil() as usize;
        
        if lower_index == upper_index {
            sorted[lower_index]
        } else {
            let lower_value = sorted[lower_index] as f64;
            let upper_value = sorted[upper_index] as f64;
            let fraction = rank - lower_index as f64;
            (lower_value + fraction * (upper_value - lower_value)).round() as u64
        }
    }

    fn get_stats(&self) -> RouteStats {
        let success_rate = if self.total_requests > 0 {
            self.successful_requests as f64 / self.total_requests as f64
        } else {
            0.0
        };

        let avg_latency_ms = if !self.latencies.is_empty() {
            self.latencies.iter().sum::<u64>() as f64 / self.latencies.len() as f64
        } else {
            0.0
        };

        let min_latency_ms = self.latencies.iter().min().copied().unwrap_or(0);
        let max_latency_ms = self.latencies.iter().max().copied().unwrap_or(0);

        RouteStats {
            total_requests: self.total_requests,
            successful_requests: self.successful_requests,
            failed_requests: self.failed_requests,
            success_rate,
            avg_latency_ms,
            p50_latency_ms: self.calculate_percentile(50.0),
            p95_latency_ms: self.calculate_percentile(95.0),
            p99_latency_ms: self.calculate_percentile(99.0),
            min_latency_ms,
            max_latency_ms,
            ema_latency_ms: self.ema_latency,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteMetrics {
    data: Arc<Mutex<RouteMetricsData>>,
}

impl RouteMetrics {
    pub fn new(window_size: usize, ema_alpha: f64) -> Self {
        Self {
            data: Arc::new(Mutex::new(RouteMetricsData::new(window_size, ema_alpha))),
        }
    }

    pub fn record_latency(&self, latency: Duration) {
        let mut data = self.data.lock().unwrap();
        data.record_latency(latency.as_millis() as u64);
    }

    pub fn record_success(&self) {
        let mut data = self.data.lock().unwrap();
        data.record_success(true);
    }

    pub fn record_failure(&self) {
        let mut data = self.data.lock().unwrap();
        data.record_success(false);
    }

    pub fn record_request(&self, success: bool, latency: Duration) {
        let mut data = self.data.lock().unwrap();
        data.record_latency(latency.as_millis() as u64);
        data.record_success(success);
    }

    pub fn get_stats(&self) -> RouteStats {
        let data = self.data.lock().unwrap();
        data.get_stats()
    }

    pub fn total_samples(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.latencies.len()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    routes: Arc<Mutex<HashMap<String, RouteMetrics>>>,
    window_size: usize,
    ema_alpha: f64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::with_config(1000, 0.2)
    }

    pub fn with_config(window_size: usize, ema_alpha: f64) -> Self {
        Self {
            routes: Arc::new(Mutex::new(HashMap::new())),
            window_size,
            ema_alpha,
        }
    }

    fn get_or_create_metrics(&self, route_id: &str) -> RouteMetrics {
        let mut routes = self.routes.lock().unwrap();
        routes
            .entry(route_id.to_string())
            .or_insert_with(|| RouteMetrics::new(self.window_size, self.ema_alpha))
            .clone()
    }

    pub fn record_latency(&self, route_id: &str, latency: Duration) {
        let metrics = self.get_or_create_metrics(route_id);
        metrics.record_latency(latency);
    }

    pub fn record_success(&self, route_id: &str) {
        let metrics = self.get_or_create_metrics(route_id);
        metrics.record_success();
    }

    pub fn record_failure(&self, route_id: &str) {
        let metrics = self.get_or_create_metrics(route_id);
        metrics.record_failure();
    }

    pub fn record_request(&self, route_id: &str, success: bool, latency: Duration) {
        let metrics = self.get_or_create_metrics(route_id);
        metrics.record_request(success, latency);
    }

    pub fn get_stats(&self, route_id: &str) -> Option<RouteStats> {
        let routes = self.routes.lock().unwrap();
        routes.get(route_id).map(|m| m.get_stats())
    }

    pub fn get_all_stats(&self) -> HashMap<String, RouteStats> {
        let routes = self.routes.lock().unwrap();
        routes
            .iter()
            .map(|(id, metrics)| (id.clone(), metrics.get_stats()))
            .collect()
    }

    pub fn total_samples(&self, route_id: &str) -> usize {
        let routes = self.routes.lock().unwrap();
        routes.get(route_id).map(|m| m.total_samples()).unwrap_or(0)
    }

    pub fn reset(&self, route_id: &str) {
        let mut routes = self.routes.lock().unwrap();
        routes.remove(route_id);
    }

    pub fn reset_all(&self) {
        let mut routes = self.routes.lock().unwrap();
        routes.clear();
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(collector.get_stats("stripe").is_none());
    }

    #[test]
    fn test_record_latency() {
        let collector = MetricsCollector::new();
        collector.record_latency("stripe", Duration::from_millis(45));
        collector.record_latency("stripe", Duration::from_millis(55));

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.p50_latency_ms, 50);
        assert_eq!(stats.avg_latency_ms, 50.0);
    }

    #[test]
    fn test_success_rate_calculation() {
        let collector = MetricsCollector::new();
        for _ in 0..70 {
            collector.record_success("stripe");
        }
        for _ in 0..30 {
            collector.record_failure("stripe");
        }

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.total_requests, 100);
        assert_eq!(stats.successful_requests, 70);
        assert_eq!(stats.failed_requests, 30);
        assert_eq!(stats.success_rate, 0.7);
    }

    #[test]
    fn test_rolling_window_eviction() {
        let collector = MetricsCollector::with_config(100, 0.2);
        for i in 0..200 {
            collector.record_request(
                "stripe",
                i % 2 == 0,
                Duration::from_millis(10),
            );
        }

        assert_eq!(collector.total_samples("stripe"), 100);
        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.total_requests, 100);
    }

    #[test]
    fn test_percentile_accuracy() {
        let collector = MetricsCollector::new();
        let latencies = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        for lat in latencies {
            collector.record_latency("stripe", Duration::from_millis(lat));
        }

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.p50_latency_ms, 55);
        assert_eq!(stats.p95_latency_ms, 95);
        assert_eq!(stats.p99_latency_ms, 99);
    }

    #[test]
    fn test_min_max_latency() {
        let collector = MetricsCollector::new();
        collector.record_latency("stripe", Duration::from_millis(10));
        collector.record_latency("stripe", Duration::from_millis(100));
        collector.record_latency("stripe", Duration::from_millis(50));

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.min_latency_ms, 10);
        assert_eq!(stats.max_latency_ms, 100);
    }

    #[test]
    fn test_exponential_moving_average() {
        let collector = MetricsCollector::with_config(1000, 0.2);
        collector.record_latency("stripe", Duration::from_millis(100));
        collector.record_latency("stripe", Duration::from_millis(100));
        collector.record_latency("stripe", Duration::from_millis(100));

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.ema_latency_ms, 100.0);

        collector.record_latency("stripe", Duration::from_millis(200));
        let stats = collector.get_stats("stripe").unwrap();
        assert!(stats.ema_latency_ms > 100.0 && stats.ema_latency_ms < 200.0);
    }

    #[test]
    fn test_multiple_routes() {
        let collector = MetricsCollector::new();
        collector.record_success("stripe");
        collector.record_success("adyen");
        collector.record_failure("stripe");

        let stripe_stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stripe_stats.total_requests, 2);
        assert_eq!(stripe_stats.success_rate, 0.5);

        let adyen_stats = collector.get_stats("adyen").unwrap();
        assert_eq!(adyen_stats.total_requests, 1);
        assert_eq!(adyen_stats.success_rate, 1.0);
    }

    #[test]
    fn test_get_all_stats() {
        let collector = MetricsCollector::new();
        collector.record_success("stripe");
        collector.record_success("adyen");

        let all_stats = collector.get_all_stats();
        assert_eq!(all_stats.len(), 2);
        assert!(all_stats.contains_key("stripe"));
        assert!(all_stats.contains_key("adyen"));
    }

    #[test]
    fn test_reset_route() {
        let collector = MetricsCollector::new();
        collector.record_success("stripe");
        assert!(collector.get_stats("stripe").is_some());

        collector.reset("stripe");
        assert!(collector.get_stats("stripe").is_none());
    }

    #[test]
    fn test_reset_all() {
        let collector = MetricsCollector::new();
        collector.record_success("stripe");
        collector.record_success("adyen");

        collector.reset_all();
        assert!(collector.get_stats("stripe").is_none());
        assert!(collector.get_stats("adyen").is_none());
    }

    #[test]
    fn test_empty_stats() {
        let collector = MetricsCollector::new();
        collector.record_success("stripe");

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.p50_latency_ms, 0);
        assert_eq!(stats.avg_latency_ms, 0.0);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let collector = MetricsCollector::new();
        let collector_clone = collector.clone();

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                collector_clone.record_success("stripe");
            }
        });

        for _ in 0..100 {
            collector.record_failure("stripe");
        }

        handle.join().unwrap();

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.total_requests, 200);
    }

    #[test]
    fn test_record_request_combined() {
        let collector = MetricsCollector::new();
        collector.record_request("stripe", true, Duration::from_millis(50));
        collector.record_request("stripe", false, Duration::from_millis(100));

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.success_rate, 0.5);
        assert_eq!(stats.avg_latency_ms, 75.0);
    }

    #[test]
    fn test_window_size_enforcement() {
        let collector = MetricsCollector::with_config(5, 0.2);
        for i in 0..10 {
            collector.record_latency("stripe", Duration::from_millis(i * 10));
        }

        assert_eq!(collector.total_samples("stripe"), 5);
        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.min_latency_ms, 50);
        assert_eq!(stats.max_latency_ms, 90);
    }

    #[test]
    fn test_percentile_edge_cases() {
        let collector = MetricsCollector::new();
        collector.record_latency("stripe", Duration::from_millis(100));

        let stats = collector.get_stats("stripe").unwrap();
        assert_eq!(stats.p50_latency_ms, 100);
        assert_eq!(stats.p95_latency_ms, 100);
        assert_eq!(stats.p99_latency_ms, 100);
    }

    #[test]
    fn test_large_dataset_percentiles() {
        let collector = MetricsCollector::new();
        for i in 1..=1000 {
            collector.record_latency("stripe", Duration::from_millis(i));
        }

        let stats = collector.get_stats("stripe").unwrap();
        assert!(stats.p50_latency_ms >= 495 && stats.p50_latency_ms <= 505);
        assert!(stats.p95_latency_ms >= 945 && stats.p95_latency_ms <= 955);
        assert!(stats.p99_latency_ms >= 985 && stats.p99_latency_ms <= 995);
    }
}
