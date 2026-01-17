use super::span_context::{SpanContext, SpanStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct TracerConfig {
    pub service_name: String,
    pub service_version: String,
    pub environment: String,
    pub sampling_rate: f64,
    pub max_spans_per_trace: usize,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            service_name: "transaction_router".to_string(),
            service_version: "0.1.0".to_string(),
            environment: "development".to_string(),
            sampling_rate: 1.0,
            max_spans_per_trace: 1000,
        }
    }
}

impl TracerConfig {
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            ..Default::default()
        }
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.service_version = version.to_string();
        self
    }

    pub fn with_environment(mut self, env: &str) -> Self {
        self.environment = env.to_string();
        self
    }

    pub fn with_sampling_rate(mut self, rate: f64) -> Self {
        self.sampling_rate = rate.clamp(0.0, 1.0);
        self
    }
}

pub struct Tracer {
    config: TracerConfig,
    spans: Arc<Mutex<HashMap<String, Vec<SpanContext>>>>,
    active_spans: Arc<Mutex<Vec<SpanContext>>>,
}

impl Tracer {
    pub fn new(config: TracerConfig) -> Self {
        Self {
            config,
            spans: Arc::new(Mutex::new(HashMap::new())),
            active_spans: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(TracerConfig::default())
    }

    pub fn start_span(&self, name: &str) -> SpanContext {
        let mut span = SpanContext::new(name);
        span.set_attribute("service.name", &self.config.service_name);
        span.set_attribute("service.version", &self.config.service_version);
        span.set_attribute("deployment.environment", &self.config.environment);

        let mut active = self.active_spans.lock().unwrap();
        active.push(span.clone());

        span
    }

    pub fn start_child_span(&self, parent: &SpanContext, name: &str) -> SpanContext {
        let mut span = parent.child(name);
        span.set_attribute("service.name", &self.config.service_name);
        span.set_attribute("service.version", &self.config.service_version);

        let mut active = self.active_spans.lock().unwrap();
        active.push(span.clone());

        span
    }

    pub fn end_span(&self, mut span: SpanContext) {
        span.end();

        let mut active = self.active_spans.lock().unwrap();
        active.retain(|s| s.span_id() != span.span_id());

        let mut spans = self.spans.lock().unwrap();
        spans
            .entry(span.trace_id().to_string())
            .or_insert_with(Vec::new)
            .push(span);
    }

    pub fn end_span_with_status(&self, mut span: SpanContext, status: SpanStatus) {
        span.set_status(status);
        self.end_span(span);
    }

    pub fn current_span(&self) -> Option<SpanContext> {
        let active = self.active_spans.lock().unwrap();
        active.last().cloned()
    }

    pub fn get_trace(&self, trace_id: &str) -> Option<Vec<SpanContext>> {
        let spans = self.spans.lock().unwrap();
        spans.get(trace_id).cloned()
    }

    pub fn get_all_traces(&self) -> HashMap<String, Vec<SpanContext>> {
        let spans = self.spans.lock().unwrap();
        spans.clone()
    }

    pub fn active_span_count(&self) -> usize {
        let active = self.active_spans.lock().unwrap();
        active.len()
    }

    pub fn total_span_count(&self) -> usize {
        let spans = self.spans.lock().unwrap();
        spans.values().map(|v| v.len()).sum()
    }

    pub fn clear(&self) {
        let mut spans = self.spans.lock().unwrap();
        spans.clear();
        let mut active = self.active_spans.lock().unwrap();
        active.clear();
    }

    pub fn config(&self) -> &TracerConfig {
        &self.config
    }
}

impl Clone for Tracer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            spans: self.spans.clone(),
            active_spans: self.active_spans.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracer_config_default() {
        let config = TracerConfig::default();
        assert_eq!(config.service_name, "transaction_router");
        assert_eq!(config.sampling_rate, 1.0);
    }

    #[test]
    fn test_tracer_config_builder() {
        let config = TracerConfig::new("my_service")
            .with_version("2.0.0")
            .with_environment("production")
            .with_sampling_rate(0.5);

        assert_eq!(config.service_name, "my_service");
        assert_eq!(config.service_version, "2.0.0");
        assert_eq!(config.environment, "production");
        assert_eq!(config.sampling_rate, 0.5);
    }

    #[test]
    fn test_tracer_start_span() {
        let tracer = Tracer::with_default_config();
        let span = tracer.start_span("test_operation");

        assert_eq!(span.name, "test_operation");
        assert_eq!(
            span.attributes.get("service.name"),
            Some(&"transaction_router".to_string())
        );
        assert_eq!(tracer.active_span_count(), 1);
    }

    #[test]
    fn test_tracer_end_span() {
        let tracer = Tracer::with_default_config();
        let span = tracer.start_span("test_operation");
        let trace_id = span.trace_id().to_string();

        tracer.end_span(span);

        assert_eq!(tracer.active_span_count(), 0);
        assert_eq!(tracer.total_span_count(), 1);

        let trace = tracer.get_trace(&trace_id).unwrap();
        assert_eq!(trace.len(), 1);
        assert!(trace[0].is_ended());
    }

    #[test]
    fn test_tracer_child_span() {
        let tracer = Tracer::with_default_config();
        let parent = tracer.start_span("parent");
        let child = tracer.start_child_span(&parent, "child");

        assert_eq!(child.trace_id(), parent.trace_id());
        assert_eq!(child.parent_span_id(), Some(parent.span_id()));
        assert_eq!(tracer.active_span_count(), 2);
    }

    #[test]
    fn test_tracer_current_span() {
        let tracer = Tracer::with_default_config();

        assert!(tracer.current_span().is_none());

        let span1 = tracer.start_span("span1");
        assert_eq!(tracer.current_span().unwrap().name, "span1");

        let span2 = tracer.start_span("span2");
        assert_eq!(tracer.current_span().unwrap().name, "span2");

        tracer.end_span(span2);
        assert_eq!(tracer.current_span().unwrap().name, "span1");

        tracer.end_span(span1);
        assert!(tracer.current_span().is_none());
    }

    #[test]
    fn test_tracer_end_with_status() {
        let tracer = Tracer::with_default_config();
        let span = tracer.start_span("test");
        let trace_id = span.trace_id().to_string();

        tracer.end_span_with_status(span, SpanStatus::Error);

        let trace = tracer.get_trace(&trace_id).unwrap();
        assert_eq!(trace[0].status, SpanStatus::Error);
    }

    #[test]
    fn test_tracer_clear() {
        let tracer = Tracer::with_default_config();

        let span1 = tracer.start_span("span1");
        tracer.end_span(span1);
        let _span2 = tracer.start_span("span2");

        assert_eq!(tracer.total_span_count(), 1);
        assert_eq!(tracer.active_span_count(), 1);

        tracer.clear();

        assert_eq!(tracer.total_span_count(), 0);
        assert_eq!(tracer.active_span_count(), 0);
    }

    #[test]
    fn test_tracer_clone() {
        let tracer1 = Tracer::with_default_config();
        let span = tracer1.start_span("test");
        tracer1.end_span(span);

        let tracer2 = tracer1.clone();

        assert_eq!(tracer1.total_span_count(), tracer2.total_span_count());
    }
}
