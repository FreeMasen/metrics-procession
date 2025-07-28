use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use metrics_procession::recorder::ProcessionRecorder;
use std::hint::black_box;

static K: u64 = 1024;
static SIZES: &[u64] = &[K, 2 * K, 4 * K, 8 * K, 16 * K];

macro_rules! bench_inner {
    ($c:ident, $grp:literal, $ct:ident, $size:ident, $ctor:expr, $met_name:ident $loop_:tt) => {{
        let mut group = $c.benchmark_group($grp);
        for &size in SIZES.iter() {
            group.throughput(Throughput::Elements(size));
            group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &$size| {
                b.iter_custom(|$ct| {
                    let recorder = ProcessionRecorder::default();
                    let dur = metrics::with_local_recorder(&recorder, move || {
                        let $met_name = $ctor;
                        let start = std::time::Instant::now();
                        {
                            $loop_
                        }
                        start.elapsed()
                    });
                    black_box(recorder.memory_size());
                    dur
                });
            });
        }
    }};
}

pub fn counters(c: &mut Criterion) {
    bench_inner!(c, "no-label-counter", ct, size, metrics::counter!("no-label"), met {
        for _ in 0..ct*size {
            met.increment(1);
        }
    });
    bench_inner!(c, "one-label-counter-static", ct, size, metrics::counter!("one-label", "label" => "value"), met {
        for _ in 0..ct*size {
            met.increment(1);
        }
    });
    bench_inner!(c, "two-label-counter-static", ct, size, metrics::counter!("two-label", "label" => "value", "other-label" => "other-value"), met {
        for _ in 0..ct*size {
            met.increment(1);
        }
    });
    bench_inner!(c, "one-label-counter-dynamic", ct, size, (), _m {
        for _ in 0..ct*size {
            metrics::counter!("two-label", "label" => (ct * size).to_string()).increment(1);
        }
    });
    bench_inner!(c, "two-label-counter-dynamic", ct, size, (), _m {
        for _ in 0..ct*size {
            metrics::counter!("two-label", "label" => (ct * size).to_string(), "other-label" => ((ct * size) % 1024).to_string()).increment(1);
        }
    });
}

pub fn gauges(c: &mut Criterion) {
    bench_inner!(c, "no-label-gauge-increment", ct, size, metrics::gauge!("no-label"), met {
        for _ in 0..ct*size {
            met.increment(1.0);
        }
    });
    bench_inner!(c, "no-label-gauge-set", ct, size, metrics::gauge!("no-label"), met {
        for v in 0..ct*size {
            met.set(v as f64);
        }
    });
    bench_inner!(c, "one-label-gauge-increment", ct, size, metrics::gauge!("no-label", "label" => "value"), met {
        for _ in 0..ct*size {
            met.increment(1.0);
        }
    });
    bench_inner!(c, "one-label-gauge-set", ct, size, metrics::gauge!("no-label", "label" => "value"), met {
        for v in 0..ct*size {
            met.set(v as f64);
        }
    });
    bench_inner!(c, "two-label-gauge-increment", ct, size, metrics::gauge!("two-label", "label" => "value", "other-label" => "other-value"), met {
        for _ in 0..ct*size {
            met.increment(1.0);
        }
    });
    bench_inner!(c, "two-label-gauge-set", ct, size, metrics::gauge!("two-label", "label" => "value", "other-label" => "other-value"), met {
        for v in 0..ct*size {
            met.set(v as f64);
        }
    });
}

pub fn histograms(c: &mut Criterion) {
    bench_inner!(c, "no-label-histo", ct, size, metrics::histogram!("no-label"), met {
        for v in 0..ct*size {
            met.record(v as f64);
        }
    });
    bench_inner!(c, "one-label-histo-static", ct, size, metrics::histogram!("one-label", "label" => "value"), met {
        for v in 0..ct*size {
            met.record(v as f64);
        }
    });
    bench_inner!(c, "two-label-histo-static", ct, size, metrics::histogram!("two-label", "label" => "value", "other-label" => "other-value"), met {
        for v in 0..ct*size {
            met.record(v as f64);
        }
    });
    bench_inner!(c, "one-label-histo-dynamic", ct, size, (), _m {
        for v in 0..ct*size {
            metrics::histogram!("two-label", "label" => (ct * size).to_string()).record(v as f64);
        }
    });
    bench_inner!(c, "two-label-histo-dynamic", ct, size, (), _m {
        for v in 0..ct*size {
            metrics::histogram!("two-label", "label" => (ct * size).to_string(), "other-label" => ((ct * size) % 1024).to_string()).record(v as f64);
        }
    });
}

criterion_group!(benches, counters, gauges, histograms);
criterion_main!(benches);
