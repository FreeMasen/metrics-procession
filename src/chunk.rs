use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::event::Event;

/// A chunk of metrics that represents all events emitted from the `reference_time`
/// through 65 seconds after that reference time.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// The start time of this chunk
    pub reference_time: OffsetDateTime,
    /// The events that have happened within 65 seconds of the reference time
    pub events: Vec<Event>,
}

impl Chunk {
    /// Create a new chunk from the provided time
    pub fn new(reference_time: OffsetDateTime) -> Self {
        Self {
            reference_time,
            events: Default::default(),
        }
    }

    /// Add a new event into this chunk
    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    /// A naive method for trying to determine the total memory size used by this chunk
    pub fn memory_size(&self) -> usize {
        use std::mem::size_of;
        size_of::<Self>() + (self.events.len() * (size_of::<Event>()))
    }
}

/// Create a new chunk with the reference time being [`time::OffsetDateTime::now_utc()`]
impl Default for Chunk {
    fn default() -> Self {
        Self::new(OffsetDateTime::now_utc())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Entry, Event, Op};
    use time::{Date, Duration, Time};

    #[test]
    fn test_chunk_creation() {
        let reference_time = OffsetDateTime::new_utc(
            Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
            Time::from_hms(12, 0, 0).unwrap(),
        );

        let chunk = Chunk::new(reference_time);
        assert_eq!(chunk.reference_time, reference_time);
        assert!(chunk.events.is_empty());
    }

    #[test]
    fn test_chunk_default() {
        let chunk = Chunk::default();
        assert!(chunk.events.is_empty());
        // Reference time should be close to now (within 1 second)
        let now = OffsetDateTime::now_utc();
        let diff = (now - chunk.reference_time).abs();
        assert!(diff < Duration::seconds(1));
    }

    #[test]
    fn test_push_events() {
        let mut chunk = Chunk::default();

        let event1 = Event {
            entry: Entry::Counter {
                value: 1,
                op: Op::Add,
            },
            ms: 100,
            label: 1,
        };

        let event2 = Event {
            entry: Entry::Gauge {
                value: 42.5,
                op: Op::Set,
            },
            ms: 200,
            label: 2,
        };

        chunk.push(event1);
        chunk.push(event2);

        assert_eq!(chunk.events.len(), 2);
        assert_eq!(chunk.events[0], event1);
        assert_eq!(chunk.events[1], event2);
    }

    #[test]
    fn test_memory_size_calculation() {
        let mut chunk = Chunk::default();
        let initial_size = chunk.memory_size();

        // Add some events
        for i in 0..100 {
            chunk.push(Event {
                entry: Entry::Counter {
                    value: i,
                    op: Op::Add,
                },
                ms: i as u16,
                label: i as u16,
            });
        }

        let final_size = chunk.memory_size();
        assert!(final_size > initial_size);

        // Size should be roughly predictable
        let expected_size = std::mem::size_of::<Chunk>() + (100 * std::mem::size_of::<Event>());
        assert_eq!(final_size, expected_size);
    }

    #[test]
    fn test_chunk_serialization() {
        let mut chunk = Chunk::default();
        chunk.push(Event {
            entry: Entry::Histogram { value: 123.45 },
            ms: 500,
            label: 10,
        });

        // Test JSON serialization
        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: Chunk = serde_json::from_str(&json).unwrap();
        assert_eq!(chunk, deserialized);
    }
}
