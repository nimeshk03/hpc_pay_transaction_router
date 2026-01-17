use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

#[derive(Debug, Clone)]
pub struct MetricValue {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: i64,
}

impl MetricValue {
    pub fn counter(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            metric_type: MetricType::Counter,
            value,
            labels: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn gauge(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            metric_type: MetricType::Gauge,
            value,
            labels: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn histogram(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            metric_type: MetricType::Histogram,
            value,
            labels: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels.extend(labels);
        self
    }
}

pub trait MetricsExporter: Send + Sync {
    fn export(&self, metric: MetricValue);
    fn export_batch(&self, metrics: Vec<MetricValue>);
    fn render(&self) -> String;
    fn reset(&self);
}

#[derive(Clone)]
pub struct PrometheusExporter {
    metrics: Arc<Mutex<HashMap<String, Vec<MetricValue>>>>,
    prefix: String,
}

impl PrometheusExporter {
    pub fn new(prefix: &str) -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            prefix: prefix.to_string(),
        }
    }

    pub fn with_default_prefix() -> Self {
        Self::new("transaction_router")
    }

    fn metric_key(&self, metric: &MetricValue) -> String {
        let mut key = format!("{}_{}", self.prefix, metric.name);
        if !metric.labels.is_empty() {
            let mut labels: Vec<_> = metric.labels.iter().collect();
            labels.sort_by_key(|(k, _)| *k);
            let label_str: Vec<String> = labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect();
            key = format!("{}{{{}}}", key, label_str.join(","));
        }
        key
    }

    fn format_metric(&self, metric: &MetricValue) -> String {
        let key = self.metric_key(metric);
        format!("{} {}", key, metric.value)
    }

    pub fn get_metric(&self, name: &str) -> Option<Vec<MetricValue>> {
        let metrics = self.metrics.lock().unwrap();
        metrics.get(name).cloned()
    }

    pub fn get_all_metrics(&self) -> HashMap<String, Vec<MetricValue>> {
        let metrics = self.metrics.lock().unwrap();
        metrics.clone()
    }
}

impl MetricsExporter for PrometheusExporter {
    fn export(&self, metric: MetricValue) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics
            .entry(metric.name.clone())
            .or_default()
            .push(metric);
    }

    fn export_batch(&self, batch: Vec<MetricValue>) {
        let mut metrics = self.metrics.lock().unwrap();
        for metric in batch {
            metrics
                .entry(metric.name.clone())
                .or_default()
                .push(metric);
        }
    }

    fn render(&self) -> String {
        let metrics = self.metrics.lock().unwrap();
        let mut output = Vec::new();

        for (name, values) in metrics.iter() {
            if let Some(first) = values.first() {
                let type_str = match first.metric_type {
                    MetricType::Counter => "counter",
                    MetricType::Gauge => "gauge",
                    MetricType::Histogram => "histogram",
                    MetricType::Summary => "summary",
                };
                output.push(format!(
                    "# HELP {}_{} {}\n# TYPE {}_{} {}",
                    self.prefix, name, name, self.prefix, name, type_str
                ));
            }

            for metric in values {
                output.push(self.format_metric(metric));
            }
        }

        output.join("\n")
    }

    fn reset(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.clear();
    }
}

pub struct RoutingMetricsCollector {
    exporter: Arc<dyn MetricsExporter>,
}

impl RoutingMetricsCollector {
    pub fn new(exporter: Arc<dyn MetricsExporter>) -> Self {
        Self { exporter }
    }

    pub fn record_routing_decision(&self, psp_id: &str, latency_ms: u64, success: bool) {
        let status = if success { "success" } else { "failure" };

        self.exporter.export(
            MetricValue::counter("routing_decisions_total", 1.0)
                .with_label("psp_id", psp_id)
                .with_label("status", status),
        );

        self.exporter.export(
            MetricValue::histogram("routing_decision_latency_ms", latency_ms as f64)
                .with_label("psp_id", psp_id),
        );
    }

    pub fn record_circuit_breaker_state(&self, psp_id: &str, state: &str) {
        let state_value = match state {
            "closed" => 0.0,
            "half_open" => 1.0,
            "open" => 2.0,
            _ => -1.0,
        };

        self.exporter.export(
            MetricValue::gauge("circuit_breaker_state", state_value)
                .with_label("psp_id", psp_id),
        );
    }

    pub fn record_queue_depth(&self, depth: usize) {
        self.exporter
            .export(MetricValue::gauge("queue_depth", depth as f64));
    }

    pub fn record_backpressure_status(&self, status: &str) {
        let status_value = match status {
            "normal" => 0.0,
            "warning" => 1.0,
            "critical" => 2.0,
            "overload" => 3.0,
            _ => -1.0,
        };

        self.exporter
            .export(MetricValue::gauge("backpressure_status", status_value));
    }

    pub fn record_health_status(&self, psp_id: &str, healthy: bool) {
        self.exporter.export(
            MetricValue::gauge("psp_health_status", if healthy { 1.0 } else { 0.0 })
                .with_label("psp_id", psp_id),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_value_counter() {
        let metric = MetricValue::counter("requests_total", 100.0)
            .with_label("method", "POST");

        assert_eq!(metric.name, "requests_total");
        assert_eq!(metric.metric_type, MetricType::Counter);
        assert_eq!(metric.value, 100.0);
        assert_eq!(metric.labels.get("method"), Some(&"POST".to_string()));
    }

    #[test]
    fn test_metric_value_gauge() {
        let metric = MetricValue::gauge("temperature", 23.5);

        assert_eq!(metric.name, "temperature");
        assert_eq!(metric.metric_type, MetricType::Gauge);
        assert_eq!(metric.value, 23.5);
    }

    #[test]
    fn test_metric_value_histogram() {
        let metric = MetricValue::histogram("latency_ms", 150.0)
            .with_label("endpoint", "/api/route");

        assert_eq!(metric.name, "latency_ms");
        assert_eq!(metric.metric_type, MetricType::Histogram);
        assert_eq!(metric.value, 150.0);
    }

    #[test]
    fn test_prometheus_exporter_export() {
        let exporter = PrometheusExporter::new("test");

        exporter.export(MetricValue::counter("requests", 1.0));
        exporter.export(MetricValue::counter("requests", 1.0));

        let metrics = exporter.get_metric("requests").unwrap();
        assert_eq!(metrics.len(), 2);
    }

    #[test]
    fn test_prometheus_exporter_render() {
        let exporter = PrometheusExporter::new("app");

        exporter.export(MetricValue::counter("requests_total", 100.0));
        exporter.export(
            MetricValue::gauge("active_connections", 5.0)
                .with_label("server", "web1"),
        );

        let output = exporter.render();

        assert!(output.contains("app_requests_total"));
        assert!(output.contains("app_active_connections"));
        assert!(output.contains("server=\"web1\""));
    }

    #[test]
    fn test_prometheus_exporter_reset() {
        let exporter = PrometheusExporter::new("test");

        exporter.export(MetricValue::counter("requests", 1.0));
        assert!(!exporter.get_all_metrics().is_empty());

        exporter.reset();
        assert!(exporter.get_all_metrics().is_empty());
    }

    #[test]
    fn test_routing_metrics_collector() {
        let exporter = Arc::new(PrometheusExporter::new("router"));
        let collector = RoutingMetricsCollector::new(exporter.clone());

        collector.record_routing_decision("stripe", 50, true);
        collector.record_circuit_breaker_state("stripe", "closed");
        collector.record_queue_depth(100);
        collector.record_backpressure_status("normal");
        collector.record_health_status("stripe", true);

        let output = exporter.render();
        assert!(output.contains("routing_decisions_total"));
        assert!(output.contains("routing_decision_latency_ms"));
        assert!(output.contains("circuit_breaker_state"));
        assert!(output.contains("queue_depth"));
        assert!(output.contains("backpressure_status"));
        assert!(output.contains("psp_health_status"));
    }
}
