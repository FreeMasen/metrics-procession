//! This example is an attempt to quantify the memory impact of keeping
//! a time series for the metrics crate in memory. To run this execute the following
//!
//! ```shell
//! $ cargo run --example size_of -- [batch_count]
//! ```
//!
//! where the argument provided will calculate the total number of events to emit while
//! tracking the `TimeStreamRecorder::memory_size` for each batch.
//!
//! A batch is 65,536 events with a default batch count of 4096
//!
//! The metrics generated are incrementing counters with 1 label, the label has 4096 variations
//! on the value
//!

use std::time::Duration;

use clap::Parser;
use indicatif::{
    HumanBytes, HumanCount, MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle,
};
use metrics_procession::recorder::ProcessionRecorder;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

/// Calculate the memory size of a given number of metrics
#[derive(Debug, Parser)]
#[clap(about)]
struct Args {
    #[clap(default_value_t = 4096)]
    count: u64,
}

fn main() {
    let Args { count } = Args::parse();
    let recorder = ProcessionRecorder::default();
    metrics::set_global_recorder(recorder.clone()).unwrap();
    let mpg = MultiProgress::new();
    let pg = mpg.add(ProgressBar::new(count * (u16::MAX as u64)).with_style(
        ProgressStyle::with_template("[{eta}] {bar:40.cyan/blue} {per_sec} {percent}%").unwrap(),
    ));
    let pg2 = mpg.add(
        ProgressBar::new_spinner()
            .with_style(ProgressStyle::with_template("[{elapsed}] {msg}").unwrap())
            .with_message("getting started"),
    );
    pg2.enable_steady_tick(Duration::from_millis(500));
    let mut ttl = 0;
    for i in 0..count {
        (0..=u16::MAX)
            .into_par_iter()
            .progress_with(pg.clone())
            .for_each(|i| {
                metrics::counter!("some-counter", "with-a-label" => format!("{}", i % 4096))
                    .increment(1);
            });
        ttl += u16::MAX as u64;
        pg2.set_message(format!(
            "{} events take up {} space in memory ({}/event)",
            HumanCount((i + 1) * (u16::MAX as u64)),
            HumanBytes(recorder.memory_size() as u64),
            HumanBytes(recorder.memory_size() as u64 / ttl),
        ));
    }
    pg2.abandon();
    std::thread::sleep(Duration::from_secs(15));
}
