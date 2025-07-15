use serde::{Deserialize, Serialize};

/// A single metrics event represented in the most compact form
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct Event {
    /// The type and value of this event
    pub entry: Entry,
    /// The number of milliseconds since the owning [`crate::chunk::Chunk`]'s `reference_time`
    pub ms: u16,
    /// The label identifier from the owning [`crate::Procession`]'s [`crate::label_set::LabelSet`]
    pub label: u16,
}

/// A raw metrics event representing the type and value of what was emitted by the instrumentation
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum Entry {
    /// A gauge event
    Gauge { value: f32, op: Op },
    /// A counter event
    Counter { value: u32, op: Op },
    /// A histogram event
    Histogram { value: f32 },
}

/// An [`Entry`]'s operation, used for handling the `increment` and `set` methods
/// of [`metrics::CounterFn`] and [`metrics::GaugeFn`] or the `decrement` method of a gauge
#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Set,
}
