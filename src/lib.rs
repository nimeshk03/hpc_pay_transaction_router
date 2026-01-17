pub mod backpressure;
pub mod circuit_breaker;
pub mod config;
#[cfg(feature = "redis")]
pub mod distributed;
pub mod errors;
pub mod events;
pub mod health;
pub mod metrics;
pub mod models;
pub mod queue;
pub mod router;
pub mod telemetry;

pub use backpressure::{BackpressureMonitor, BackpressureConfig, BackpressureStatus};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig as CBConfig, CircuitBreakerMetrics, CircuitState};
pub use config::{ConfigLoader, ConfigValidator, ConfigWatcher};
#[cfg(feature = "redis")]
pub use distributed::{RedisClient, RedisConfig, RedisError, DistributedLock, DistributedCounter, StateSync};
pub use errors::{ConfigError, RouteError, TransactionError};
pub use events::{Event, EventType, RoutingEvent, CircuitBreakerEvent, ConfigEvent, EventPublisher, InMemoryEventPublisher};
pub use health::{HealthChecker, HealthCheckConfig, HealthCheckResult, HealthStatus, LatencyPredictor, LatencyPredictorConfig};
pub use metrics::{MetricsCollector, RouteMetrics, RouteStats};
pub use queue::{PriorityQueue, PriorityItem};
pub use models::{
    CircuitBreakerConfig, CostStructure, Limits, PaymentMethod, Priority, RouteConfig,
    RoutingDecision, TransactionRequest,
};
pub use router::{
    MerchantPreferences, RouteFilter, RouteScore, RouteScorer, RouteSelector, RoutingMetrics,
    ScoringWeights, SelectionStrategy, TransactionRouter,
};
pub use telemetry::{MetricsExporter, PrometheusExporter, MetricType, MetricValue, RoutingMetricsCollector, SpanContext, TraceContext, SpanStatus, SpanBuilder};
