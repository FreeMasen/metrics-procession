#![doc = include_str!("../README.md")]
pub mod chunk;
pub mod event;
pub mod iter;
pub mod label_set;
pub mod procession;
pub mod recorder;

#[cfg(test)]
mod tests {
    use metrics::{Key, Label};

    use crate::{
        chunk::Chunk,
        event::{Entry, Event, Op},
        label_set::LabelSet,
        procession::Procession,
    };

    #[test]
    fn ser_de() {
        let labels = LabelSet(
            [
                (
                    Key::from_parts("label1", vec![Label::new("key", "value")]),
                    1,
                ),
                (
                    Key::from_parts(
                        "label2",
                        vec![Label::new("key", "value"), Label::new("other", "value")],
                    ),
                    2,
                ),
                (Key::from_parts("label3", vec![]), 3),
            ]
            .into_iter()
            .collect(),
        );
        let streams = Procession {
            labels,
            chunks: vec![
                Chunk {
                    reference_time: time::OffsetDateTime::new_utc(
                        time::Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
                        time::Time::from_hms(0, 0, 0).unwrap(),
                    ),
                    events: vec![
                        Event {
                            entry: Entry::Counter {
                                value: 1,
                                op: Op::Add,
                            },
                            ms: 0,
                            label: 1,
                        },
                        Event {
                            entry: Entry::Gauge {
                                value: 1.0,
                                op: Op::Set,
                            },
                            ms: 1,
                            label: 2,
                        },
                        Event {
                            entry: Entry::Histogram { value: 1.0 },
                            ms: 2,
                            label: 3,
                        },
                    ],
                },
                Chunk {
                    reference_time: time::OffsetDateTime::new_utc(
                        time::Date::from_calendar_date(2025, time::Month::January, 1).unwrap(),
                        time::Time::from_hms(1, 0, 0).unwrap(),
                    ),
                    events: vec![
                        Event {
                            entry: Entry::Counter {
                                value: 1,
                                op: Op::Set,
                            },
                            ms: 0,
                            label: 1,
                        },
                        Event {
                            entry: Entry::Gauge {
                                value: 1.0,
                                op: Op::Add,
                            },
                            ms: 1,
                            label: 2,
                        },
                        Event {
                            entry: Entry::Histogram { value: 1.0 },
                            ms: 2,
                            label: 3,
                        },
                    ],
                },
            ],
        };
        let json = serde_json::to_string_pretty(&streams).unwrap();
        let back = serde_json::from_str(&json).unwrap();
        assert_eq!(streams, back);
    }
}
