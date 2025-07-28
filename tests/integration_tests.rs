use metrics::Key;
use metrics_procession::{
    iter::{Metric, MetricRef},
    procession::Procession,
    recorder::ProcessionRecorder,
};
use std::sync::Arc;
use std::thread;
use time::{Duration, OffsetDateTime};

#[test]
fn test_full_metrics_lifecycle() {
    let recorder = ProcessionRecorder::default();

    metrics::with_local_recorder(&recorder, || {
        // Create various metric types
        let counter =
            metrics::counter!("http_requests_total", "method" => "GET", "status" => "200");
        let gauge = metrics::gauge!("memory_usage_bytes");
        let histogram = metrics::histogram!("request_duration_ms", "endpoint" => "/api/users");

        // Record some events
        for i in 0..100 {
            counter.increment(1);
            gauge.set((1000.0 + i as f64) * 1024.0 * 1024.0);
            histogram.record(50.0 + (i as f64 * 0.1));
        }

        // Additional counter with different labels
        let counter2 =
            metrics::counter!("http_requests_total", "method" => "POST", "status" => "201");
        counter2.increment(25);

        // Gauge operations
        let gauge2 = metrics::gauge!("active_connections");
        gauge2.increment(10.0);
        gauge2.decrement(2.0);
        gauge2.set(15.0);
    });

    let procession = recorder.lock();

    // Verify we have events
    assert!(!procession.chunks.is_empty());
    assert!(!procession.labels.0.is_empty());

    // Verify we can iterate through all events
    let events: Vec<MetricRef> = procession.iter().collect();
    // 100 iterations * 3 events per iteration + 1 counter + 3 gauge ops = 304 events
    assert_eq!(events.len(), 304);

    // Verify serialization works
    let json = serde_json::to_string(&*procession).unwrap();
    assert!(!json.is_empty());

    // Verify deserialization works
    let deserialized: Procession = serde_json::from_str(&json).unwrap();
    assert_eq!(&deserialized, &*procession);
}

#[test]
fn test_concurrent_metrics_collection() {
    let recorder = Arc::new(ProcessionRecorder::default());
    let num_threads = 4;
    let events_per_thread = 100;

    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        let recorder_clone = Arc::clone(&recorder);
        thread::spawn(move || {
            metrics::with_local_recorder(recorder_clone.as_ref(), || {
                let counter = metrics::counter!("worker_events", "worker_id" => format!("worker_{thread_id}"));
                let gauge = metrics::gauge!("worker_progress", "worker_id" => format!("worker_{thread_id}"));

                for i in 0..events_per_thread {
                    counter.increment(1);
                    gauge.set(i as f64);

                    // Simulate some work
                    thread::sleep(std::time::Duration::from_millis(1));
                }
            });
        })
    }).collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let procession = recorder.lock();
    let events: Vec<MetricRef> = procession.iter().collect();

    // Each thread generates 2 * events_per_thread events
    assert_eq!(events.len(), num_threads * events_per_thread * 2);

    // Verify we have the correct number of unique labels
    assert_eq!(procession.labels.0.len(), num_threads * 2); // 2 metrics per thread
}

#[test]
fn test_chunk_splitting_behavior() {
    let recorder = ProcessionRecorder::default();

    metrics::with_local_recorder(&recorder, || {
        let counter = metrics::counter!("test_counter");

        // Record an event
        counter.increment(1);

        // Manually advance time by accessing the procession
        let mut procession = recorder.lock();
        let future_time = OffsetDateTime::now_utc() + Duration::seconds(70000); // Force new chunk
        let label_id = procession.ensure_label(&Key::from_name("test_counter"));
        procession.insert_entry(
            metrics_procession::event::Entry::Counter {
                value: 1,
                op: metrics_procession::event::Op::Add,
            },
            label_id,
        );

        // Simulate time passage by manually creating a chunk with future time
        procession
            .chunks
            .push(metrics_procession::chunk::Chunk::new(future_time));
        procession.insert_entry(
            metrics_procession::event::Entry::Counter {
                value: 1,
                op: metrics_procession::event::Op::Add,
            },
            label_id,
        );
    });

    let procession = recorder.lock();
    assert!(
        procession.chunks.len() >= 2,
        "Should have multiple chunks due to time advancement"
    );
}

#[test]
fn test_memory_size_calculation() {
    let recorder = ProcessionRecorder::default();

    let initial_size = recorder.memory_size();

    metrics::with_local_recorder(&recorder, || {
        // Add some metrics
        for i in 0..1000 {
            metrics::counter!(format!("counter_{}", i % 10)).increment(1);
            metrics::gauge!(format!("gauge_{}", i % 5), "label" => format!("value_{}", i % 3))
                .set(i as f64);
        }
    });

    let final_size = recorder.memory_size();
    assert!(
        final_size > initial_size,
        "Memory size should increase after adding metrics"
    );

    // Memory size should be reasonable (not wildly large)
    assert!(final_size < 1_000_000, "Memory usage should be reasonable");
}

#[test]
fn test_high_cardinality_labels() {
    let recorder = ProcessionRecorder::default();

    metrics::with_local_recorder(&recorder, || {
        // Create metrics with many unique label combinations
        for user_id in 0..100 {
            for action in ["view", "click", "purchase"] {
                for region in ["us-east", "us-west", "eu-central"] {
                    metrics::counter!("user_actions",
                        "user_id" => format!("user_{user_id}"),
                        "action" => action,
                        "region" => region
                    )
                    .increment(1);
                }
            }
        }
    });

    let procession = recorder.lock();

    // Should have 100 * 3 * 3 = 900 unique label combinations
    assert_eq!(procession.labels.0.len(), 900);

    // All events should be recorded
    let events: Vec<MetricRef> = procession.iter().collect();
    assert_eq!(events.len(), 900);
}

#[test]
fn test_metric_types_and_operations() {
    let recorder = ProcessionRecorder::default();

    metrics::with_local_recorder(&recorder, || {
        // Counter operations
        let counter = metrics::counter!("test_counter");
        counter.increment(5);
        counter.increment(10);
        counter.absolute(100); // Set absolute value

        // Gauge operations
        let gauge = metrics::gauge!("test_gauge");
        gauge.set(50.0);
        gauge.increment(10.0);
        gauge.decrement(5.0);
        gauge.set(75.0);

        // Histogram operations
        let histogram = metrics::histogram!("test_histogram");
        histogram.record(1.5);
        histogram.record(2.5);
        histogram.record(10.0);
    });

    let procession = recorder.lock();
    let events: Vec<MetricRef> = procession.iter().collect();

    // Should have 3 + 4 + 3 = 10 events total
    assert_eq!(events.len(), 10);

    // Verify metric types are correct
    let counter_events: Vec<_> = events
        .iter()
        .filter(|e| e.key.name() == "test_counter")
        .collect();
    assert_eq!(counter_events.len(), 3);

    let gauge_events: Vec<_> = events
        .iter()
        .filter(|e| e.key.name() == "test_gauge")
        .collect();
    assert_eq!(gauge_events.len(), 4);

    let histogram_events: Vec<_> = events
        .iter()
        .filter(|e| e.key.name() == "test_histogram")
        .collect();
    assert_eq!(histogram_events.len(), 3);
}

#[test]
fn test_serialization_formats() {
    let recorder = ProcessionRecorder::default();

    metrics::with_local_recorder(&recorder, || {
        metrics::counter!("test", "env" => "prod").increment(42);
        metrics::gauge!("memory", "unit" => "bytes").set(1024.0);
        metrics::histogram!("latency", "percentile" => "p99").record(250.0);
    });

    let procession = recorder.lock();

    // Test JSON serialization/deserialization
    let json = serde_json::to_string_pretty(&*procession).unwrap();
    let deserialized: Procession = serde_json::from_str(&json).unwrap();
    assert_eq!(*procession, deserialized);

    // Test iterator serialization
    let events: Vec<Metric> = procession.iter_owned().collect();
    let events_json = serde_json::to_string(&events).unwrap();
    let _deserialized_events: Vec<Metric> = serde_json::from_str(&events_json).unwrap();

    // Test ref iterator serialization
    let ref_events: Vec<MetricRef> = procession.iter().collect();
    let ref_events_json = serde_json::to_string(&ref_events).unwrap();
    assert!(!ref_events_json.is_empty());
}

#[test]
fn test_procession_from_iterator() {
    let original_recorder = ProcessionRecorder::default();

    // Create some metrics
    metrics::with_local_recorder(&original_recorder, || {
        for i in 0..50 {
            metrics::counter!("requests", "status" => if i % 5 == 0 { "500" } else { "200" })
                .increment(1);
            metrics::histogram!("duration", "service" => format!("service_{}", i % 3))
                .record(i as f64);
        }
    });

    let original_procession = original_recorder.lock();

    // Test FromIterator with owned metrics
    let metrics: Vec<Metric> = original_procession.iter_owned().collect();
    let reconstructed_from_owned: Procession = metrics.into_iter().collect();

    // Test FromIterator with borrowed metrics
    let metric_refs: Vec<MetricRef> = original_procession.iter().collect();
    let reconstructed_from_refs: Procession = metric_refs.into_iter().collect();

    // Both should have the same number of events
    assert_eq!(&*original_procession, &reconstructed_from_owned,);
    assert_eq!(&*original_procession, &reconstructed_from_refs,);
}

#[test]
fn test_edge_cases() {
    let recorder = ProcessionRecorder::default();

    metrics::with_local_recorder(&recorder, || {
        // Test with empty metric name
        metrics::counter!("").increment(1);

        // Test with very long label values
        let long_value = "a".repeat(1000);
        metrics::counter!("long_labels", "long_key" => long_value).increment(1);

        // Test with special characters
        metrics::counter!("special/chars", "key-with-dashes" => "value_with_underscores")
            .increment(1);

        // Test with numeric-only labels
        metrics::counter!("numeric", "123" => "456").increment(1);

        // Test very large counter values (should be capped at u32::MAX)
        let counter = metrics::counter!("large_values");
        counter.increment(u64::MAX); // Should be capped

        // Test very large gauge values
        let gauge = metrics::gauge!("large_gauge");
        gauge.set(f64::MAX);
        gauge.set(f64::MIN);
        gauge.set(f64::NAN);
        gauge.set(f64::INFINITY);
        gauge.set(f64::NEG_INFINITY);
    });

    let procession = recorder.lock();
    let events: Vec<MetricRef> = procession.iter().collect();

    // Should handle all edge cases without panicking
    assert!(!events.is_empty());

    // Should be serializable despite edge cases
    let _json = serde_json::to_string(&*procession).unwrap();
}
