use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureStatus {
    Normal,
    Warning,
    Critical,
    Overload,
}

#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    pub max_queue_depth: usize,
    pub warning_threshold: f64,
    pub critical_threshold: f64,
    pub overload_threshold: f64,
    pub measurement_window: Duration,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_depth: 10000,
            warning_threshold: 0.6,
            critical_threshold: 0.8,
            overload_threshold: 0.95,
            measurement_window: Duration::from_secs(60),
        }
    }
}

impl BackpressureConfig {
    pub fn new(max_queue_depth: usize) -> Self {
        Self {
            max_queue_depth,
            ..Default::default()
        }
    }

    pub fn with_thresholds(
        mut self,
        warning: f64,
        critical: f64,
        overload: f64,
    ) -> Self {
        self.warning_threshold = warning;
        self.critical_threshold = critical;
        self.overload_threshold = overload;
        self
    }

    pub fn with_measurement_window(mut self, window: Duration) -> Self {
        self.measurement_window = window;
        self
    }
}

#[derive(Debug)]
struct BackpressureMetrics {
    current_depth: AtomicUsize,
    total_enqueued: AtomicU64,
    total_dequeued: AtomicU64,
    total_rejected: AtomicU64,
    peak_depth: AtomicUsize,
}

impl BackpressureMetrics {
    fn new() -> Self {
        Self {
            current_depth: AtomicUsize::new(0),
            total_enqueued: AtomicU64::new(0),
            total_dequeued: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
            peak_depth: AtomicUsize::new(0),
        }
    }

    fn increment_depth(&self) -> usize {
        let new_depth = self.current_depth.fetch_add(1, Ordering::SeqCst) + 1;
        self.total_enqueued.fetch_add(1, Ordering::SeqCst);
        
        let mut peak = self.peak_depth.load(Ordering::SeqCst);
        while new_depth > peak {
            match self.peak_depth.compare_exchange(
                peak,
                new_depth,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
        
        new_depth
    }

    fn decrement_depth(&self) -> usize {
        self.total_dequeued.fetch_add(1, Ordering::SeqCst);
        let current = self.current_depth.load(Ordering::SeqCst);
        if current > 0 {
            self.current_depth.fetch_sub(1, Ordering::SeqCst).saturating_sub(1)
        } else {
            0
        }
    }

    fn reject(&self) {
        self.total_rejected.fetch_add(1, Ordering::SeqCst);
    }

    fn reset(&self) {
        self.current_depth.store(0, Ordering::SeqCst);
        self.total_enqueued.store(0, Ordering::SeqCst);
        self.total_dequeued.store(0, Ordering::SeqCst);
        self.total_rejected.store(0, Ordering::SeqCst);
        self.peak_depth.store(0, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone)]
pub struct BackpressureMonitor {
    config: BackpressureConfig,
    metrics: Arc<BackpressureMetrics>,
}

impl BackpressureMonitor {
    pub fn new(config: BackpressureConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(BackpressureMetrics::new()),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(BackpressureConfig::default())
    }

    pub fn try_enqueue(&self) -> bool {
        let current = self.metrics.current_depth.load(Ordering::SeqCst);
        
        if current >= self.config.max_queue_depth {
            self.metrics.reject();
            false
        } else {
            self.metrics.increment_depth();
            true
        }
    }

    pub fn enqueue(&self) {
        self.metrics.increment_depth();
    }

    pub fn dequeue(&self) {
        self.metrics.decrement_depth();
    }

    pub fn current_depth(&self) -> usize {
        self.metrics.current_depth.load(Ordering::SeqCst)
    }

    pub fn max_depth(&self) -> usize {
        self.config.max_queue_depth
    }

    pub fn utilization(&self) -> f64 {
        let current = self.current_depth() as f64;
        let max = self.config.max_queue_depth as f64;
        if max == 0.0 {
            0.0
        } else {
            current / max
        }
    }

    pub fn status(&self) -> BackpressureStatus {
        let util = self.utilization();
        
        if util >= self.config.overload_threshold {
            BackpressureStatus::Overload
        } else if util >= self.config.critical_threshold {
            BackpressureStatus::Critical
        } else if util >= self.config.warning_threshold {
            BackpressureStatus::Warning
        } else {
            BackpressureStatus::Normal
        }
    }

    pub fn is_under_backpressure(&self) -> bool {
        matches!(
            self.status(),
            BackpressureStatus::Warning
                | BackpressureStatus::Critical
                | BackpressureStatus::Overload
        )
    }

    pub fn should_shed_load(&self) -> bool {
        matches!(
            self.status(),
            BackpressureStatus::Critical | BackpressureStatus::Overload
        )
    }

    pub fn total_enqueued(&self) -> u64 {
        self.metrics.total_enqueued.load(Ordering::SeqCst)
    }

    pub fn total_dequeued(&self) -> u64 {
        self.metrics.total_dequeued.load(Ordering::SeqCst)
    }

    pub fn total_rejected(&self) -> u64 {
        self.metrics.total_rejected.load(Ordering::SeqCst)
    }

    pub fn peak_depth(&self) -> usize {
        self.metrics.peak_depth.load(Ordering::SeqCst)
    }

    pub fn rejection_rate(&self) -> f64 {
        let total = self.total_enqueued() + self.total_rejected();
        if total == 0 {
            0.0
        } else {
            self.total_rejected() as f64 / total as f64
        }
    }

    pub fn throughput(&self, elapsed: Duration) -> f64 {
        let dequeued = self.total_dequeued() as f64;
        let seconds = elapsed.as_secs_f64();
        if seconds == 0.0 {
            0.0
        } else {
            dequeued / seconds
        }
    }

    pub fn reset(&self) {
        self.metrics.reset();
    }

    pub fn get_config(&self) -> BackpressureConfig {
        self.config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = BackpressureMonitor::with_default_config();
        assert_eq!(monitor.current_depth(), 0);
        assert_eq!(monitor.status(), BackpressureStatus::Normal);
    }

    #[test]
    fn test_enqueue_dequeue() {
        let monitor = BackpressureMonitor::with_default_config();
        
        monitor.enqueue();
        assert_eq!(monitor.current_depth(), 1);
        
        monitor.dequeue();
        assert_eq!(monitor.current_depth(), 0);
    }

    #[test]
    fn test_try_enqueue_success() {
        let config = BackpressureConfig::new(10);
        let monitor = BackpressureMonitor::new(config);
        
        for _ in 0..10 {
            assert!(monitor.try_enqueue());
        }
        
        assert_eq!(monitor.current_depth(), 10);
    }

    #[test]
    fn test_try_enqueue_rejection() {
        let config = BackpressureConfig::new(5);
        let monitor = BackpressureMonitor::new(config);
        
        for _ in 0..5 {
            assert!(monitor.try_enqueue());
        }
        
        assert!(!monitor.try_enqueue());
        assert_eq!(monitor.total_rejected(), 1);
    }

    #[test]
    fn test_utilization() {
        let config = BackpressureConfig::new(100);
        let monitor = BackpressureMonitor::new(config);
        
        for _ in 0..50 {
            monitor.enqueue();
        }
        
        assert_eq!(monitor.utilization(), 0.5);
    }

    #[test]
    fn test_status_transitions() {
        let config = BackpressureConfig::new(100)
            .with_thresholds(0.6, 0.8, 0.95);
        let monitor = BackpressureMonitor::new(config);
        
        assert_eq!(monitor.status(), BackpressureStatus::Normal);
        
        for _ in 0..60 {
            monitor.enqueue();
        }
        assert_eq!(monitor.status(), BackpressureStatus::Warning);
        
        for _ in 0..20 {
            monitor.enqueue();
        }
        assert_eq!(monitor.status(), BackpressureStatus::Critical);
        
        for _ in 0..15 {
            monitor.enqueue();
        }
        assert_eq!(monitor.status(), BackpressureStatus::Overload);
    }

    #[test]
    fn test_is_under_backpressure() {
        let config = BackpressureConfig::new(100);
        let monitor = BackpressureMonitor::new(config);
        
        assert!(!monitor.is_under_backpressure());
        
        for _ in 0..60 {
            monitor.enqueue();
        }
        
        assert!(monitor.is_under_backpressure());
    }

    #[test]
    fn test_should_shed_load() {
        let config = BackpressureConfig::new(100);
        let monitor = BackpressureMonitor::new(config);
        
        assert!(!monitor.should_shed_load());
        
        for _ in 0..80 {
            monitor.enqueue();
        }
        
        assert!(monitor.should_shed_load());
    }

    #[test]
    fn test_peak_depth() {
        let monitor = BackpressureMonitor::with_default_config();
        
        for _ in 0..50 {
            monitor.enqueue();
        }
        assert_eq!(monitor.peak_depth(), 50);
        
        for _ in 0..30 {
            monitor.dequeue();
        }
        assert_eq!(monitor.peak_depth(), 50);
        
        for _ in 0..40 {
            monitor.enqueue();
        }
        assert_eq!(monitor.peak_depth(), 60);
    }

    #[test]
    fn test_rejection_rate() {
        let config = BackpressureConfig::new(10);
        let monitor = BackpressureMonitor::new(config);
        
        for _ in 0..10 {
            monitor.try_enqueue();
        }
        
        for _ in 0..5 {
            monitor.try_enqueue();
        }
        
        assert_eq!(monitor.rejection_rate(), 5.0 / 15.0);
    }

    #[test]
    fn test_throughput() {
        let monitor = BackpressureMonitor::with_default_config();
        
        for _ in 0..100 {
            monitor.enqueue();
            monitor.dequeue();
        }
        
        let throughput = monitor.throughput(Duration::from_secs(1));
        assert_eq!(throughput, 100.0);
    }

    #[test]
    fn test_reset() {
        let monitor = BackpressureMonitor::with_default_config();
        
        for _ in 0..50 {
            monitor.enqueue();
        }
        
        monitor.reset();
        
        assert_eq!(monitor.current_depth(), 0);
        assert_eq!(monitor.total_enqueued(), 0);
        assert_eq!(monitor.peak_depth(), 0);
    }

    #[test]
    fn test_concurrent_operations() {
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
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(monitor.current_depth(), 1000);
        assert_eq!(monitor.total_enqueued(), 1000);
    }

    #[test]
    fn test_concurrent_enqueue_dequeue() {
        use std::thread;
        
        let monitor = BackpressureMonitor::with_default_config();
        let mut handles = vec![];
        
        for _ in 0..5 {
            let monitor_clone = monitor.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    monitor_clone.enqueue();
                }
            });
            handles.push(handle);
        }
        
        for _ in 0..5 {
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
    }

    #[test]
    fn test_custom_config() {
        let config = BackpressureConfig::new(500)
            .with_thresholds(0.5, 0.7, 0.9)
            .with_measurement_window(Duration::from_secs(30));
        
        let monitor = BackpressureMonitor::new(config);
        assert_eq!(monitor.max_depth(), 500);
    }

    #[test]
    fn test_zero_max_depth() {
        let config = BackpressureConfig::new(0);
        let monitor = BackpressureMonitor::new(config);
        
        assert!(!monitor.try_enqueue());
        assert_eq!(monitor.utilization(), 0.0);
    }
}
