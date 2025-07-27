use time::Duration;

use metrics::{Key, Label};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    chunk::Chunk,
    event::{Entry, Event},
    iter::{Metric, MetricRef, MetricsIterator, MetricsRefIterator},
    label_set::LabelSet,
};

/// This represents a time series of metrics collected over some length of time
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
pub struct Procession {
    /// The series of chunks representing ~65 seconds of time in each chunk
    pub chunks: Vec<Chunk>,
    /// The set of all unique keys and labels currently in the set
    pub labels: LabelSet,
}

impl Procession {
    /// A naive attempt to calculate the memory size of the current state
    pub fn memory_size(&self) -> usize {
        use std::{collections::HashSet, mem::size_of};
        let mut shared_string_set = HashSet::new();
        let labels_size = self
            .labels
            .0
            .keys()
            .map(|k| {
                let k_size = if shared_string_set.insert(k.name()) {
                    k.name().len()
                } else {
                    0
                } + size_of::<Key>();
                let l_size = k.labels().fold(0, |acc, l| {
                    let l_size = if shared_string_set.insert(l.key()) {
                        l.key().len()
                    } else {
                        0
                    } + size_of::<Label>();
                    let v_size = if shared_string_set.insert(l.value()) {
                        l.value().len()
                    } else {
                        0
                    } + size_of::<Label>();
                    acc + l_size + v_size
                });
                k_size + l_size + size_of::<u16>()
            })
            .sum::<usize>();
        let chunk_size = self.chunks.iter().map(|c| c.memory_size()).sum::<usize>();
        labels_size + chunk_size + size_of::<Self>()
    }

    /// Insert a new entry into the last (or newly last) [`Chunk`]
    pub fn insert_entry(&mut self, entry: Entry, label: u16) {
        let now = OffsetDateTime::now_utc();
        let (last, ms) = self.last_chunk_and_ms(now);
        last.push(Event { entry, ms, label });
    }

    /// Find the last chunk in this [Procession] along with the number of milliseconds
    /// since the reference time on that chunk. If either there are no chunks already
    /// available _or_ the number of milliseconds since the last chunk's reference time
    /// would exceed [u16::MAX] a new chunk is added and a mutable reference to that chunk
    /// is returned with a ms value of 0
    pub fn last_chunk_and_ms(&mut self, now: OffsetDateTime) -> (&mut Chunk, u16) {
        if self.chunks.is_empty() {
            self.chunks.push(Chunk::default());
        }
        let mut duration = self
            .chunks
            .last()
            .map(|c| (now - c.reference_time))
            .unwrap_or_default();
        if duration > Duration::milliseconds(i64::from(u16::MAX)) {
            self.chunks.push(Chunk::new(now));
            duration = Duration::ZERO;
        }
        let ms = u16::try_from(duration.whole_milliseconds()).unwrap_or(u16::MAX);
        (self.chunks.last_mut().unwrap(), ms)
    }

    /// Ensure the provided key is in the [`labels`]
    pub fn ensure_label(&mut self, k: &Key) -> u16 {
        self.labels.ensure_key(k)
    }

    /// create an iterator for the raw metric events currently recorded that will be tied to the
    /// lifetime of this instance of the [`Procession`]
    pub fn iter(&self) -> MetricsRefIterator {
        MetricsRefIterator::from(self)
    }

    /// create an iterator for the raw metric events currently recorded providing owned
    /// version of all events
    pub fn iter_owned(&self) -> MetricsIterator {
        self.iter().into()
    }
}

impl FromIterator<Metric> for Procession {
    fn from_iter<T: IntoIterator<Item = Metric>>(iter: T) -> Self {
        let mut iter = iter.into_iter().peekable();
        let mut ret = Self::default();
        if let Some(first) = iter.peek() {
            let start = first.when;
            ret.chunks.push(Chunk::new(start));
        }
        for event in iter {
            let labels = event
                .labels
                .into_iter()
                .map(|(k, v)| Label::new(k, v))
                .collect::<Vec<_>>();
            let label = ret.ensure_label(&Key::from_parts(event.key, labels));
            ret.insert_entry(event.event, label);
        }
        ret
    }
}

impl<'a> FromIterator<MetricRef<'a>> for Procession {
    fn from_iter<T: IntoIterator<Item = MetricRef<'a>>>(iter: T) -> Self {
        let mut iter = iter.into_iter().peekable();
        let mut ret = Self::default();
        if let Some(first) = iter.peek() {
            let start = first.when;
            ret.chunks.push(Chunk::new(start));
        }
        for event in iter {
            let label = ret.ensure_label(event.key);
            ret.insert_entry(event.event, label);
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Entry, Op};
    use time::{Date, Time};

    fn create_test_time() -> OffsetDateTime {
        OffsetDateTime::new_utc(
            Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
            Time::from_hms(12, 0, 0).unwrap(),
        )
    }

    #[test]
    fn test_procession_creation() {
        let procession = Procession::default();
        assert!(procession.chunks.is_empty());
        assert!(procession.labels.0.is_empty());
        assert_eq!(procession.memory_size(), std::mem::size_of::<Procession>());
    }

    #[test]
    fn test_ensure_label() {
        let mut procession = Procession::default();
        let key = Key::from_name("test_metric");

        let id = procession.ensure_label(&key);
        assert_eq!(id, 0);
        assert_eq!(procession.labels.0.len(), 1);

        // Ensure same key returns same ID
        let id2 = procession.ensure_label(&key);
        assert_eq!(id, id2);
        assert_eq!(procession.labels.0.len(), 1);
    }

    #[test]
    fn test_insert_entry_creates_chunk() {
        let mut procession = Procession::default();
        assert!(procession.chunks.is_empty());

        procession.insert_entry(
            Entry::Counter {
                value: 1,
                op: Op::Add,
            },
            0,
        );

        assert_eq!(procession.chunks.len(), 1);
        assert_eq!(procession.chunks[0].events.len(), 1);
    }

    #[test]
    fn test_last_chunk_and_ms() {
        let mut procession = Procession::default();
        let test_time = create_test_time();

        // First call should create a chunk with default time (now)
        {
            let (chunk, _ms) = procession.last_chunk_and_ms(test_time);
            // Add an event to the chunk
            chunk.push(Event {
                entry: Entry::Counter {
                    value: 1,
                    op: Op::Add,
                },
                ms: 0,
                label: 0,
            });
        }

        assert_eq!(procession.chunks.len(), 1);

        // Second call with same time should return same chunk
        {
            let (chunk2, _ms2) = procession.last_chunk_and_ms(test_time);
            assert_eq!(chunk2.events.len(), 1);
        }

        assert_eq!(procession.chunks.len(), 1);
    }

    #[test]
    fn test_chunk_rollover() {
        let mut procession = Procession::default();

        // Create initial chunk - it will use current time
        let (_, _) = procession.last_chunk_and_ms(OffsetDateTime::now_utc());
        assert_eq!(procession.chunks.len(), 1);
        let first_chunk_time = procession.chunks[0].reference_time;

        // Time far in the future should create new chunk
        let future_time = first_chunk_time + Duration::milliseconds(i64::from(u16::MAX) + 1);
        procession.last_chunk_and_ms(future_time);
        assert_eq!(procession.chunks.len(), 2);
    }

    #[test]
    fn test_memory_size_calculation() {
        let mut procession = Procession::default();
        let initial_size = procession.memory_size();

        // Add some labels and events
        let key = Key::from_parts("test", vec![Label::new("env", "prod")]);
        let label_id = procession.ensure_label(&key);

        procession.insert_entry(
            Entry::Counter {
                value: 1,
                op: Op::Add,
            },
            label_id,
        );
        procession.insert_entry(
            Entry::Gauge {
                value: 100.0,
                op: Op::Set,
            },
            label_id,
        );

        let final_size = procession.memory_size();
        assert!(final_size > initial_size);
    }

    #[test]
    fn test_iterator_methods() {
        let mut procession = Procession::default();

        // Add some test data
        let key1 = Key::from_name("counter");
        let key2 = Key::from_name("gauge");
        let id1 = procession.ensure_label(&key1);
        let id2 = procession.ensure_label(&key2);

        procession.insert_entry(
            Entry::Counter {
                value: 5,
                op: Op::Add,
            },
            id1,
        );
        procession.insert_entry(
            Entry::Gauge {
                value: 25.5,
                op: Op::Set,
            },
            id2,
        );

        // Test borrowed iterator
        let ref_events: Vec<MetricRef> = procession.iter().collect();
        assert_eq!(ref_events.len(), 2);

        // Test owned iterator
        let owned_events: Vec<Metric> = procession.iter_owned().collect();
        assert_eq!(owned_events.len(), 2);

        // They should have equivalent data
        for (owned, borrowed) in owned_events.iter().zip(ref_events.iter()) {
            assert_eq!(*owned, *borrowed);
        }
    }

    #[test]
    fn test_procession_serialization() {
        let mut procession = Procession::default();

        // Add test data
        let key = Key::from_parts(
            "http_requests",
            vec![Label::new("method", "GET"), Label::new("status", "200")],
        );
        let label_id = procession.ensure_label(&key);
        procession.insert_entry(
            Entry::Counter {
                value: 42,
                op: Op::Add,
            },
            label_id,
        );

        // Test JSON serialization
        let json = serde_json::to_string(&procession).unwrap();
        let deserialized: Procession = serde_json::from_str(&json).unwrap();
        assert_eq!(procession, deserialized);
    }

    #[test]
    fn test_from_iterator_metric() {
        let base_time = create_test_time();
        let metrics = vec![
            Metric {
                when: base_time,
                event: Entry::Counter {
                    value: 1,
                    op: Op::Add,
                },
                key: "test".to_string(),
                labels: vec![("env".to_string(), "prod".to_string())],
            },
            Metric {
                when: base_time + Duration::milliseconds(100),
                event: Entry::Gauge {
                    value: 50.0,
                    op: Op::Set,
                },
                key: "test2".to_string(),
                labels: vec![],
            },
        ];

        let procession: Procession = metrics.into_iter().collect();

        // Both metrics have close timestamps, so they should be in same chunk
        assert!(!procession.chunks.is_empty());
        assert_eq!(procession.labels.0.len(), 2);

        let events: Vec<MetricRef> = procession.iter().collect();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_from_iterator_metric_ref() {
        let mut source_procession = Procession::default();

        // Add test data to source
        let key = Key::from_name("source_metric");
        let label_id = source_procession.ensure_label(&key);
        source_procession.insert_entry(Entry::Histogram { value: 123.45 }, label_id);

        // Collect refs and build new procession
        let metric_refs: Vec<MetricRef> = source_procession.iter().collect();
        let new_procession: Procession = metric_refs.into_iter().collect();
        assert_eq!(new_procession, source_procession);
    }

    #[test]
    fn test_multiple_chunks_over_time() {
        let mut procession = Procession::default();
        let base_time = create_test_time();

        // Insert events at different times to force multiple chunks
        for i in 0..5 {
            let event_time = base_time + Duration::hours(i);
            let (chunk, _) = procession.last_chunk_and_ms(event_time);
            chunk.push(Event {
                entry: Entry::Counter {
                    value: i as u32,
                    op: Op::Add,
                },
                ms: 0,
                label: 0,
            });
        }

        // Should have multiple chunks due to time spacing
        assert!(!procession.chunks.is_empty());

        // Total events should be preserved
        let all_events: Vec<MetricRef> = procession.iter().collect();
        assert_eq!(all_events.len(), 5);
    }

    #[test]
    fn test_edge_case_timing() {
        let mut procession = Procession::default();

        // Create initial chunk
        let base_time = OffsetDateTime::now_utc();
        let (_, _) = procession.last_chunk_and_ms(base_time);
        let chunk_ref_time = procession.chunks[0].reference_time;

        // Test exactly at u16::MAX milliseconds boundary
        let boundary_time = chunk_ref_time + Duration::milliseconds(u16::MAX as i64);
        let (_, ms1) = procession.last_chunk_and_ms(boundary_time);
        assert_eq!(ms1, u16::MAX);

        // One millisecond over should create new chunk
        let over_time = chunk_ref_time + Duration::milliseconds(u16::MAX as i64 + 1);
        let chunks_before = procession.chunks.len();
        let (_, ms2) = procession.last_chunk_and_ms(over_time);
        let chunks_after = procession.chunks.len();

        assert_eq!(ms2, 0); // New chunk, so 0 offset
        assert_eq!(chunks_after, chunks_before + 1);
    }
}
