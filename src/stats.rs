use std::fmt::{self, Display};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use lazy_static::lazy_static;

use crate::pretty;

const MICROS_IN_SEC: f64 = 1_000_000.0;

lazy_static! {
    static ref STATS: Stats = Stats::default();
}

#[derive(Debug, Default)]
pub struct Counter {
    counter: AtomicU64,
    micros: AtomicU64,
}

impl Counter {
    #[inline]
    pub fn inc(&self, n: usize) {
        self.counter.fetch_add(n as u64, Ordering::SeqCst);
    }

    #[inline]
    pub fn elapsed(&self, elapsed: &Duration) {
        let micros = elapsed.as_micros() as u64;
        self.micros.fetch_add(micros, Ordering::SeqCst);
    }

    #[inline]
    pub fn counter(&self) -> u64 {
        self.counter.load(Ordering::Acquire)
    }

    #[inline]
    pub fn micros(&self) -> u64 {
        self.micros.load(Ordering::Acquire)
    }

    #[inline]
    pub fn timer(&self) -> Timer<'_> {
        Timer(Instant::now(), &self)
    }

    pub fn fit_to_rayon_threads(&self) -> &Self {
        let threads = rayon::current_num_threads();
        let micros = self.micros();
        let micros = micros / threads as u64;
        self.micros.store(micros, Ordering::Release);
        &self
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.counter() == 0
    }

    pub fn to_bytes_string(&self) -> String {
        let num_bytes = self.counter() as f64;
        let micros_n = self.micros();
        let micros = micros_n as f64;
        let secs = micros / MICROS_IN_SEC;
        let num_bytes_per_sec = if micros_n == 0 {
            num_bytes
        } else {
            num_bytes / secs
        };

        format!(
            "{:.2}s - {}/s",
            secs,
            pretty::bytes(num_bytes_per_sec as usize)
        )
    }

    pub fn to_ops_string(&self) -> String {
        let num_ops = self.counter() as f64;
        let micros_n = self.micros();
        let micros = micros_n as f64;
        let secs = micros / MICROS_IN_SEC;
        let num_ops_per_sec = if micros_n == 0 {
            num_ops
        } else {
            num_ops / secs
        };

        format!("{:.2}s - {} ops/s", secs, num_ops_per_sec as usize)
    }
}

#[derive(Debug)]
pub struct Timer<'a>(Instant, &'a Counter);

impl<'a> Timer<'a> {
    #[inline]
    pub fn bytes(&self, n: usize) {
        self.1.inc(n)
    }
}

impl<'a> Drop for Timer<'a> {
    #[inline]
    fn drop(&mut self) {
        let duration = self.0.elapsed();
        self.1.elapsed(&duration);
    }
}

#[derive(Debug, Default)]
pub struct Stats {
    hashing: Counter,
    packing: Counter,
    unpacking: Counter,
    walking: Counter,
    download: Counter,
    upload: Counter,
}

impl Stats {
    #[inline]
    pub fn current() -> &'static Self {
        &STATS
    }

    #[inline]
    pub fn hashing(&self) -> &Counter {
        &self.hashing
    }

    #[inline]
    pub fn packing(&self) -> &Counter {
        &self.packing
    }

    #[inline]
    pub fn unpacking(&self) -> &Counter {
        &self.unpacking
    }

    #[inline]
    pub fn walking(&self) -> &Counter {
        &self.walking
    }

    #[inline]
    pub fn download(&self) -> &Counter {
        &self.download
    }

    #[inline]
    pub fn upload(&self) -> &Counter {
        &self.upload
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if !self.hashing.is_empty() {
            write!(
                f,
                "hashing: {}; ",
                self.hashing.fit_to_rayon_threads().to_bytes_string()
            )?;
        }

        if !self.packing.is_empty() {
            write!(f, "packing: {}; ", self.packing.to_bytes_string())?;
        }

        if !self.unpacking.is_empty() {
            write!(f, "unpacking: {}; ", self.unpacking.to_bytes_string())?;
        }

        if !self.walking.is_empty() {
            write!(f, "walking: {}; ", self.walking.to_ops_string())?;
        }

        if !self.download.is_empty() {
            write!(f, "download: {}; ", self.download.to_bytes_string())?;
        }

        if !self.upload.is_empty() {
            write!(f, "upload: {}; ", self.upload.to_bytes_string())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;

    #[test]
    fn timer() {
        let stats = Stats::default();
        {
            let _timer = stats.hashing().timer();
            thread::sleep(Duration::from_millis(100));
        }

        let micros = stats.hashing().micros();
        assert!(
            micros >= 90_000 && micros <= 110_000,
            "expect 90_000 >= {} <= 110_000",
            micros
        );
    }
}
