use std::sync::{Arc, Mutex, MutexGuard};

use metrics::{CounterFn, GaugeFn, HistogramFn, Recorder};

use crate::{
    event::{Entry, Op},
    procession::Procession,
};

#[derive(Debug, Clone, Default)]
pub struct ProcessionRecorder(Arc<Mutex<Procession>>);

impl ProcessionRecorder {
    pub fn lock(&self) -> MutexGuard<Procession> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn memory_size(&self) -> usize {
        self.0.lock().unwrap().memory_size()
    }
}

impl Recorder for ProcessionRecorder {
    fn describe_counter(
        &self,
        _: metrics::KeyName,
        _: Option<metrics::Unit>,
        _: metrics::SharedString,
    ) {
    }

    fn describe_gauge(
        &self,
        _: metrics::KeyName,
        _: Option<metrics::Unit>,
        _: metrics::SharedString,
    ) {
    }

    fn describe_histogram(
        &self,
        _: metrics::KeyName,
        _: Option<metrics::Unit>,
        _: metrics::SharedString,
    ) {
    }

    fn register_counter(&self, key: &metrics::Key, _: &metrics::Metadata<'_>) -> metrics::Counter {
        let label = self.0.lock().unwrap().ensure_label(key);
        metrics::Counter::from_arc(Arc::new(Counter(label, self.clone())))
    }

    fn register_gauge(&self, key: &metrics::Key, _: &metrics::Metadata<'_>) -> metrics::Gauge {
        let label = self.0.lock().unwrap().ensure_label(key);
        metrics::Gauge::from_arc(Arc::new(Gauge(label, self.clone())))
    }

    fn register_histogram(
        &self,
        key: &metrics::Key,
        _: &metrics::Metadata<'_>,
    ) -> metrics::Histogram {
        let label = self.0.lock().unwrap().ensure_label(key);
        metrics::Histogram::from_arc(Arc::new(Histo(label, self.clone())))
    }
}

struct Counter(u16, ProcessionRecorder);

impl CounterFn for Counter {
    fn increment(&self, value: u64) {
        self.insert(value, Op::Add);
    }

    fn absolute(&self, value: u64) {
        self.insert(value, Op::Set);
    }
}

impl Counter {
    pub fn insert(&self, value: u64, op: Op) {
        let Ok(value) = u32::try_from(value) else {
            log::warn!("value has exceeded a u32, skipping event");
            return;
        };
        self.1
            .0
            .lock()
            .unwrap()
            .insert_entry(Entry::Counter { value, op }, self.0);
    }
}

struct Gauge(u16, ProcessionRecorder);

impl GaugeFn for Gauge {
    fn increment(&self, value: f64) {
        self.1.0.lock().unwrap().insert_entry(
            Entry::Gauge {
                value: value as f32,
                op: Op::Add,
            },
            self.0,
        );
    }

    fn decrement(&self, value: f64) {
        self.1.0.lock().unwrap().insert_entry(
            Entry::Gauge {
                value: value as f32,
                op: Op::Sub,
            },
            self.0,
        );
    }

    fn set(&self, value: f64) {
        self.1.0.lock().unwrap().insert_entry(
            Entry::Gauge {
                value: value as f32,
                op: Op::Set,
            },
            self.0,
        );
    }
}

struct Histo(u16, ProcessionRecorder);

impl HistogramFn for Histo {
    fn record(&self, value: f64) {
        self.1.0.lock().unwrap().insert_entry(
            Entry::Histogram {
                value: value as f32,
            },
            self.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_emit() {
        let recorder = ProcessionRecorder::default();
        metrics::with_local_recorder(&recorder, || {
            let ct = metrics::counter!("one_counter");
            let ct2 = metrics::counter!("with_label", "label" => "this-one");
            let g = metrics::gauge!("one_gauge");
            let g2 = metrics::gauge!("with_labels", "label1" => "value1", "label2" => "value2");
            let h = metrics::histogram!("one_histo");
            let h2 = metrics::histogram!("with_label", "tid" => format!("{:?}", std::thread::current().id()));

            for i in 0..1000 {
                ct.increment(1);
                ct2.increment(2);
                g.set(i as f64);
                g2.set((i / 2) as f64);
                h.record(i as f64);
                h2.record((i / 2) as f64);
            }
        });
    }
}
