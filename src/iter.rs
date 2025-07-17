//! This module is responsible for creating iterators from a [`crate::Procession`]
use std::sync::OnceLock;

use metrics::Key;
use serde::{
    Deserialize, Serialize,
    ser::{SerializeMap, SerializeSeq},
};
use time::{Duration, OffsetDateTime};

/// Only used in cases of an emergency, when a [`metrics::Key`] can somehow be lost when
/// attempting to create a [`Metric`]
static EMPTY_KEY: OnceLock<Key> = OnceLock::new();

use crate::{
    chunk::Chunk,
    event::{Entry, Event},
    procession::Procession,
};

/// A single event cloned out of the [Procession], this representation will
/// allocation the strings needed to represent the value w/o holding a reference
/// the time [Procession] itself. This type can be serialized and deserialized
/// and represents and "owned" version of the [MetricRef] type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub when: OffsetDateTime,
    pub event: Entry,
    pub key: String,
    pub labels: Vec<(String, String)>,
}

/// A single event borrowed from the [Procession], this representation
/// will not cause any additional allocations and can be serialized, the
/// timestamp is re-calculated as part of the construction but no other
/// computation should occur.
#[derive(Debug)]
pub struct MetricRef<'a> {
    /// The time this event occurred
    pub when: OffsetDateTime,
    /// The value emitted for the key
    pub event: Entry,
    /// The key and labels provided by the metrics crate
    pub key: &'a Key,
}

impl Serialize for MetricRef<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut m = serializer.serialize_map(Some(4))?;
        m.serialize_entry("when", &self.when)?;
        m.serialize_entry("event", &self.event)?;
        m.serialize_entry("key", &self.key.name())?;
        m.serialize_entry("labels", &LabelsSet(self.key))?;
        m.end()
    }
}

/// Helper for serializing/deserializing the key type
struct LabelsSet<'a>(&'a Key);

impl Serialize for LabelsSet<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let labels = self.0.labels();
        let mut s = serializer.serialize_seq(Some(labels.len()))?;
        for label in labels {
            s.serialize_element(&(label.key(), label.value()))?;
        }
        s.end()
    }
}

/// An iterator that will clone values out of the source [`Procession`]
///
/// warning: this will re-allocate all of the [`String`]s from the [`metrics::Key`]
/// type potentially many times.
pub struct MetricsIterator<'a>(MetricsRefIterator<'a>);

impl<'a> From<MetricsRefIterator<'a>> for MetricsIterator<'a> {
    fn from(value: MetricsRefIterator<'a>) -> Self {
        Self(value)
    }
}

impl<'a> From<&'a Procession> for MetricsRefIterator<'a> {
    fn from(value: &'a Procession) -> Self {
        Self {
            stream: value,
            chunk_index: 0,
            event_index: 0,
        }
    }
}
impl<'a> From<&'a Procession> for MetricsIterator<'a> {
    fn from(value: &'a Procession) -> Self {
        Self(MetricsRefIterator::from(value))
    }
}

impl Iterator for MetricsIterator<'_> {
    type Item = Metric;
    fn next(&mut self) -> Option<Self::Item> {
        let MetricRef { when, event, key } = self.0.next()?;
        Some(Metric {
            when,
            event,
            key: key.name().to_string(),
            labels: key
                .labels()
                .map(|l| (l.key().to_string(), l.value().to_string()))
                .collect(),
        })
    }
}

/// An iterator that will borrow values from the owning [`Procession`], unlike the [`MetricsIterator`]
/// this will not perform any reallocations but can be safely `collect`ed, as long as the underlying
/// [`Procession`] is not dropped, and serialized
pub struct MetricsRefIterator<'a> {
    stream: &'a Procession,
    chunk_index: usize,
    event_index: usize,
}

impl<'a> Iterator for MetricsRefIterator<'a> {
    type Item = MetricRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (event, chunk) = self.get_next_event()?;
        let when = chunk.reference_time + Duration::milliseconds(event.ms as i64);
        let Some(key) = self.stream.labels.0.iter().find_map(|(k, v)| {
            if *v == event.label {
                return Some(k);
            }
            None
        }) else {
            return Some(MetricRef {
                when,
                event: event.entry,
                key: EMPTY_KEY.get_or_init(|| Key::from_name("")),
            });
        };
        Some(MetricRef {
            when,
            event: event.entry,
            key,
        })
    }
}

impl<'a> MetricsRefIterator<'a> {
    /// This method will do the majority of the work needed by the `next` implementation
    /// above. The returned [`Event`] represents the correct value that should come
    /// next in the series but since it only contains the millisecond count since its owning
    /// [`Chunk`]'s `reference_time` we will also return the correct [`Chunk`]
    ///
    /// This method will also handle the index management for the calculation of the next event
    /// we should emit. If the current chunk is exhausted, it will reset the `event_index` and
    /// increment the `chunk_index`, otherwise it will increment the `event_index` only
    fn get_next_event<'s, 'r>(&'s mut self) -> Option<(&'r Event, &'r Chunk)>
    where
        'a: 'r,
    {
        let mut chunk = self.stream.chunks.get(self.chunk_index)?;
        if let Some(event) = chunk.events.get(self.event_index) {
            self.event_index += 1;
            return Some((event, chunk));
        }
        self.chunk_index += 1;
        self.event_index = 0;
        chunk = self.stream.chunks.get(self.chunk_index)?;
        let ret = chunk.events.get(self.event_index)?;
        self.event_index += 1;
        Some((ret, chunk))
    }
}

impl PartialEq<MetricRef<'_>> for Metric {
    fn eq(&self, other: &MetricRef) -> bool {
        self.when.eq(&other.when)
            && self.event.eq(&other.event)
            && self.key.eq(other.key.name())
            && self.labels.len() == other.key.labels().len()
            && self
                .labels
                .iter()
                .all(|(k, v)| other.key.labels().any(|l| k == l.key() && v == l.value()))
    }
}

impl PartialEq<Metric> for MetricRef<'_> {
    fn eq(&self, other: &Metric) -> bool {
        other.eq(self)
    }
}

#[cfg(test)]
mod tests {
    use metrics::{Key, Label};
    use time::{Date, Time};

    use crate::{event::Op, label_set::LabelSet};

    use super::*;

    #[test]
    fn iter_works_as_expected() {
        let time_stream = build_test_stream();
        let iter = MetricsIterator::from(&time_stream);
        let flattened: Vec<Metric> = iter.collect();
        insta::assert_json_snapshot!(flattened);
    }

    #[test]
    fn iter_ref_and_iter_match() {
        let time_stream = build_test_stream();
        let met_refs = MetricsRefIterator::from(&time_stream);
        let mets = MetricsIterator::from(&time_stream);
        for (l, r) in mets.zip(met_refs) {
            assert_eq!(l, r);
        }
    }

    fn build_test_stream() -> Procession {
        let start = OffsetDateTime::new_utc(
            Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
            Time::from_hms(0, 0, 0).unwrap(),
        );
        let mut labels = LabelSet([].into_iter().collect());
        let k1 = Key::from_name("no-labels");
        let mut raw_labels = Vec::new();
        raw_labels.push(labels.ensure_key(&k1));
        let k2 = Key::from_parts("one-label", vec![Label::new("label", "value")]);
        raw_labels.push(labels.ensure_key(&k2));
        let k3 = Key::from_parts(
            "two-labels",
            vec![
                Label::new("3label1", "value1"),
                Label::new("3label2", "value2"),
            ],
        );
        raw_labels.push(labels.ensure_key(&k3));
        let k4 = Key::from_parts(
            "three-labels",
            vec![
                Label::new("4label1", "value1"),
                Label::new("4label2", "value2"),
                Label::new("4label3", "value3"),
            ],
        );
        raw_labels.push(labels.ensure_key(&k4));
        let k5 = Key::from_parts(
            "three-labels",
            vec![
                Label::new("5label1", "value1"),
                Label::new("5label2", "value2"),
                Label::new("5label3", "value3"),
                Label::new("5label4", "value4"),
            ],
        );
        raw_labels.push(labels.ensure_key(&k5));

        let streams = (0..128)
            .map(|v| {
                let reference_time = start + Duration::minutes(v);
                let events = (0..128)
                    .map(|v| Event {
                        entry: Entry::Counter {
                            value: 1,
                            op: Op::Add,
                        },
                        ms: v as u16,
                        label: raw_labels[v % 5],
                    })
                    .collect();
                Chunk {
                    reference_time,
                    events,
                }
            })
            .collect();
        Procession {
            labels,
            chunks: streams,
        }
    }
}
