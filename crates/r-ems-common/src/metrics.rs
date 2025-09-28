//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;

#[derive(Debug, Default)]
pub struct JitterHistogram {
    samples: Mutex<Vec<f64>>,
}

impl JitterHistogram {
    pub fn record(&self, jitter: Duration) {
        let nanos = jitter.as_secs_f64() * 1_000_000_000.0;
        self.samples.lock().push(nanos);
    }

    pub fn summary(&self) -> Option<JitterSummary> {
        let samples = self.samples.lock();
        let slice = samples.as_slice();
        if slice.is_empty() {
            return None;
        }
        let count = slice.len() as f64;
        let mean = slice.iter().sum::<f64>() / count;
        let variance = if slice.len() > 1 {
            let sum_sq = slice
                .iter()
                .map(|value| {
                    let delta = value - mean;
                    delta * delta
                })
                .sum::<f64>();
            sum_sq / (count - 1.0)
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        let max = slice.iter().copied().fold(f64::MIN, f64::max);
        let min = slice.iter().copied().fold(f64::MAX, f64::min);
        Some(JitterSummary {
            mean_ns: mean,
            std_dev_ns: std_dev,
            max_ns: max,
            min_ns: min,
            samples: slice.len() as u64,
        })
    }

    pub fn write_json<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        if let Some(summary) = self.summary() {
            let mut file = File::create(path)?;
            let json = serde_json::to_vec_pretty(&summary)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
            file.write_all(&json)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct JitterSummary {
    pub mean_ns: f64,
    pub std_dev_ns: f64,
    pub max_ns: f64,
    pub min_ns: f64,
    pub samples: u64,
}

/// Helper for measuring tick intervals against a target period.
#[derive(Debug)]
pub struct LoopTimingReporter {
    target_interval: Duration,
    last_tick: Mutex<Option<Instant>>,
    histogram: JitterHistogram,
}

impl LoopTimingReporter {
    pub fn new(target_interval: Duration) -> Self {
        Self {
            target_interval,
            last_tick: Mutex::new(None),
            histogram: JitterHistogram::default(),
        }
    }

    pub fn record_tick(&self) {
        let mut last_tick = self.last_tick.lock();
        let now = Instant::now();
        if let Some(previous) = *last_tick {
            let actual = now.duration_since(previous);
            let jitter = if actual > self.target_interval {
                actual - self.target_interval
            } else {
                self.target_interval - actual
            };
            self.histogram.record(jitter);
        }
        *last_tick = Some(now);
    }

    pub fn histogram(&self) -> &JitterHistogram {
        &self.histogram
    }
}
