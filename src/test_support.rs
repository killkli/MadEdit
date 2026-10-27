//! Small, dependency-free helpers shared by regression tests and benchmarks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "madedit-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create test directory: {e}"),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Fixed-seed operation generation keeps model failures reproducible.
pub struct Rng(pub u64);

impl Rng {
    pub fn below(&mut self, limit: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % limit as u64) as usize
    }
}

/// Preserve individual samples as well as summaries for offline comparisons.
pub fn report_benchmark(case: &str, samples: Vec<Duration>) {
    use std::io::Write;
    assert!(!samples.is_empty());
    let samples_ms: Vec<_> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    let mut ordered = samples_ms.clone();
    ordered.sort_by(f64::total_cmp);
    let median_ms = ordered[samples.len() / 2];
    let p90_ms = ordered[samples.len() * 9 / 10];
    println!("{case}: median={median_ms:.3} ms p90={p90_ms:.3} ms");
    if let Some(path) = std::env::var_os("MADEDIT_BENCH_JSONL") {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        let record = serde_json::json!({
            "case": case, "median_ms": median_ms, "p90_ms": p90_ms, "samples_ms": samples_ms,
        });
        writeln!(file, "{record}").unwrap();
    }
}
