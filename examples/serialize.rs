use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use clap::Parser;
use metrics_procession::{iter::MetricRef, recorder::ProcessionRecorder};
use rand::Rng;

/// This example provides a view of the collected metrics by serializing the TimeStream into multiple formats.
/// It will generate a random set of metrics events and then write the results out to a file or stdout
///
#[derive(Debug, Parser)]
#[command(about)]
struct Args {
    /// How many metrics to emit
    #[clap(default_value_t = 4096)]
    count: u64,
    /// What format to output, the original json blob, an array of metrics
    /// events with timestamps and labels, or a json-lines entry of metrics events
    #[clap(long, short, default_value = "original")]
    format: OutputFormat,
    #[clap(long, short)]
    dest: Option<PathBuf>,
}

fn main() {
    let mut rng = rand::rng();
    let Args {
        count,
        format,
        dest,
    } = Args::parse();
    let recorder = ProcessionRecorder::default();
    metrics::set_global_recorder(recorder.clone()).unwrap();
    let counters = [
        metrics::counter!("counter1"),
        metrics::counter!("counter2", "label1" => "value1"),
        metrics::counter!("counter3", "2label1" => "2value1", "2label2" => "2value2"),
    ];
    let gauges = [
        metrics::gauge!("gauge1"),
        metrics::gauge!("gauge2", "label1" => "value1"),
        metrics::gauge!("gauge3", "2label1" => "2value1", "2label2" => "2value2"),
    ];
    let histos = [
        metrics::histogram!("histo1"),
        metrics::histogram!("histo2", "label1" => "value1"),
        metrics::histogram!("histo3", "2label1" => "2value1", "2label2" => "2value2"),
    ];
    for _i in 0..count {
        match rng.random_range(0..3) {
            0 => {
                let counter_idx = rng.random_range(0..counters.len());
                counters[counter_idx].increment(1);
            }
            1 => {
                let gauge_idx = rng.random_range(0..gauges.len());
                gauges[gauge_idx].set(rng.random::<u8>() as f64);
            }
            2 => {
                let histo_idx = rng.random_range(0..histos.len());
                histos[histo_idx].record(rng.random::<u8>() as f64);
            }
            _ => unreachable!("not in range"),
        }
    }
    let mut out = open_dest(dest);
    let metrics = recorder.lock();
    match format {
        OutputFormat::Original => serde_json::to_writer_pretty(&mut out, &*metrics).unwrap(),
        OutputFormat::Array => {
            let v: Vec<MetricRef> = metrics.iter().collect();
            serde_json::to_writer_pretty(&mut out, &v).unwrap();
        }
        OutputFormat::JsonLines => {
            let mut b = BufWriter::new(out);
            for metric in metrics.iter() {
                let line = serde_json::to_string(&metric).unwrap();
                b.write_all(line.as_bytes()).unwrap();
                b.write_all(b"\n").unwrap();
            }
        }
    }
}

fn open_dest(path: Option<PathBuf>) -> Box<dyn Write> {
    if let Some(p) = &path {
        if let Ok(f) = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(p)
        {
            return write_erase(f);
        }
    }
    write_erase(std::io::stdout().lock())
}

fn write_erase(f: impl Write + 'static) -> Box<dyn Write + 'static> {
    Box::new(f)
}

#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    #[default]
    Original,
    Array,
    JsonLines,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "original" | "o" => Self::Original,
            "array" | "a" => Self::Array,
            "json-lines" | "j" => Self::JsonLines,
            _ => {
                return Err(format!(
                    "expected `original`, `o`, `array`, `a`, `json-lines`, or `j` found `{s}`"
                ));
            }
        })
    }
}

impl ToString for OutputFormat {
    fn to_string(&self) -> String {
        match self {
            OutputFormat::Original => "Original",
            OutputFormat::Array => "Array",
            OutputFormat::JsonLines => "JsonLines",
        }
        .to_string()
    }
}
