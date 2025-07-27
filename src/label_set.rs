// this module uses a hashmap with the `Key` type which shouldn't really
// change during its lifetime
#![allow(clippy::mutable_key_type)]
use std::collections::BTreeMap;

use metrics::Key;
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeSeq};

/// A set of labels mapping from the original [`metrics::Key`] to a unique identifier
/// and will be used to lookup what identifier to use when recording metrics events
#[derive(Debug, PartialEq, Clone, Default)]
pub struct LabelSet(pub BTreeMap<Key, u16>);

impl LabelSet {
    /// Get the identifier for the provided key
    pub fn get(&self, key: &Key) -> Option<u16> {
        self.0.get(key).copied()
    }

    /// ensure the [`metrics::Key`] is in the set, inserting a clone if not
    /// already present, returning the correct identifier for the provided key
    pub fn ensure_key(&mut self, key: &Key) -> u16 {
        if let Some(v) = self.0.get(key) {
            return *v;
        }
        let v = u16::try_from(self.0.len()).unwrap_or_else(|_| {
            eprintln!("too many labels!!!");
            u16::MAX
        });
        self.0.insert(key.clone(), v);
        v
    }
}

/// Helper struct for serializing the [`LabelSet`] set to avoid needing to re-allocate the
/// strings owned by the [`metrics::Key`] type along with its value to make it possible
/// to deserialize a serialized `LabelSet` with the correct key<->id mapping
#[derive(Debug, Serialize)]
struct SerKey<'a> {
    key_name: &'a str,
    labels: Vec<SerLabel<'a>>,
    value: u16,
}

/// Helper struct for serializing just the key-value pair owned by a [`metrics::Label`]
/// this will use a tuple of string references
#[derive(Debug, Serialize, Deserialize)]
struct SerLabel<'a>(&'a str, &'a str);
impl<'a> From<SerLabel<'a>> for metrics::Label {
    fn from(value: SerLabel<'a>) -> Self {
        metrics::Label::new(value.0.to_string(), value.1.to_string())
    }
}

/// Helper struct for serializing just the key-value pair owned by a [`metrics::Label`]
/// this will use a tuple of string references
struct SerLabels<'a>(Vec<SerLabel<'a>>);
impl metrics::IntoLabels for SerLabels<'_> {
    fn into_labels(self) -> Vec<metrics::Label> {
        self.0.into_iter().map(metrics::Label::from).collect()
    }
}

impl Serialize for LabelSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut m = serializer.serialize_seq(Some(self.0.len()))?;
        for (k, v) in self.0.iter() {
            let ser_key = SerKey {
                key_name: k.name(),
                labels: k.labels().map(|l| SerLabel(l.key(), l.value())).collect(),
                value: *v,
            };
            m.serialize_element(&ser_key)?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for LabelSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct LabelSetVisitor;

        impl<'de> Visitor<'de> for LabelSetVisitor {
            type Value = LabelSet;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("sequence of label set entries")
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut ret = BTreeMap::new();
                while let Some(element) = seq.next_element::<SerKey<'de>>()? {
                    let SerKey {
                        key_name,
                        labels,
                        value,
                    } = element;
                    let key = Key::from_parts(key_name.to_string(), SerLabels(labels));
                    ret.insert(key, value);
                }
                Ok(LabelSet(ret))
            }
        }

        deserializer.deserialize_seq(LabelSetVisitor)
    }
}

impl<'de> Deserialize<'de> for SerKey<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EntryVisitor;
        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = SerKey<'de>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("map of label data")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut key_name: Option<&str> = None;
                let mut labels: Option<Vec<SerLabel<'_>>> = None;
                let mut value: Option<u16> = None;
                while let Some((k, v)) = map.next_entry()? {
                    match (k, v) {
                        ("key_name", SerKeyValue::KeyName(name)) => key_name = Some(name),
                        ("value", SerKeyValue::Value(i)) => value = Some(i),
                        ("labels", SerKeyValue::Labels(ls)) => labels = Some(ls),
                        _ => {}
                    }
                    if key_name.is_some() && labels.is_some() && value.is_some() {
                        break;
                    }
                }
                Ok(SerKey {
                    key_name: key_name.ok_or_else(|| {
                        serde::de::Error::custom("key_name missing from label set entry")
                    })?,
                    labels: labels.ok_or_else(|| {
                        serde::de::Error::custom("key_name missing from label set entry")
                    })?,
                    value: value.ok_or_else(|| {
                        serde::de::Error::custom("key_name missing from label set entry")
                    })?,
                })
            }
        }

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum SerKeyValue<'a> {
            KeyName(&'a str),
            Labels(Vec<SerLabel<'a>>),
            Value(u16),
        }
        deserializer.deserialize_map(EntryVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::Label;

    #[test]
    fn test_label_set_creation() {
        let label_set = LabelSet::default();
        assert!(label_set.0.is_empty());
    }

    #[test]
    fn test_ensure_key_new() {
        let mut label_set = LabelSet::default();
        let key = Key::from_name("test_metric");

        let id = label_set.ensure_key(&key);
        assert_eq!(id, 0); // First key should get ID 0
        assert_eq!(label_set.0.len(), 1);
        assert_eq!(label_set.get(&key), Some(0));
    }

    #[test]
    fn test_ensure_key_existing() {
        let mut label_set = LabelSet::default();
        let key = Key::from_name("test_metric");

        let id1 = label_set.ensure_key(&key);
        let id2 = label_set.ensure_key(&key); // Same key again

        assert_eq!(id1, id2);
        assert_eq!(label_set.0.len(), 1); // Should still be only 1 key
    }

    #[test]
    fn test_multiple_keys() {
        let mut label_set = LabelSet::default();

        let key1 = Key::from_name("metric1");
        let key2 = Key::from_name("metric2");
        let key3 = Key::from_parts("metric3", vec![Label::new("env", "prod")]);

        let id1 = label_set.ensure_key(&key1);
        let id2 = label_set.ensure_key(&key2);
        let id3 = label_set.ensure_key(&key3);

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
        assert_eq!(label_set.0.len(), 3);
    }

    #[test]
    fn test_keys_with_labels() {
        let mut label_set = LabelSet::default();

        let key1 = Key::from_parts(
            "http_requests",
            vec![Label::new("method", "GET"), Label::new("status", "200")],
        );

        let key2 = Key::from_parts(
            "http_requests",
            vec![Label::new("method", "POST"), Label::new("status", "201")],
        );

        let id1 = label_set.ensure_key(&key1);
        let id2 = label_set.ensure_key(&key2);

        assert_ne!(id1, id2); // Different label sets should get different IDs
        assert_eq!(label_set.0.len(), 2);
    }

    #[test]
    fn test_get_nonexistent_key() {
        let label_set = LabelSet::default();
        let key = Key::from_name("nonexistent");

        assert_eq!(label_set.get(&key), None);
    }

    #[test]
    fn test_label_set_serialization() {
        let mut label_set = LabelSet::default();

        // Add some keys with various label combinations
        label_set.ensure_key(&Key::from_name("simple"));
        label_set.ensure_key(&Key::from_parts(
            "with_labels",
            vec![Label::new("env", "test"), Label::new("service", "api")],
        ));

        // Test JSON serialization
        let json = serde_json::to_string(&label_set).unwrap();
        let deserialized: LabelSet = serde_json::from_str(&json).unwrap();
        assert_eq!(label_set, deserialized);
    }

    #[test]
    fn test_large_number_of_keys() {
        let mut label_set = LabelSet::default();

        // Add many keys to test ID assignment
        for i in 0..1000 {
            let key = Key::from_parts(
                format!("metric_{i}"),
                vec![Label::new("index", i.to_string())],
            );
            let id = label_set.ensure_key(&key);
            assert_eq!(id as usize, i);
        }

        assert_eq!(label_set.0.len(), 1000);
    }

    #[test]
    fn test_key_order_independence() {
        let mut label_set1 = LabelSet::default();
        let mut label_set2 = LabelSet::default();

        // Same labels but different order
        let key1 = Key::from_parts("metric", vec![Label::new("a", "1"), Label::new("b", "2")]);

        let key2 = Key::from_parts("metric", vec![Label::new("b", "2"), Label::new("a", "1")]);

        let id1 = label_set1.ensure_key(&key1);
        let id2 = label_set2.ensure_key(&key2);

        // Keys with same labels in different order should be considered equal
        assert_eq!(key1, key2);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_empty_key_name() {
        let mut label_set = LabelSet::default();
        let key = Key::from_name("");

        let id = label_set.ensure_key(&key);
        assert_eq!(id, 0);
        assert_eq!(label_set.get(&key), Some(0));
    }

    #[test]
    fn test_special_characters_in_labels() {
        let mut label_set = LabelSet::default();
        let key = Key::from_parts(
            "metric",
            vec![
                Label::new("special/chars", "value_with-dashes"),
                Label::new("unicode", "🦀"),
                Label::new("spaces", "value with spaces"),
            ],
        );

        let id = label_set.ensure_key(&key);
        assert_eq!(id, 0);

        // Should serialize/deserialize correctly
        let json = serde_json::to_string(&label_set).unwrap();
        let deserialized: LabelSet = serde_json::from_str(&json).unwrap();
        assert_eq!(label_set, deserialized);
    }

    #[test]
    fn test_max_labels_saturates() {
        let mut label_set = LabelSet::default();

        for i in 0..=u16::MAX {
            let key = Key::from_name(format!("metric_{i}"));
            let id = label_set.ensure_key(&key);
            assert_eq!(id, i);
        }
        let id = label_set.ensure_key(&Key::from_name(""));
        assert_eq!(id, u16::MAX);
    }
}
