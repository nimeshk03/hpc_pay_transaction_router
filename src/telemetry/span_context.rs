use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub sampled: bool,
    pub baggage: HashMap<String, String>,
}

impl TraceContext {
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string().replace("-", ""),
            span_id: Self::generate_span_id(),
            parent_span_id: None,
            sampled: true,
            baggage: HashMap::new(),
        }
    }

    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Self::generate_span_id(),
            parent_span_id: Some(self.span_id.clone()),
            sampled: self.sampled,
            baggage: self.baggage.clone(),
        }
    }

    fn generate_span_id() -> String {
        let uuid = Uuid::new_v4();
        uuid.to_string().replace("-", "")[..16].to_string()
    }

    pub fn with_baggage(mut self, key: &str, value: &str) -> Self {
        self.baggage.insert(key.to_string(), value.to_string());
        self
    }

    pub fn to_w3c_traceparent(&self) -> String {
        let sampled_flag = if self.sampled { "01" } else { "00" };
        format!(
            "00-{}-{}-{}",
            self.trace_id,
            self.span_id,
            sampled_flag
        )
    }

    pub fn from_w3c_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }

        let sampled = parts[3] == "01";

        Some(Self {
            trace_id: parts[1].to_string(),
            span_id: parts[2].to_string(),
            parent_span_id: None,
            sampled,
            baggage: HashMap::new(),
        })
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SpanContext {
    pub name: String,
    pub trace_context: TraceContext,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub status: SpanStatus,
    pub attributes: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStatus {
    Unset,
    Ok,
    Error,
}

#[derive(Debug, Clone)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: i64,
    pub attributes: HashMap<String, String>,
}

impl SpanContext {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            trace_context: TraceContext::new(),
            start_time: chrono::Utc::now().timestamp_millis(),
            end_time: None,
            status: SpanStatus::Unset,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn with_trace_context(name: &str, trace_context: TraceContext) -> Self {
        Self {
            name: name.to_string(),
            trace_context,
            start_time: chrono::Utc::now().timestamp_millis(),
            end_time: None,
            status: SpanStatus::Unset,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn child(&self, name: &str) -> Self {
        Self {
            name: name.to_string(),
            trace_context: self.trace_context.child(),
            start_time: chrono::Utc::now().timestamp_millis(),
            end_time: None,
            status: SpanStatus::Unset,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn set_attribute(&mut self, key: &str, value: &str) {
        self.attributes.insert(key.to_string(), value.to_string());
    }

    pub fn add_event(&mut self, name: &str) {
        self.events.push(SpanEvent {
            name: name.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            attributes: HashMap::new(),
        });
    }

    pub fn add_event_with_attributes(&mut self, name: &str, attributes: HashMap<String, String>) {
        self.events.push(SpanEvent {
            name: name.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            attributes,
        });
    }

    pub fn set_status(&mut self, status: SpanStatus) {
        self.status = status;
    }

    pub fn end(&mut self) {
        self.end_time = Some(chrono::Utc::now().timestamp_millis());
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.end_time.map(|end| end - self.start_time)
    }

    pub fn is_ended(&self) -> bool {
        self.end_time.is_some()
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_context.trace_id
    }

    pub fn span_id(&self) -> &str {
        &self.trace_context.span_id
    }

    pub fn parent_span_id(&self) -> Option<&str> {
        self.trace_context.parent_span_id.as_deref()
    }
}

pub struct SpanBuilder {
    name: String,
    trace_context: Option<TraceContext>,
    attributes: HashMap<String, String>,
}

impl SpanBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            trace_context: None,
            attributes: HashMap::new(),
        }
    }

    pub fn with_parent(mut self, parent: &SpanContext) -> Self {
        self.trace_context = Some(parent.trace_context.child());
        self
    }

    pub fn with_trace_context(mut self, ctx: TraceContext) -> Self {
        self.trace_context = Some(ctx);
        self
    }

    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    pub fn start(self) -> SpanContext {
        let mut span = match self.trace_context {
            Some(ctx) => SpanContext::with_trace_context(&self.name, ctx),
            None => SpanContext::new(&self.name),
        };
        span.attributes = self.attributes;
        span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_new() {
        let ctx = TraceContext::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());
        assert!(ctx.sampled);
    }

    #[test]
    fn test_trace_context_child() {
        let parent = TraceContext::new();
        let child = parent.child();

        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
    }

    #[test]
    fn test_trace_context_w3c_format() {
        let ctx = TraceContext::new();
        let header = ctx.to_w3c_traceparent();

        assert!(header.starts_with("00-"));
        assert!(header.ends_with("-01"));

        let parsed = TraceContext::from_w3c_traceparent(&header).unwrap();
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.span_id, ctx.span_id);
        assert!(parsed.sampled);
    }

    #[test]
    fn test_trace_context_baggage() {
        let ctx = TraceContext::new()
            .with_baggage("user_id", "123")
            .with_baggage("tenant", "acme");

        assert_eq!(ctx.baggage.get("user_id"), Some(&"123".to_string()));
        assert_eq!(ctx.baggage.get("tenant"), Some(&"acme".to_string()));
    }

    #[test]
    fn test_span_context_new() {
        let span = SpanContext::new("test_operation");

        assert_eq!(span.name, "test_operation");
        assert!(!span.is_ended());
        assert_eq!(span.status, SpanStatus::Unset);
    }

    #[test]
    fn test_span_context_child() {
        let parent = SpanContext::new("parent");
        let child = parent.child("child");

        assert_eq!(child.trace_id(), parent.trace_id());
        assert_ne!(child.span_id(), parent.span_id());
        assert_eq!(child.parent_span_id(), Some(parent.span_id()));
    }

    #[test]
    fn test_span_context_attributes() {
        let mut span = SpanContext::new("test");
        span.set_attribute("key1", "value1");
        span.set_attribute("key2", "value2");

        assert_eq!(span.attributes.get("key1"), Some(&"value1".to_string()));
        assert_eq!(span.attributes.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_span_context_events() {
        let mut span = SpanContext::new("test");
        span.add_event("event1");
        span.add_event("event2");

        assert_eq!(span.events.len(), 2);
        assert_eq!(span.events[0].name, "event1");
        assert_eq!(span.events[1].name, "event2");
    }

    #[test]
    fn test_span_context_end() {
        let mut span = SpanContext::new("test");
        assert!(!span.is_ended());
        assert!(span.duration_ms().is_none());

        std::thread::sleep(std::time::Duration::from_millis(10));
        span.end();

        assert!(span.is_ended());
        assert!(span.duration_ms().unwrap() >= 10);
    }

    #[test]
    fn test_span_builder() {
        let span = SpanBuilder::new("operation")
            .with_attribute("service", "router")
            .with_attribute("version", "1.0")
            .start();

        assert_eq!(span.name, "operation");
        assert_eq!(span.attributes.get("service"), Some(&"router".to_string()));
        assert_eq!(span.attributes.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_span_builder_with_parent() {
        let parent = SpanContext::new("parent");
        let child = SpanBuilder::new("child")
            .with_parent(&parent)
            .start();

        assert_eq!(child.trace_id(), parent.trace_id());
        assert_eq!(child.parent_span_id(), Some(parent.span_id()));
    }
}
