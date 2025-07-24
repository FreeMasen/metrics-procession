use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, Write, stdout},
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use metrics::Key;
use metrics_procession::{
    event::Op,
    iter::{Metric, MetricRef},
    procession::Procession,
};
use metrics_util::storage::Summary;
use regex::Regex;
use time::{PrimitiveDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Parser)]
pub struct Args {
    /// Where to find the serialized metrics
    source: PathBuf,
    /// A key to filter events for
    #[arg(short, long = "key")]
    keys: Vec<Regex>,
    #[arg(short, long = "label")]
    labels: Vec<KeyValue>,
    #[clap(long, short, value_parser = parse_date_time)]
    start: Option<PrimitiveDateTime>,
    #[clap(long, short, value_parser = parse_date_time)]
    end: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone)]
struct KeyValue {
    key: Regex,
    value: Option<Regex>,
}

impl FromStr for KeyValue {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('=').take(2);
        let Some(key) = parts.next() else {
            return Err(format!(
                "expected 2 values seperated by an equal sign but string was empty: `{s}`"
            ));
        };
        let value = parts.next();
        let key = Regex::new(key).map_err(|e| format!("Error parsing key regex: {e}"))?;
        let value = value
            .map(|value| Regex::new(value).map_err(|e| format!("Error parsing value regex: {e}")))
            .transpose()?;
        Ok(Self { key, value })
    }
}

impl KeyValue {
    fn matches(&self, k: &Key) -> bool {
        for l in k.labels() {
            if self.key.is_match(l.key()) {
                if let Some(v) = self.value.as_ref() {
                    if v.is_match(l.value()) {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }
        false
    }
}

fn main() {
    let Args {
        source,
        keys,
        labels,
        start,
        end,
    } = Args::parse();
    let metrics = deser_metrics(&source);
    let mut collector = QueryCollector::default();
    for metric in metrics.iter() {
        if !keys.iter().all(|re| re.is_match(metric.key.name())) {
            continue;
        }
        if !labels.iter().all(|kv| kv.matches(metric.key)) {
            continue;
        }
        if let Some(start) = start {
            if start.assume_offset(metric.when.offset()) > metric.when {
                continue;
            }
        }
        if let Some(end) = end {
            if end.assume_offset(metric.when.offset()) >= metric.when {
                continue;
            }
        }

        collector.track_metric(metric);
    }
    collector.report_into(&mut stdout().lock()).unwrap();
}

fn parse_date_time(s: &str) -> Result<PrimitiveDateTime, String> {
    let res = PrimitiveDateTime::parse(s, &Rfc3339)
        .map_err(|e| format!("expected RFC3339 formatted date or date-time found `{s}`: {e}"));
    if res.is_err() {
        if let Ok(dt) = time::Date::parse(s, &Rfc3339) {
            return Ok(PrimitiveDateTime::new(dt, time::Time::MIDNIGHT));
        }
    }
    res
}

/// Attempt to deserialize the provided file path as a Procession
fn deser_metrics(path: &Path) -> Procession {
    if path
        .extension()
        .map(|e| e == "postcard")
        .unwrap_or_default()
    {
        let bytes = std::fs::read(path).unwrap();
        let events: Vec<Metric> = postcard::from_bytes(&bytes).unwrap();
        return events.into_iter().collect();
    }
    // If the line was a jsonl file, we can assume each line will be a Metric
    if path.extension().map(|e| e == "jsonl").unwrap_or_default() {
        let buf = BufReader::new(
            File::options()
                .read(true)
                .create(false)
                .write(false)
                .open(path)
                .unwrap(),
        );
        return buf
            .lines()
            .filter_map(|r| {
                let line = r.ok()?;
                if line.trim().is_empty() {
                    return None;
                }
                serde_json::from_str::<Metric>(&line).ok()
            })
            .collect();
    }
    let s = std::fs::read_to_string(path).unwrap();
    // If we have any other file extension, first try and deserialize the full Procession type
    if let Ok(proc) = serde_json::from_str::<Procession>(&s)
        .inspect_err(|e| eprintln!("failed to deser as Procession: {e}"))
    {
        return proc;
    }
    // If the above failed, let finally try and use the array of metrics method for deserializing
    serde_json::from_str::<Vec<Metric>>(&s)
        .unwrap()
        .into_iter()
        .collect()
}

#[derive(Default)]
struct QueryCollector {
    counters: HashMap<Key, usize>,
    gauges: HashMap<Key, GaugeResult>,
    histograms: HashMap<Key, Summary>,
}

impl QueryCollector {
    fn report_into(&self, dest: &mut dyn Write) -> Result<(), io::Error> {
        if !self.counters.is_empty() {
            dest.write_fmt(format_args!("{:->5}COUNTERS{:->5}", "", ""))?;
            for (k, v) in &self.counters {
                dest.write_fmt(format_args!("-\n{} {{", k.name(),))?;
                for label in k.labels() {
                    dest.write_fmt(format_args!("\n  {} => {}", label.key(), label.value()))?;
                }
                dest.write_fmt(format_args!("}}\n{v}\n-"))?;
            }
            dest.write_all(b"\n")?;
        }
        if !self.gauges.is_empty() {
            dest.write_fmt(format_args!("{:->5}GAUGES{:->5}", "", ""))?;
            for (k, v) in &self.gauges {
                dest.write_fmt(format_args!("-\n{} {{", k.name(),))?;
                for label in k.labels() {
                    dest.write_fmt(format_args!("\n  {} => {}", label.key(), label.value()))?;
                }
                dest.write_fmt(format_args!("}}\n"))?;
                dest.write_fmt(format_args!("   min: {:.02},\n", v.min))?;
                dest.write_fmt(format_args!("   max: {:.02},\n", v.max))?;
                dest.write_fmt(format_args!("   avg: {:.02},\n", v.avg))?;
                dest.write_fmt(format_args!("latest: {:.02},\n", v.latest))?;
                dest.write_fmt(format_args!(" count: {:},\n-\n", v.count))?;
            }
        }
        if !self.histograms.is_empty() {
            dest.write_fmt(format_args!("{:->5}HISTOS{:->5}", "", ""))?;
            for (k, v) in &self.histograms {
                dest.write_fmt(format_args!("-\n{} {{", k.name(),))?;
                for label in k.labels() {
                    dest.write_fmt(format_args!("\n  {} => {}", label.key(), label.value()))?;
                }
                dest.write_fmt(format_args!("}}\n"))?;
                for q in [0.5, 0.75, 0.9, 0.99] {
                    let value = v.quantile(q).unwrap();
                    dest.write_fmt(format_args!("p{q:.02}: {value:>.02}\n"))?;
                }
            }
        }
        Ok(())
    }

    fn track_metric(&mut self, metric: MetricRef) {
        let MetricRef { event, key, .. } = metric;
        match event {
            metrics_procession::event::Entry::Gauge { value, op } => {
                self.track_gauge(key.clone(), op, value)
            }
            metrics_procession::event::Entry::Counter { value, op } => {
                self.track_counter(key.clone(), op, value)
            }
            metrics_procession::event::Entry::Histogram { value } => {
                self.track_histo(key.clone(), value)
            }
        }
    }
    fn track_counter(&mut self, key: Key, op: Op, value: u32) {
        if matches!(op, Op::Set) {
            self.counters.insert(key, value as _);
            return;
        }
        let def = self.counters.entry(key).or_default();
        *def += value as usize;
    }

    fn track_gauge(&mut self, key: Key, op: Op, value: f32) {
        let v = self.gauges.entry(key).or_default();
        let value = match op {
            Op::Add => v.latest + value,
            Op::Sub => v.latest - value,
            Op::Set => value,
        };
        v.latest = value;
        v.max = v.max.max(value);
        v.min = v.min.min(value);
        v.avg = v.avg + ((value - v.avg) / (v.count as f32 + 1.0));
        v.count += 1;
    }

    fn track_histo(&mut self, key: Key, value: f32) {
        let v = self
            .histograms
            .entry(key)
            .or_insert_with(|| Summary::new(0.01, 1024, 0.1));
        v.add(value as f64);
    }
}

#[derive(Default)]
struct GaugeResult {
    min: f32,
    max: f32,
    avg: f32,
    latest: f32,
    count: usize,
}
