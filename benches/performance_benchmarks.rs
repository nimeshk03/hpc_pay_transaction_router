use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use transaction_router::{
    PriorityQueue, BackpressureMonitor, BackpressureConfig, MetricsCollector,
    HealthChecker, LatencyPredictor, LatencyPredictorConfig,
};
use transaction_router::queue::priority_queue::Priority;
use std::time::Duration;

fn priority_queue_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_queue");
    
    // Benchmark enqueue operations
    group.bench_function("enqueue_single", |b| {
        let queue = PriorityQueue::new();
        b.iter(|| {
            queue.enqueue(black_box(42), Priority::Normal);
        });
    });
    
    // Benchmark dequeue operations
    group.bench_function("dequeue_single", |b| {
        let queue = PriorityQueue::new();
        for i in 0..1000 {
            queue.enqueue(i, Priority::Normal);
        }
        b.iter(|| {
            queue.dequeue();
        });
    });
    
    // Benchmark mixed operations
    group.bench_function("mixed_operations", |b| {
        let queue = PriorityQueue::new();
        b.iter(|| {
            queue.enqueue(black_box(42), Priority::Normal);
            queue.dequeue();
        });
    });
    
    // Benchmark with different priorities
    for priority in [Priority::Low, Priority::Normal, Priority::High, Priority::Critical] {
        group.bench_with_input(
            BenchmarkId::new("enqueue_by_priority", format!("{:?}", priority)),
            &priority,
            |b, &prio| {
                let queue = PriorityQueue::new();
                b.iter(|| {
                    queue.enqueue(black_box(42), prio);
                });
            },
        );
    }
    
    group.finish();
}

fn backpressure_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure");
    
    // Benchmark try_enqueue
    group.bench_function("try_enqueue", |b| {
        let config = BackpressureConfig::new(10000);
        let monitor = BackpressureMonitor::new(config);
        b.iter(|| {
            monitor.try_enqueue();
        });
    });
    
    // Benchmark status check
    group.bench_function("status_check", |b| {
        let monitor = BackpressureMonitor::with_default_config();
        for _ in 0..5000 {
            monitor.enqueue();
        }
        b.iter(|| {
            black_box(monitor.status());
        });
    });
    
    // Benchmark utilization calculation
    group.bench_function("utilization", |b| {
        let monitor = BackpressureMonitor::with_default_config();
        for _ in 0..5000 {
            monitor.enqueue();
        }
        b.iter(|| {
            black_box(monitor.utilization());
        });
    });
    
    group.finish();
}

fn metrics_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics");
    
    // Benchmark latency recording
    group.bench_function("record_latency", |b| {
        let collector = MetricsCollector::new();
        b.iter(|| {
            collector.record_latency(black_box("stripe"), Duration::from_millis(50));
        });
    });
    
    // Benchmark success recording
    group.bench_function("record_success", |b| {
        let collector = MetricsCollector::new();
        b.iter(|| {
            collector.record_success(black_box("stripe"));
        });
    });
    
    // Benchmark combined request recording
    group.bench_function("record_request", |b| {
        let collector = MetricsCollector::new();
        b.iter(|| {
            collector.record_request(
                black_box("stripe"),
                true,
                Duration::from_millis(50),
            );
        });
    });
    
    // Benchmark stats retrieval
    group.bench_function("get_stats", |b| {
        let collector = MetricsCollector::new();
        for _ in 0..100 {
            collector.record_latency("stripe", Duration::from_millis(50));
        }
        b.iter(|| {
            black_box(collector.get_stats("stripe"));
        });
    });
    
    group.finish();
}

fn health_checker_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("health_checker");
    
    // Benchmark probe recording
    group.bench_function("record_probe_success", |b| {
        let checker = HealthChecker::with_default_config();
        b.iter(|| {
            checker.record_probe_success(black_box("stripe"), 100);
        });
    });
    
    // Benchmark status check
    group.bench_function("get_status", |b| {
        let checker = HealthChecker::with_default_config();
        for _ in 0..50 {
            checker.record_probe_success("stripe", 100);
        }
        b.iter(|| {
            black_box(checker.get_status("stripe"));
        });
    });
    
    group.finish();
}

fn predictor_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("predictor");
    
    // Benchmark latency update
    group.bench_function("update", |b| {
        let predictor = LatencyPredictor::with_default_config();
        b.iter(|| {
            predictor.update(black_box("stripe"), Duration::from_millis(50));
        });
    });
    
    // Benchmark prediction
    group.bench_function("predict", |b| {
        let predictor = LatencyPredictor::with_default_config();
        for _ in 0..100 {
            predictor.update("stripe", Duration::from_millis(50));
        }
        b.iter(|| {
            black_box(predictor.predict("stripe"));
        });
    });
    
    // Benchmark anomaly detection
    group.bench_function("is_anomaly", |b| {
        let predictor = LatencyPredictor::with_default_config();
        for _ in 0..100 {
            predictor.update("stripe", Duration::from_millis(50));
        }
        b.iter(|| {
            black_box(predictor.is_anomaly("stripe", Duration::from_millis(500)));
        });
    });
    
    group.finish();
}

fn throughput_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(1000));
    
    // Benchmark sustained queue operations
    group.bench_function("queue_1000_ops", |b| {
        b.iter(|| {
            let queue = PriorityQueue::new();
            for i in 0..1000 {
                queue.enqueue(i, Priority::Normal);
            }
            for _ in 0..1000 {
                queue.dequeue();
            }
        });
    });
    
    // Benchmark sustained metrics operations
    group.bench_function("metrics_1000_ops", |b| {
        b.iter(|| {
            let collector = MetricsCollector::new();
            for _ in 0..1000 {
                collector.record_request("stripe", true, Duration::from_millis(50));
            }
        });
    });
    
    group.finish();
}

fn memory_stress_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_stress");
    group.sample_size(10);
    
    // Test with large queue
    group.bench_function("large_queue_10k", |b| {
        b.iter(|| {
            let queue = PriorityQueue::with_capacity(10000);
            for i in 0..10000 {
                queue.enqueue(i, Priority::Normal);
            }
            for _ in 0..10000 {
                queue.dequeue();
            }
        });
    });
    
    // Test with many metrics
    group.bench_function("many_metrics_10k", |b| {
        b.iter(|| {
            let collector = MetricsCollector::new();
            for i in 0..10000 {
                collector.record_latency(&format!("route_{}", i % 100), Duration::from_millis(50));
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    priority_queue_benchmarks,
    backpressure_benchmarks,
    metrics_benchmarks,
    health_checker_benchmarks,
    predictor_benchmarks,
    throughput_benchmarks,
    memory_stress_test,
);
criterion_main!(benches);
