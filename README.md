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
