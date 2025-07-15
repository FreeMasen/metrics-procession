// this module uses a hashmap with the `Key` type which shouldn't really
// change during its lifetime
#![allow(clippy::mutable_key_type)]
use std::collections::HashMap;

use metrics::Key;
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeSeq};

/// A set of labels mapping from the original [`metrics::Key`] to a unique identifier
/// and will be used to lookup what identifier to use when recording metrics events
#[derive(Debug, PartialEq, Clone, Default)]
pub struct LabelSet(pub HashMap<Key, u16>);

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
                let mut ret = HashMap::new();
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
