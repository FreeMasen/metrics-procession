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
        use std::{mem::size_of, collections::HashSet};
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
