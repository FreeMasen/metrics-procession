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
