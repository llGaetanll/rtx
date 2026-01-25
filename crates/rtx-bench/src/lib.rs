use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;

/// Metadata from the first line of a benchmark JSONL file.
#[derive(Deserialize)]
pub struct BenchmarkMetadata {
    pub version: u32,
    pub timestamp: String,
    pub git_sha: String,
    pub scene: String,
    pub resolution: [u32; 2],
}

/// Per-frame timing data from benchmark JSONL files.
#[derive(Deserialize)]
pub struct FrameRecord {
    pub frame: u32,
    pub t: f32,
    pub time_us: u64,
}

/// A parsed benchmark run with metadata and frame times.
pub struct BenchmarkRun {
    pub metadata: BenchmarkMetadata,
    pub frame_times: Vec<u64>,
}

/// Load a benchmark run from a JSONL file.
pub fn load_benchmark_run(path: &Path) -> Result<BenchmarkRun> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // First line is metadata
    let metadata_line = lines.next().context("Empty benchmark file")??;
    let metadata: BenchmarkMetadata =
        serde_json::from_str(&metadata_line).context("Failed to parse benchmark metadata")?;

    // Remaining lines are frame records
    let mut frame_times = Vec::new();
    for line in lines {
        let line = line?;
        let record: FrameRecord = serde_json::from_str(&line)?;
        frame_times.push(record.time_us);
    }

    Ok(BenchmarkRun {
        metadata,
        frame_times,
    })
}

/// Parsed benchmark filename components.
struct BenchmarkFilename {
    timestamp: String,
    name: String,
}

/// Parse a benchmark filename like "2026-01-15-23-45-09-two_spheres.jsonl"
fn parse_benchmark_filename(filename: &str) -> Option<BenchmarkFilename> {
    let stem = filename.strip_suffix(".jsonl")?;
    // Format: YYYY-MM-DD-HH-MM-SS-name
    // The timestamp is 19 chars (2026-01-15-23-45-09) plus a dash
    if stem.len() < 21 {
        return None;
    }
    let timestamp = &stem[..19];
    let name = &stem[20..];
    Some(BenchmarkFilename {
        timestamp: timestamp.to_string(),
        name: name.to_string(),
    })
}

/// Load all benchmark data from bench-results directory.
///
/// Returns a nested HashMap: benchmark_name -> git_sha -> frame_times
/// Only includes the most recent run per SHA per benchmark.
pub fn load_all_benchmarks(
    bench_results_dir: &Path,
) -> Result<HashMap<String, HashMap<String, Vec<u64>>>> {
    // First pass: collect all runs, grouped by (benchmark_name, sha)
    // Track timestamp to keep only most recent
    let mut all_runs: HashMap<String, HashMap<String, (String, Vec<u64>)>> = HashMap::new();

    // Iterate over SHA directories
    for sha_entry in fs::read_dir(bench_results_dir)? {
        let sha_entry = sha_entry?;
        let sha_path = sha_entry.path();
        if !sha_path.is_dir() {
            continue;
        }
        let sha = sha_entry.file_name().to_string_lossy().to_string();

        // Iterate over JSONL files in this SHA directory
        for file_entry in fs::read_dir(&sha_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            let filename = file_entry.file_name().to_string_lossy().to_string();

            if !filename.ends_with(".jsonl") {
                continue;
            }

            let parsed = match parse_benchmark_filename(&filename) {
                Some(p) => p,
                None => continue,
            };

            let run = match load_benchmark_run(&file_path) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Warning: failed to load {}: {}", file_path.display(), e);
                    continue;
                }
            };

            let by_sha = all_runs.entry(parsed.name.clone()).or_default();
            let entry = by_sha.entry(sha.clone());

            match entry {
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert((parsed.timestamp, run.frame_times));
                }
                std::collections::hash_map::Entry::Occupied(mut o) => {
                    // Keep the more recent one (lexicographic comparison works for our timestamp format)
                    if parsed.timestamp > o.get().0 {
                        o.insert((parsed.timestamp, run.frame_times));
                    }
                }
            }
        }
    }

    // Second pass: strip timestamps, just keep frame_times
    let result = all_runs
        .into_iter()
        .map(|(name, by_sha)| {
            let by_sha_times = by_sha
                .into_iter()
                .map(|(sha, (_ts, times))| (sha, times))
                .collect();
            (name, by_sha_times)
        })
        .collect();

    Ok(result)
}
