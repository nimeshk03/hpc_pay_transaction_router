# Transaction Router

Ultra-low-latency transaction routing engine for payment orchestration platforms.

## Overview

This library provides intelligent routing of payment transactions across multiple Payment Service Providers (PSPs) based on real-time network health, cost, and performance metrics.

## Features

### Core Data Models (Completed)
- **TransactionRequest**: Validated transaction data with priority support
- **RouteConfig**: PSP configuration with cost structure and limits
- **RoutingDecision**: Detailed routing decision metadata
- **Error Handling**: Comprehensive error types for all operations

### Configuration Management (Completed)
- **ConfigLoader**: Load configurations from JSON/YAML files
- **ConfigValidator**: Business rule validation for configurations
- **ConfigWatcher**: Hot-reload capability with file change detection
- **Sample Configs**: Production-ready configuration examples

### Route Filtering & Selection (Completed)
- **RouteFilter**: Filter routes by payment method, currency, amount limits, and health status
- **RouteScorer**: Score routes using weighted algorithm (latency, cost, success rate, availability)
- **RouteSelector**: Select routes using highest score or weighted random strategies
- **Fallback Routes**: Automatic fallback route selection for resilience

### Transaction Router (Completed)
- **TransactionRouter**: Complete end-to-end routing with filtering, scoring, and selection
- **Merchant Preferences**: Per-merchant preferred/blocked PSPs and custom scoring weights
- **Audit Logging**: Complete decision history with timestamps and metadata
- **Metrics Collection**: Real-time routing metrics (latency, success rate, throughput)
- **Route Updates**: Hot-reload support for configuration changes

### Circuit Breaker (Completed)
- **CircuitBreaker**: Three-state circuit breaker (CLOSED, OPEN, HALF_OPEN)
- **Automatic State Transitions**: Failure threshold detection and timeout-based recovery
- **Metrics Tracking**: Success/failure rates, rejection counts, state transitions
- **Configurable Thresholds**: Customizable failure/success thresholds and timeouts
- **Thread-Safe**: Concurrent access support with Arc<Mutex>
- **Call Wrapper**: Convenient API for wrapping function calls
- **Router Integration**: Per-PSP circuit breakers with automatic route filtering
- **Cascading Failure Prevention**: Routes with open circuits automatically excluded from selection

### Metrics Collection System (Completed)
- **MetricsCollector**: Per-route metrics with rolling windows
- **Percentile Calculations**: P50, P95, P99 latency tracking with interpolation
- **Exponential Moving Average**: Smoothed latency tracking
- **Rolling Window**: Configurable window size for recent data
- **Success Rate Tracking**: Real-time success/failure rate calculation
- **Min/Max Tracking**: Latency bounds monitoring
- **Thread-Safe**: Concurrent metrics updates
- **High Performance**: Sub-microsecond metric recording

### Active Health Probes (Completed)
- **HealthChecker**: Per-route health status tracking
- **Health Status States**: Healthy, Degraded, Unhealthy, Unknown
- **Availability Tracking**: Success rate calculation for health probes
- **Consecutive Thresholds**: Configurable success/failure thresholds
- **Rolling Window**: Configurable probe history window
- **Response Time Tracking**: Average response time monitoring
- **Thread-Safe**: Concurrent probe recording
- **Status Transitions**: Automatic state transitions based on probe results

### Predictive Latency Model (Completed)
- **LatencyPredictor**: EMA-based latency prediction per route
- **Anomaly Detection**: Statistical anomaly detection with configurable thresholds
- **Degradation Detection**: Automatic detection of degrading performance
- **Trend Analysis**: Performance trend calculation (improving/degrading)
- **Standard Deviation**: Variance tracking for anomaly detection
- **Configurable EMA**: Adjustable smoothing factor (alpha)
- **Thread-Safe**: Concurrent prediction updates
- **High Accuracy**: Sub-millisecond prediction precision

### High-Performance Priority Queue (Completed)
- **PriorityQueue**: Concurrent priority-based transaction ordering
- **Four Priority Levels**: Critical, High, Normal, Low
- **FIFO Within Priority**: Maintains insertion order within same priority
- **Thread-Safe**: Concurrent enqueue/dequeue operations
- **High Throughput**: Optimized for high-concurrency scenarios
- **Flexible Operations**: Enqueue, dequeue, peek, drain, clear
- **Priority Filtering**: Count items by priority level

### Backpressure Monitoring & Load Shedding (Completed)
- **BackpressureMonitor**: Real-time queue depth monitoring
- **Four Status Levels**: Normal, Warning, Critical, Overload
- **Configurable Thresholds**: Customizable warning/critical/overload levels
- **Load Shedding**: Automatic rejection when queue is full
- **Metrics Tracking**: Enqueued, dequeued, rejected, peak depth
- **Utilization Monitoring**: Real-time queue utilization percentage
- **Throughput Calculation**: Requests per second measurement
- **Thread-Safe**: Concurrent monitoring operations

### Performance Benchmarking (Completed)
- **Comprehensive Benchmark Suite**: Criterion-based performance testing
- **Component Benchmarks**: Queue, backpressure, metrics, health, predictor
- **Throughput Tests**: Sustained 1000+ ops performance measurement
- **Memory Stability Tests**: 7 comprehensive memory leak tests
- **Concurrent Load Tests**: Multi-threaded stress testing
- **Production-Ready**: Verified performance characteristics

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
transaction_router = "0.1.0"
```

## Quick Start

### 1. Create a Transaction Request

```rust
use transaction_router::{TransactionRequest, PaymentMethod, Priority};
use rust_decimal::Decimal;
use std::str::FromStr;

let tx = TransactionRequest::new(
    "merchant_123".to_string(),
    Decimal::from_str("100.00").unwrap(),
    "USD".to_string(),
    PaymentMethod::Card,
)?
.with_priority(Priority::High)
.with_timeout(3000);

println!("Transaction ID: {}", tx.id);
```

### 2. Load Configuration

```rust
use transaction_router::ConfigLoader;

// Load from JSON
let config = ConfigLoader::load("config/routes.json")?;
println!("Loaded {} routes", config.routes.len());

// Load from YAML
let config = ConfigLoader::load("config/routes.yaml")?;
```

### 3. Validate Configuration

```rust
use transaction_router::ConfigValidator;

let config = ConfigLoader::load("config/routes.json")?;
ConfigValidator::validate(&config)?;
println!("Configuration is valid!");
```

### 4. Watch for Configuration Changes

```rust
use transaction_router::ConfigWatcher;
use std::time::Duration;

let watcher = ConfigWatcher::new("config/routes.json")?;

loop {
    if let Some(new_config) = watcher.try_reload()? {
        println!("Configuration reloaded! Version: {}", new_config.version);
    }
    std::thread::sleep(Duration::from_secs(5));
}
```

### 5. Route Selection

```rust
use transaction_router::{
    RouteFilter, RouteScorer, RouteSelector, ScoringWeights,
    TransactionRequest, PaymentMethod
};
use rust_decimal::Decimal;
use std::str::FromStr;

// Load routes from configuration
let config = ConfigLoader::load("config/routes.json")?;
let routes = config.routes;

// Create transaction
let tx = TransactionRequest::new(
    "merchant_123".to_string(),
    Decimal::from_str("100.00").unwrap(),
    "USD".to_string(),
    PaymentMethod::Card,
)?;

// Filter eligible routes
let eligible = RouteFilter::filter_eligible(&routes, &tx);
println!("Found {} eligible routes", eligible.len());

// Score routes with custom weights
let weights = ScoringWeights::new(0.3, 0.3, 0.25, 0.15);
let scorer = RouteScorer::new(weights);
let scores = scorer.score_routes(&eligible, &tx);

// Select best route with fallbacks
let selector = RouteSelector::with_highest_score();
let (primary, fallbacks) = selector.select_with_fallbacks(&eligible, &scores, 2);

if let Some(route) = primary {
    println!("Selected route: {}", route.name);
    println!("Fallback routes: {:?}", 
        fallbacks.iter().map(|r| &r.name).collect::<Vec<_>>());
}
```

### 6. Weighted Random Selection

```rust
use transaction_router::{RouteSelector, SelectionStrategy};

// Use weighted random for load distribution
let selector = RouteSelector::with_weighted_random();
let selected = selector.select(&eligible, &scores);

// Routes with higher scores are selected more frequently
if let Some(route) = selected {
    println!("Randomly selected: {}", route.name);
}
```

### 7. Complete Transaction Routing

```rust
use transaction_router::{
    ConfigLoader, TransactionRouter, TransactionRequest, 
    PaymentMethod, MerchantPreferences, ScoringWeights
};
use rust_decimal::Decimal;
use std::str::FromStr;

// Load configuration
let config = ConfigLoader::load("config/routes.json")?;

// Create router with custom settings
let router = TransactionRouter::new(config)?
    .with_scoring_weights(ScoringWeights::new(0.3, 0.3, 0.25, 0.15));

// Set merchant preferences
let mut preferences = MerchantPreferences::default();
preferences.preferred_psps.push("stripe".to_string());
preferences.blocked_psps.push("slow_psp".to_string());
router.set_merchant_preferences("merchant_vip".to_string(), preferences);

// Route transaction
let tx = TransactionRequest::new(
    "merchant_vip".to_string(),
    Decimal::from_str("100.00").unwrap(),
    "USD".to_string(),
    PaymentMethod::Card,
)?;

let decision = router.route(&tx)?;

println!("Selected: {:?}", decision.selected_route);
println!("Fallbacks: {:?}", decision.fallback_routes);
println!("Decision time: {}μs", decision.decision_time_us);

// Get metrics
let metrics = router.get_metrics();
println!("Total requests: {}", metrics.total_requests);
println!("Success rate: {:.2}%", metrics.success_rate() * 100.0);
println!("Avg latency: {:.2}μs", metrics.average_latency_us());

// View audit log
let audit_log = router.get_audit_log();
for decision in audit_log.iter().take(5) {
    println!("Request {}: {:?}", decision.request_id, decision.selected_route);
}
```

### 8. Circuit Breaker Usage

```rust
use transaction_router::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use std::time::Duration;

// Create circuit breaker with custom config
let config = CircuitBreakerConfig::new(
    5,    // failure_threshold
    2,    // success_threshold
    60,   // timeout_secs
    3     // half_open_max_calls
);
let breaker = CircuitBreaker::new(config);

// Use call wrapper for automatic state management
let result = breaker.call(|| {
    // Your PSP API call here
    make_payment_request()
});

match result {
    Ok(response) => println!("Payment successful: {:?}", response),
    Err(e) => println!("Payment failed or circuit open: {:?}", e),
}

// Manual state tracking
if breaker.is_call_permitted() {
    match make_payment_request() {
        Ok(_) => breaker.record_success(),
        Err(_) => breaker.record_failure(),
    }
}

// Check circuit state
println!("Circuit state: {:?}", breaker.state());

// Get metrics
let metrics = breaker.get_metrics();
println!("Success rate: {:.2}%", metrics.success_rate() * 100.0);
println!("Failed calls: {}", metrics.failed_calls);
println!("Rejected calls: {}", metrics.rejected_calls);
```

### 9. Circuit Breaker Integration with Router

```rust
use transaction_router::{
    ConfigLoader, TransactionRouter, TransactionRequest, PaymentMethod
};
use rust_decimal::Decimal;
use std::str::FromStr;

// Load configuration and create router
let config = ConfigLoader::load("config/routes.json")?;
let router = TransactionRouter::new(config)?;

// Routes with open circuits are automatically filtered out
let tx = TransactionRequest::new(
    "merchant_123".to_string(),
    Decimal::from_str("100.00").unwrap(),
    "USD".to_string(),
    PaymentMethod::Card,
)?;

let decision = router.route(&tx)?;
println!("Selected route: {:?}", decision.selected_route);

// Record success/failure for circuit breaker tracking
if let Some(ref psp_id) = decision.selected_route {
    // After successful payment
    router.record_route_success(psp_id);
    
    // Or after failed payment
    // router.record_route_failure(psp_id);
}

// Check circuit breaker states
let states = router.get_all_circuit_states();
for (psp_id, state) in states {
    println!("{}: {:?}", psp_id, state);
}

// Get circuit breaker metrics for all PSPs
let cb_metrics = router.get_all_circuit_metrics();
for (psp_id, metrics) in cb_metrics {
    println!("{} - Success rate: {:.2}%", psp_id, metrics.success_rate() * 100.0);
}

// Access individual circuit breaker
if let Some(breaker) = router.get_circuit_breaker("stripe") {
    println!("Stripe circuit state: {:?}", breaker.state());
}
```

### 10. Metrics Collection System

```rust
use transaction_router::MetricsCollector;
use std::time::Duration;

// Create metrics collector with default config (1000 sample window, 0.2 EMA alpha)
let collector = MetricsCollector::new();

// Or with custom configuration
let collector = MetricsCollector::with_config(
    5000,  // window_size: keep last 5000 samples
    0.1    // ema_alpha: smoothing factor for exponential moving average
);

// Record latency measurements
collector.record_latency("stripe", Duration::from_millis(45));
collector.record_latency("stripe", Duration::from_millis(55));

// Record success/failure
collector.record_success("stripe");
collector.record_failure("stripe");

// Record combined request (success + latency)
collector.record_request("stripe", true, Duration::from_millis(50));

// Get statistics for a route
if let Some(stats) = collector.get_stats("stripe") {
    println!("Total requests: {}", stats.total_requests);
    println!("Success rate: {:.2}%", stats.success_rate * 100.0);
    println!("Average latency: {:.2}ms", stats.avg_latency_ms);
    println!("P50 latency: {}ms", stats.p50_latency_ms);
    println!("P95 latency: {}ms", stats.p95_latency_ms);
    println!("P99 latency: {}ms", stats.p99_latency_ms);
    println!("Min latency: {}ms", stats.min_latency_ms);
    println!("Max latency: {}ms", stats.max_latency_ms);
    println!("EMA latency: {:.2}ms", stats.ema_latency_ms);
}

// Get statistics for all routes
let all_stats = collector.get_all_stats();
for (route_id, stats) in all_stats {
    println!("{}: {:.2}% success, P95: {}ms", 
        route_id, stats.success_rate * 100.0, stats.p95_latency_ms);
}

// Reset metrics for a specific route
collector.reset("stripe");

// Reset all metrics
collector.reset_all();
```

### 11. Active Health Probes

```rust
use transaction_router::{HealthChecker, HealthCheckConfig, HealthStatus};

// Create health checker with default config
let checker = HealthChecker::with_default_config();

// Or with custom configuration
let config = HealthCheckConfig::new(
    30,    // probe_interval_secs: check every 30 seconds
    5000,  // timeout_ms: 5 second timeout
    3,     // healthy_threshold: 3 consecutive successes = healthy
    3      // unhealthy_threshold: 3 consecutive failures = unhealthy
).with_window_size(100);  // keep last 100 probe results

let checker = HealthChecker::new(config);

// Add routes to monitor
checker.add_route("stripe");
checker.add_route("adyen");

// Record probe results
checker.record_probe_success("stripe", 150);  // 150ms response time
checker.record_probe_failure("adyen", "Connection timeout".to_string());

// Check health status
let status = checker.get_status("stripe");
match status {
    HealthStatus::Healthy => println!("Route is healthy"),
    HealthStatus::Degraded => println!("Route is degraded"),
    HealthStatus::Unhealthy => println!("Route is unhealthy"),
    HealthStatus::Unknown => println!("No probe data yet"),
}

// Check if route is healthy
if checker.is_healthy("stripe") {
    println!("Stripe is ready to handle traffic");
}

// Get availability score (0.0 - 1.0)
let availability = checker.get_availability("stripe");
println!("Availability: {:.2}%", availability * 100.0);

// Get average response time
let avg_time = checker.get_avg_response_time("stripe");
println!("Average response time: {:.2}ms", avg_time);

// Get probe count
let probe_count = checker.get_probe_count("stripe");
println!("Total probes: {}", probe_count);

// Get all route statuses
let all_statuses = checker.get_all_statuses();
for (route_id, status) in all_statuses {
    println!("{}: {:?}", route_id, status);
}

// Get all availabilities
let all_availabilities = checker.get_all_availabilities();
for (route_id, availability) in all_availabilities {
    println!("{}: {:.2}%", route_id, availability * 100.0);
}

// Reset specific route
checker.reset("stripe");

// Reset all routes
checker.reset_all();
```

### 12. Predictive Latency Model

```rust
use transaction_router::{LatencyPredictor, LatencyPredictorConfig};
use std::time::Duration;

// Create predictor with default config
let predictor = LatencyPredictor::with_default_config();

// Or with custom configuration
let config = LatencyPredictorConfig::new(0.3)  // EMA alpha
    .with_anomaly_threshold(3.0)  // 3x std dev for anomaly
    .with_degradation_config(20, 1.5);  // window=20, threshold=1.5x

let predictor = LatencyPredictor::new(config);

// Update with latency measurements
predictor.update("stripe", Duration::from_millis(100));
predictor.update("stripe", Duration::from_millis(120));
predictor.update("stripe", Duration::from_millis(110));

// Get predicted latency (EMA)
if let Some(predicted) = predictor.predict("stripe") {
    println!("Predicted latency: {}ms", predicted.as_millis());
}

// Detect anomalies
let latency = Duration::from_millis(500);
if predictor.is_anomaly("stripe", latency) {
    println!("Anomaly detected! Latency spike: {}ms", latency.as_millis());
}

// Detect degrading performance
if predictor.is_degrading("stripe") {
    println!("Warning: Route performance is degrading");
}

// Get performance trend (-1.0 to 1.0)
let trend = predictor.get_trend("stripe");
if trend > 0.2 {
    println!("Performance degrading: {:.2}%", trend * 100.0);
} else if trend < -0.2 {
    println!("Performance improving: {:.2}%", trend.abs() * 100.0);
}

// Get standard deviation
let std_dev = predictor.get_std_dev("stripe");
println!("Latency std dev: {:.2}ms", std_dev);

// Get sample count
let count = predictor.get_sample_count("stripe");
println!("Samples: {}", count);

// Get last recorded latency
if let Some(last) = predictor.get_last_latency("stripe") {
    println!("Last latency: {}ms", last.as_millis());
}

// Get all predictions
let all_preds = predictor.get_all_predictions();
for (route_id, predicted) in all_preds {
    println!("{}: {}ms predicted", route_id, predicted.as_millis());
}

// Reset specific route
predictor.reset("stripe");

// Reset all routes
predictor.reset_all();
```

### 13. High-Performance Priority Queue

```rust
use transaction_router::PriorityQueue;
use transaction_router::queue::priority_queue::Priority;

// Create a priority queue
let queue = PriorityQueue::new();

// Or with pre-allocated capacity
let queue = PriorityQueue::with_capacity(1000);

// Enqueue items with different priorities
queue.enqueue("critical_transaction", Priority::Critical);
queue.enqueue("high_priority_tx", Priority::High);
queue.enqueue("normal_tx", Priority::Normal);
queue.enqueue("low_priority_tx", Priority::Low);

// Dequeue items (highest priority first)
while let Some(item) = queue.dequeue() {
    println!("Processing: {:?} with priority {:?}", item.item, item.priority);
}

// Peek at highest priority item without removing
if let Some(priority) = queue.peek() {
    println!("Next priority: {:?}", priority);
}

// Check queue status
println!("Queue length: {}", queue.len());
println!("Is empty: {}", queue.is_empty());

// Count items by priority
let high_count = queue.count_by_priority(Priority::High);
println!("High priority items: {}", high_count);

// Drain all items in priority order
let all_items = queue.drain();
for item in all_items {
    println!("Item: {:?}", item.item);
}

// Clear the queue
queue.clear();

// Try dequeue (non-blocking)
if let Some(item) = queue.try_dequeue() {
    println!("Got item: {:?}", item.item);
}

// Concurrent usage
use std::thread;

let queue = PriorityQueue::new();
let mut handles = vec![];

for i in 0..10 {
    let queue_clone = queue.clone();
    let handle = thread::spawn(move || {
        for j in 0..100 {
            queue_clone.enqueue(i * 100 + j, Priority::Normal);
        }
    });
    handles.push(handle);
}

for handle in handles {
    handle.join().unwrap();
}

println!("Total items: {}", queue.len());
```

### 14. Backpressure Monitoring & Load Shedding

```rust
use transaction_router::{BackpressureMonitor, BackpressureConfig, BackpressureStatus};
use std::time::Duration;

// Create monitor with default config (max 10000 items)
let monitor = BackpressureMonitor::with_default_config();

// Or with custom configuration
let config = BackpressureConfig::new(1000)  // max queue depth
    .with_thresholds(0.6, 0.8, 0.95)  // warning, critical, overload
    .with_measurement_window(Duration::from_secs(60));

let monitor = BackpressureMonitor::new(config);

// Try to enqueue (with load shedding)
if monitor.try_enqueue() {
    println!("Item accepted");
    // Process item...
    monitor.dequeue();
} else {
    println!("Queue full - item rejected");
}

// Or enqueue without checking (for monitoring only)
monitor.enqueue();
// ... later
monitor.dequeue();

// Check backpressure status
match monitor.status() {
    BackpressureStatus::Normal => println!("System operating normally"),
    BackpressureStatus::Warning => println!("Queue filling up"),
    BackpressureStatus::Critical => println!("High load - consider scaling"),
    BackpressureStatus::Overload => println!("System overloaded!"),
}

// Check if under backpressure
if monitor.is_under_backpressure() {
    println!("Warning: System experiencing backpressure");
}

// Check if should shed load
if monitor.should_shed_load() {
    println!("Critical: Rejecting new requests");
}

// Get queue metrics
println!("Current depth: {}", monitor.current_depth());
println!("Max depth: {}", monitor.max_depth());
println!("Utilization: {:.2}%", monitor.utilization() * 100.0);
println!("Peak depth: {}", monitor.peak_depth());

// Get throughput metrics
println!("Total enqueued: {}", monitor.total_enqueued());
println!("Total dequeued: {}", monitor.total_dequeued());
println!("Total rejected: {}", monitor.total_rejected());
println!("Rejection rate: {:.2}%", monitor.rejection_rate() * 100.0);

// Calculate throughput
let elapsed = Duration::from_secs(60);
let throughput = monitor.throughput(elapsed);
println!("Throughput: {:.2} req/sec", throughput);

// Reset metrics
monitor.reset();

// Concurrent usage
use std::thread;

let monitor = BackpressureMonitor::new(BackpressureConfig::new(1000));
let mut handles = vec![];

for i in 0..10 {
    let monitor_clone = monitor.clone();
    let handle = thread::spawn(move || {
        for _ in 0..100 {
            if monitor_clone.try_enqueue() {
                // Process...
                monitor_clone.dequeue();
            }
        }
    });
    handles.push(handle);
}

for handle in handles {
    handle.join().unwrap();
}

println!("Rejected: {}", monitor.total_rejected());
```

## Configuration Format

### JSON Example

```json
{
  "version": 1,
  "routes": [
    {
      "psp_id": "stripe",
      "name": "Stripe",
      "base_url": "https://api.stripe.com",
      "supported_methods": ["Card", "Wallet"],
      "supported_currencies": ["USD", "EUR", "GBP"],
      "cost_structure": {
        "fixed_fee": "0.30",
        "percentage_fee": "2.9",
        "currency": "USD"
      },
      "limits": {
        "min_amount": "0.50",
        "max_amount": "999999.99",
        "daily_volume_cap": "10000000.00"
      },
      "circuit_breaker": {
        "failure_threshold": 5,
        "timeout_seconds": 60
      },
      "priority": 1,
      "enabled": true
    }
  ]
}
```

### YAML Example

```yaml
version: 1
routes:
  - psp_id: stripe
    name: Stripe
    base_url: https://api.stripe.com
    supported_methods:
      - Card
      - Wallet
    supported_currencies:
      - USD
      - EUR
    cost_structure:
      fixed_fee: "0.30"
      percentage_fee: "2.9"
      currency: USD
    limits:
      min_amount: "0.50"
      max_amount: "999999.99"
      daily_volume_cap: "10000000.00"
    circuit_breaker:
      failure_threshold: 5
      timeout_seconds: 60
    priority: 1
    enabled: true
```

## Project Structure

```
transaction_router/
├── src/
│   ├── lib.rs                 # Public API exports
│   ├── config/
│   │   ├── mod.rs            # Config module exports
│   │   ├── loader.rs         # Configuration loading
│   │   ├── validator.rs      # Configuration validation
│   │   └── watcher.rs        # Hot-reload file watching
│   ├── errors/
│   │   └── mod.rs            # Error type definitions
│   └── models/
│       ├── mod.rs            # Model exports
│       ├── transaction.rs    # Transaction data model
│       ├── route.rs          # Route configuration model
│       ├── decision.rs       # Routing decision model
│       └── tests.rs          # Model unit tests
├── tests/
│   └── integration_config_reload.rs  # Integration tests
├── config/
│   ├── routes.json           # Sample JSON config
│   └── routes.yaml           # Sample YAML config
├── Cargo.toml
└── README.md
```

## Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test Suites

```bash
# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test integration_config_reload

# Config module tests
cargo test config::

# Model tests
cargo test models::
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy -- -D warnings
```

## Development

### Using Docker

```bash
# Build development container
docker-compose build

# Start development environment
docker-compose run --rm dev

# Inside container
cargo test
cargo build
```

### Manual Setup

Requirements:
- Rust 1.70+
- Cargo

```bash
# Clone repository
git clone https://github.com/nimeshk03/hpc_pay_transaction_router.git
cd transaction_router

# Build
cargo build

# Run tests
cargo test

# Build release
cargo build --release
```

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Routing Decision Latency | < 1ms P99 | 🔄 In Progress |
| Throughput | 100,000 TPS | 🔄 In Progress |
| Availability | 99.999% | 🔄 In Progress |
| Circuit Breaker Response | < 100us | 🔄 In Progress |

## Roadmap

### Foundation
- [x] Core Data Models
- [x] Configuration Management

### Basic Routing Engine
- [ ] Route Filtering & Selection
- [ ] Basic Router Implementation

### Circuit Breaker
- [ ] Circuit Breaker State Machine
- [ ] Circuit Breaker Integration

### Health Monitoring
- [ ] Metrics Collection
- [ ] Active Health Probes
- [ ] Predictive Latency Model

### Performance Optimization
- [ ] Lock-Free Priority Queue
- [ ] Connection Pooling
- [ ] Performance Tuning

See [implementation_plan.md](../implementation_plan.md) for detailed roadmap.

## API Documentation

Generate and view documentation:

```bash
cargo doc --open
```

## Examples

See the `examples/` directory for more usage examples (coming soon).

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Run linter: `cargo clippy`
6. Format code: `cargo fmt`
7. Submit a pull request

## License

[Add your license here]

## Acknowledgments

Built as part of the HPC Pay payment orchestration platform.
