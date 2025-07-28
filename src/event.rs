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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event {
            entry: Entry::Counter {
                value: 42,
                op: Op::Add,
            },
            ms: 1500,
            label: 123,
        };

        assert_eq!(event.ms, 1500);
        assert_eq!(event.label, 123);
        let Entry::Counter { value, op } = event.entry else {
            panic!("Expected counter entry")
        };
        assert_eq!(value, 42);
        assert_eq!(op, Op::Add);
    }

    #[test]
    fn test_entry_types() {
        // Test Counter entry
        let counter_entry = Entry::Counter {
            value: 100,
            op: Op::Set,
        };
        assert!(matches!(counter_entry, Entry::Counter { .. }));

        // Test Gauge entry
        let gauge_entry = Entry::Gauge {
            value: 25.5,
            op: Op::Sub,
        };
        assert!(matches!(gauge_entry, Entry::Gauge { .. }));

        // Test Histogram entry
        let histogram_entry = Entry::Histogram { value: 99.9 };
        assert!(matches!(histogram_entry, Entry::Histogram { .. }));
    }

    #[test]
    fn test_op_types() {
        assert_eq!(Op::Add, Op::Add);
        assert_eq!(Op::Sub, Op::Sub);
        assert_eq!(Op::Set, Op::Set);

        assert_ne!(Op::Add, Op::Sub);
        assert_ne!(Op::Sub, Op::Set);
        assert_ne!(Op::Set, Op::Add);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            entry: Entry::Gauge {
                value: std::f32::consts::PI,
                op: Op::Add,
            },
            ms: 2000,
            label: 456,
        };

        // Test JSON serialization
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_entry_serialization() {
        let entries = vec![
            Entry::Counter {
                value: 1,
                op: Op::Add,
            },
            Entry::Counter {
                value: 50,
                op: Op::Set,
            },
            Entry::Gauge {
                value: 100.0,
                op: Op::Sub,
            },
            Entry::Gauge {
                value: 0.0,
                op: Op::Set,
            },
            Entry::Histogram { value: 250.5 },
        ];

        for entry in entries {
            let json = serde_json::to_string(&entry).unwrap();
            let deserialized: Entry = serde_json::from_str(&json).unwrap();
            assert_eq!(entry, deserialized);
        }
    }

    #[test]
    fn test_op_serialization() {
        let ops = vec![Op::Add, Op::Sub, Op::Set];

        for op in ops {
            let json = serde_json::to_string(&op).unwrap();
            let deserialized: Op = serde_json::from_str(&json).unwrap();
            assert_eq!(op, deserialized);
        }
    }

    #[test]
    fn test_event_copy_trait() {
        let event = Event {
            entry: Entry::Histogram { value: 42.0 },
            ms: 1000,
            label: 1,
        };

        let copied_event = event;
        assert_eq!(event, copied_event);

        // Should still be able to use original after copy
        assert_eq!(event.ms, 1000);
    }

    #[test]
    fn test_extreme_values() {
        // Test with maximum values
        let max_event = Event {
            entry: Entry::Counter {
                value: u32::MAX,
                op: Op::Set,
            },
            ms: u16::MAX,
            label: u16::MAX,
        };

        let json = serde_json::to_string(&max_event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(max_event, deserialized);

        // Test with minimum values
        let min_event = Event {
            entry: Entry::Gauge {
                value: 0.0,
                op: Op::Add,
            },
            ms: 0,
            label: 0,
        };

        let json = serde_json::to_string(&min_event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(min_event, deserialized);
    }
}
