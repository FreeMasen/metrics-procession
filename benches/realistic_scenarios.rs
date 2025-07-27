use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use metrics_procession::recorder::ProcessionRecorder;
use std::hint::black_box;
use std::sync::Arc;
use std::thread;

// Simulate HTTP request handling
pub fn http_server_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("http_server_simulation");

    for &concurrent_requests in &[10, 50, 100, 500] {
        group.throughput(Throughput::Elements(concurrent_requests));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrent_requests),
            &concurrent_requests,
            |b, &requests| {
                b.iter_custom(|iters| {
                    let recorder = ProcessionRecorder::default();

                    let total_duration = metrics::with_local_recorder(&recorder, || {
                        let start = std::time::Instant::now();

                        for _ in 0..iters {
                            // Simulate handling concurrent HTTP requests
                            for req_id in 0..requests {
                                let endpoint = match req_id % 4 {
                                    0 => "/api/users",
                                    1 => "/api/orders",
                                    2 => "/api/products",
                                    _ => "/health",
                                };

                                let method = match req_id % 3 {
                                    0 => "GET",
                                    1 => "POST",
                                    _ => "PUT",
                                };

                                let status_code = if req_id % 10 == 0 { "500" } else { "200" };

                                // Request counter
                                metrics::counter!("http_requests_total",
                                    "method" => method,
                                    "endpoint" => endpoint,
                                    "status" => status_code
                                )
                                .increment(1);

                                // Response time histogram
                                let response_time = 10.0 + (req_id as f64 * 0.5);
                                metrics::histogram!("http_request_duration_ms",
                                    "method" => method,
                                    "endpoint" => endpoint
                                )
                                .record(response_time);

                                // Active connections gauge
                                metrics::gauge!("http_active_connections").increment(1.0);
                                metrics::gauge!("http_active_connections").decrement(1.0);
                            }
                        }

                        start.elapsed()
                    });

                    black_box(recorder.memory_size());
                    total_duration
                });
            },
        );
    }
    group.finish();
}

// Simulate database connection pool metrics
pub fn database_pool_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("database_pool_simulation");

    for &pool_size in &[5, 10, 20, 50] {
        group.throughput(Throughput::Elements(pool_size));
        group.bench_with_input(
            BenchmarkId::from_parameter(pool_size),
            &pool_size,
            |b, &pool_size| {
                b.iter_custom(|iters| {
                    let recorder = ProcessionRecorder::default();

                    let total_duration = metrics::with_local_recorder(&recorder, || {
                        let start = std::time::Instant::now();

                        for iteration in 0..iters {
                            for conn_id in 0..pool_size {
                                let db_name = match conn_id % 3 {
                                    0 => "users_db",
                                    1 => "orders_db",
                                    _ => "analytics_db",
                                };

                                // Connection lifecycle
                                metrics::gauge!("db_connections_active", "database" => db_name).increment(1.0);

                                // Query metrics
                                let query_time = 5.0 + (conn_id as f64 * 2.0) + (iteration as f64 * 0.1);
                                metrics::histogram!("db_query_duration_ms", "database" => db_name).record(query_time);

                                metrics::counter!("db_queries_total",
                                    "database" => db_name,
                                    "operation" => if conn_id % 4 == 0 { "write" } else { "read" }
                                ).increment(1);

                                // Simulate occasional connection errors
                                if conn_id % 20 == 0 {
                                    metrics::counter!("db_connection_errors", "database" => db_name).increment(1);
                                }

                                metrics::gauge!("db_connections_active", "database" => db_name).decrement(1.0);
                            }

                            // Pool-level metrics
                            metrics::gauge!("db_pool_size").set(pool_size as f64);
                            metrics::gauge!("db_pool_idle").set((pool_size / 2) as f64);
                        }

                        start.elapsed()
                    });

                    black_box(recorder.memory_size());
                    total_duration
                });
            }
        );
    }
    group.finish();
}

// Simulate application with many different metric names (high cardinality)
pub fn high_cardinality_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_cardinality_simulation");

    for &num_unique_metrics in &[100, 500, 1000, 2000] {
        group.throughput(Throughput::Elements(num_unique_metrics));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_unique_metrics),
            &num_unique_metrics,
            |b, &num_unique_metrics| {
                b.iter_custom(|iters| {
                    let recorder = ProcessionRecorder::default();

                    let total_duration = metrics::with_local_recorder(&recorder, || {
                        let start = std::time::Instant::now();

                        for iteration in 0..iters {
                            for metric_id in 0..num_unique_metrics {
                                let user_id = format!("user_{}", metric_id % 50);
                                let service = format!("service_{}", metric_id % 10);
                                let region = match metric_id % 4 {
                                    0 => "us-east-1",
                                    1 => "us-west-2",
                                    2 => "eu-west-1",
                                    _ => "ap-southeast-1",
                                };

                                // Per-user metrics (high cardinality)
                                metrics::counter!("user_actions_total",
                                    "user_id" => user_id.clone(),
                                    "action_type" => if metric_id % 3 == 0 { "click" } else { "view" }
                                ).increment(1);

                                // Per-service metrics
                                metrics::histogram!("service_response_time",
                                    "service" => service.clone(),
                                    "region" => region
                                ).record((metric_id as f64 * 0.1) + (iteration as f64 * 0.01));

                                // Resource usage per service
                                metrics::gauge!("service_memory_usage",
                                    "service" => service,
                                    "region" => region
                                ).set(100.0 + (metric_id as f64 * 0.5));
                            }
                        }

                        start.elapsed()
                    });

                    black_box(recorder.memory_size());
                    total_duration
                });
            }
        );
    }
    group.finish();
}

// Simulate concurrent multi-threaded metrics collection
pub fn concurrent_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_collection");

    for &num_threads in &[2, 4, 8, 16] {
        group.throughput(Throughput::Elements(num_threads));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let recorder = Arc::new(ProcessionRecorder::default());

                    let total_duration = metrics::with_local_recorder(recorder.as_ref(), || {
                        let start = std::time::Instant::now();

                        let handles: Vec<_> = (0..num_threads)
                            .map(|thread_id| {
                                let recorder_clone = Arc::clone(&recorder);
                                thread::spawn(move || {
                                    metrics::with_local_recorder(recorder_clone.as_ref(), || {
                                        for iteration in 0..iters {
                                            let worker_id = format!("worker_{thread_id}");

                                            // Simulate work being done by each thread
                                            metrics::counter!("work_items_processed",
                                                "worker_id" => worker_id.clone()
                                            )
                                            .increment(1);

                                            let processing_time = (thread_id as f64 * 10.0)
                                                + (iteration as f64 * 0.1);
                                            metrics::histogram!("work_processing_time",
                                                "worker_id" => worker_id.clone()
                                            )
                                            .record(processing_time);

                                            metrics::gauge!("worker_queue_size",
                                                "worker_id" => worker_id
                                            )
                                            .set(iteration as f64);

                                            // Simulate some CPU-bound work to make threads actually concurrent
                                            black_box((0..100).sum::<i32>());
                                        }
                                    });
                                })
                            })
                            .collect();

                        // Wait for all threads to complete
                        for handle in handles {
                            handle.join().unwrap();
                        }

                        start.elapsed()
                    });

                    black_box(recorder.memory_size());
                    total_duration
                });
            },
        );
    }
    group.finish();
}

// Memory efficiency comparison with different recording strategies
pub fn memory_efficiency_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    group.sample_size(20); // Fewer samples since we're measuring memory

    for &events_per_chunk in &[1000, 5000, 10000, 20000] {
        group.throughput(Throughput::Elements(events_per_chunk));
        group.bench_with_input(
            BenchmarkId::from_parameter(events_per_chunk),
            &events_per_chunk,
            |b, &events_per_chunk| {
                b.iter_custom(|_iters| {
                    let recorder = ProcessionRecorder::default();
                    let start = std::time::Instant::now();

                    metrics::with_local_recorder(&recorder, || {
                        // Create a mix of metric types with varying label cardinality
                        for i in 0..events_per_chunk {
                            let label_count = i % 5; // 0-4 labels per metric
                            let value_suffix = format!("{}", i % 10);

                            match (i % 3, label_count) {
                                (0, 0) => metrics::counter!("test_counter").increment(1),
                                (0, 1) => metrics::counter!("test_counter", "label_0" => value_suffix).increment(1),
                                (0, 2) => metrics::counter!("test_counter", "label_0" => value_suffix.clone(), "label_1" => value_suffix).increment(1),
                                (0, 3) => metrics::counter!("test_counter", "label_0" => value_suffix.clone(), "label_1" => value_suffix.clone(), "label_2" => value_suffix).increment(1),
                                (0, _) => metrics::counter!("test_counter", "label_0" => value_suffix.clone(), "label_1" => value_suffix.clone(), "label_2" => value_suffix.clone(), "label_3" => value_suffix).increment(1),

                                (1, 0) => metrics::gauge!("test_gauge").set(i as f64),
                                (1, 1) => metrics::gauge!("test_gauge", "label_0" => value_suffix).set(i as f64),
                                (1, 2) => metrics::gauge!("test_gauge", "label_0" => value_suffix.clone(), "label_1" => value_suffix).set(i as f64),
                                (1, 3) => metrics::gauge!("test_gauge", "label_0" => value_suffix.clone(), "label_1" => value_suffix.clone(), "label_2" => value_suffix).set(i as f64),
                                (1, _) => metrics::gauge!("test_gauge", "label_0" => value_suffix.clone(), "label_1" => value_suffix.clone(), "label_2" => value_suffix.clone(), "label_3" => value_suffix).set(i as f64),

                                (2, 0) => metrics::histogram!("test_histogram").record(i as f64),
                                (2, 1) => metrics::histogram!("test_histogram", "label_0" => value_suffix).record(i as f64),
                                (2, 2) => metrics::histogram!("test_histogram", "label_0" => value_suffix.clone(), "label_1" => value_suffix).record(i as f64),
                                (2, 3) => metrics::histogram!("test_histogram", "label_0" => value_suffix.clone(), "label_1" => value_suffix.clone(), "label_2" => value_suffix).record(i as f64),
                                (2, _) => metrics::histogram!("test_histogram", "label_0" => value_suffix.clone(), "label_1" => value_suffix.clone(), "label_2" => value_suffix.clone(), "label_3" => value_suffix).record(i as f64),

                                _ => unreachable!(),
                            }
                        }
                    });

                    let memory_used = recorder.memory_size();
                    black_box(memory_used);
                    start.elapsed()
                });
            }
        );
    }
    group.finish();
}

criterion_group!(
    realistic_benches,
    http_server_simulation,
    database_pool_simulation,
    high_cardinality_simulation,
    concurrent_collection,
    memory_efficiency_test
);
criterion_main!(realistic_benches);
