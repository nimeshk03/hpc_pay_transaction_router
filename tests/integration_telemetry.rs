use transaction_router::{
    MetricsExporter, PrometheusExporter, MetricValue, RoutingMetricsCollector,
    SpanContext, TraceContext, SpanStatus, SpanBuilder,
};
use std::sync::Arc;

#[test]
fn test_prometheus_exporter_basic() {
    let exporter = PrometheusExporter::new("router");
    
    exporter.export(MetricValue::counter("requests_total", 100.0));
    exporter.export(MetricValue::gauge("active_connections", 5.0));
    exporter.export(MetricValue::histogram("latency_ms", 45.0));
    
    let output = exporter.render();
    
    assert!(output.contains("router_requests_total"));
    assert!(output.contains("router_active_connections"));
    assert!(output.contains("router_latency_ms"));
}

#[test]
fn test_prometheus_exporter_with_labels() {
    let exporter = PrometheusExporter::new("app");
    
    exporter.export(
        MetricValue::counter("http_requests_total", 150.0)
            .with_label("method", "POST")
            .with_label("status", "200"),
    );
    
    let output = exporter.render();
    
    assert!(output.contains("method=\"POST\""));
    assert!(output.contains("status=\"200\""));
}

#[test]
fn test_prometheus_exporter_batch() {
    let exporter = PrometheusExporter::new("test");
    
    let metrics = vec![
        MetricValue::counter("metric1", 1.0),
        MetricValue::counter("metric2", 2.0),
        MetricValue::counter("metric3", 3.0),
    ];
    
    exporter.export_batch(metrics);
    
    let all_metrics = exporter.get_all_metrics();
    assert_eq!(all_metrics.len(), 3);
}

#[test]
fn test_prometheus_exporter_reset() {
    let exporter = PrometheusExporter::new("test");
    
    exporter.export(MetricValue::counter("requests", 100.0));
    assert!(!exporter.get_all_metrics().is_empty());
    
    exporter.reset();
    assert!(exporter.get_all_metrics().is_empty());
}

#[test]
fn test_routing_metrics_collector() {
    let exporter = Arc::new(PrometheusExporter::new("router"));
    let collector = RoutingMetricsCollector::new(exporter.clone());
    
    collector.record_routing_decision("stripe", 45, true);
    collector.record_routing_decision("stripe", 52, true);
    collector.record_routing_decision("adyen", 38, false);
    
    let output = exporter.render();
    
    assert!(output.contains("routing_decisions_total"));
    assert!(output.contains("routing_decision_latency_ms"));
    assert!(output.contains("psp_id=\"stripe\""));
    assert!(output.contains("psp_id=\"adyen\""));
}

#[test]
fn test_routing_metrics_circuit_breaker() {
    let exporter = Arc::new(PrometheusExporter::new("router"));
    let collector = RoutingMetricsCollector::new(exporter.clone());
    
    collector.record_circuit_breaker_state("stripe", "closed");
    collector.record_circuit_breaker_state("adyen", "open");
    
    let output = exporter.render();
    
    assert!(output.contains("circuit_breaker_state"));
}

#[test]
fn test_routing_metrics_queue_and_backpressure() {
    let exporter = Arc::new(PrometheusExporter::new("router"));
    let collector = RoutingMetricsCollector::new(exporter.clone());
    
    collector.record_queue_depth(150);
    collector.record_backpressure_status("warning");
    
    let output = exporter.render();
    
    assert!(output.contains("queue_depth"));
    assert!(output.contains("backpressure_status"));
}

#[test]
fn test_routing_metrics_health_status() {
    let exporter = Arc::new(PrometheusExporter::new("router"));
    let collector = RoutingMetricsCollector::new(exporter.clone());
    
    collector.record_health_status("stripe", true);
    collector.record_health_status("adyen", false);
    
    let output = exporter.render();
    
    assert!(output.contains("psp_health_status"));
}

#[test]
fn test_trace_context_creation() {
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
    
    let parsed = TraceContext::from_w3c_traceparent(&header).unwrap();
    assert_eq!(parsed.trace_id, ctx.trace_id);
    assert_eq!(parsed.span_id, ctx.span_id);
}

#[test]
fn test_trace_context_baggage() {
    let ctx = TraceContext::new()
        .with_baggage("user_id", "12345")
        .with_baggage("tenant", "acme");
    
    assert_eq!(ctx.baggage.get("user_id"), Some(&"12345".to_string()));
    assert_eq!(ctx.baggage.get("tenant"), Some(&"acme".to_string()));
}

#[test]
fn test_span_context_basic() {
    let span = SpanContext::new("route_transaction");
    
    assert_eq!(span.name, "route_transaction");
    assert!(!span.is_ended());
    assert_eq!(span.status, SpanStatus::Unset);
}

#[test]
fn test_span_context_attributes_and_events() {
    let mut span = SpanContext::new("process_payment");
    
    span.set_attribute("psp_id", "stripe");
    span.set_attribute("amount", "100.00");
    span.add_event("validation_started");
    span.add_event("routing_complete");
    
    assert_eq!(span.attributes.get("psp_id"), Some(&"stripe".to_string()));
    assert_eq!(span.events.len(), 2);
}

#[test]
fn test_span_context_end() {
    let mut span = SpanContext::new("test");
    
    std::thread::sleep(std::time::Duration::from_millis(10));
    span.end();
    
    assert!(span.is_ended());
    assert!(span.duration_ms().unwrap() >= 10);
}

#[test]
fn test_span_context_child() {
    let parent = SpanContext::new("parent_operation");
    let child = parent.child("child_operation");
    
    assert_eq!(child.trace_id(), parent.trace_id());
    assert_ne!(child.span_id(), parent.span_id());
    assert_eq!(child.parent_span_id(), Some(parent.span_id()));
}

#[test]
fn test_span_builder() {
    let span = SpanBuilder::new("database_query")
        .with_attribute("db.system", "postgresql")
        .with_attribute("db.operation", "SELECT")
        .start();
    
    assert_eq!(span.name, "database_query");
    assert_eq!(span.attributes.get("db.system"), Some(&"postgresql".to_string()));
}

#[test]
fn test_span_builder_with_parent() {
    let parent = SpanContext::new("http_request");
    
    let child = SpanBuilder::new("database_call")
        .with_parent(&parent)
        .with_attribute("db.name", "users")
        .start();
    
    assert_eq!(child.trace_id(), parent.trace_id());
    assert_eq!(child.parent_span_id(), Some(parent.span_id()));
}

#[test]
fn test_full_tracing_workflow() {
    let mut root_span = SpanContext::new("handle_transaction");
    root_span.set_attribute("transaction.id", "tx-12345");
    root_span.add_event("request_received");
    
    let mut routing_span = root_span.child("route_transaction");
    routing_span.set_attribute("routing.strategy", "weighted");
    routing_span.add_event("routes_evaluated");
    routing_span.set_status(SpanStatus::Ok);
    routing_span.end();
    
    let mut psp_span = root_span.child("call_psp");
    psp_span.set_attribute("psp.id", "stripe");
    psp_span.set_attribute("psp.latency_ms", "45");
    psp_span.set_status(SpanStatus::Ok);
    psp_span.end();
    
    root_span.add_event("transaction_complete");
    root_span.set_status(SpanStatus::Ok);
    root_span.end();
    
    assert!(root_span.is_ended());
    assert!(routing_span.is_ended());
    assert!(psp_span.is_ended());
    
    assert_eq!(routing_span.trace_id(), root_span.trace_id());
    assert_eq!(psp_span.trace_id(), root_span.trace_id());
}

#[test]
fn test_metrics_and_tracing_integration() {
    let exporter = Arc::new(PrometheusExporter::new("router"));
    let collector = RoutingMetricsCollector::new(exporter.clone());
    
    let mut span = SpanContext::new("route_transaction");
    span.set_attribute("psp_id", "stripe");
    
    std::thread::sleep(std::time::Duration::from_millis(5));
    span.end();
    
    let latency = span.duration_ms().unwrap() as u64;
    collector.record_routing_decision("stripe", latency, true);
    
    let output = exporter.render();
    assert!(output.contains("routing_decisions_total"));
    assert!(output.contains("psp_id=\"stripe\""));
}
