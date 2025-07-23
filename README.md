# Metrics Procession

This project is an in-memory [metrics](https://crates.io/crates/metrics) recorder aimed at reducing
total size of metrics collected while maintaining a millisecond precision time-series.

## How it works

A time series of data in the raw-est sense is an array of triples, the first element is a
representation of the time this event was emitted, the second element is the event's type and
payload as a pair, finally the last element is a representation of the unique name and label set. If
we were to construct that value, we would en up with an object much larger than is ideal since the
`Instant` or `time::OffsetDateTime` types will en up taking at least 16 bytes of memory and metrics
deals with 64 bit types by default, with 2 tags on the largest size meaning we end up with an `enum`
of 10(8+2) bytes in size and then another 64 bytes for the `metrics::Key` value that is a total of
90 bytes per entry, which adds up quickly.

So, how small could we make this reasonably? Fist of all, we probably don't need to keep a full time
representation on each metric, if a chunk of events was associated with a reference time, then a 16
bit integer could be used to track the milliseconds since the reference time. That means we can
represent a `Chunk` as a pair of a OffsetDateTime and a `Vec` of the triple that contains the metric
type and value, the milliseconds and the unique key+label set. That reducing 16 bytes down to 2
bytes, the remaining bytes being amortized across the number of events in a give chunk.

Next we will want to try and reduce the size of the `Key` value, for that we can again use a `u16`
and then amortized the cost of each `Key` across all chunks currently in the series. For that we use
a `BTreeMap<Key, u16>` to allow looking up the id value for any give key while recording, this
mapping is owned by the series itself and not any given chunk which means we have a maximum unique
set of labels at `65535` which is reasonable for most systems but may not be suitable for all
systems. It may be valuable to add a filtering metrics layer above the `Recorder` provided by this
crate to avoid loss of data.

So, we've now knocked another 10 bytes off the storage size of each raw event, meaning we have a
total size of 2 + 10 + 2 = 14 bytes, which is a very large amount smaller than where we started.

## Additional APIs

The `Procession` type provides multiple representations that can be used to capture the current
state of the recorder that can be deserialized later. The type itself implements `Serialize` and
`Deserialize` which will include the map of `key`s to their ids and a `Vec<Chunk>` which includes
the reference time along with a `Vec<Event>` in that section of the series.

There are also 2 ways to iterate through the series, either by cloning the `Key`'s contents or by
borrowing them from the `Procession`, both representations also implement `Serialize` but only the
cloned version implements `Deserialize` since the semantics of the `Key` storage is a bit more
complicated.

> As a warning the `Procession`'s implementation of `Deseriaize` requires a borrowed string meaning
> it cannot be used with an `impl Read` type (used by `serde_json::from_reader`)
