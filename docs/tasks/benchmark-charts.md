# Benchmark Charts

Generate SVG charts from benchmark results to visualize frame times across commits.

## Relevant Files

- `crates/host/src/cli.rs` - CLI command definitions
- `crates/host/src/main.rs` - Command dispatch
- `bench-results/` - Benchmark output directory (JSONL files organized by git SHA)
- `docs/tasks/benchmarking.md` - Benchmark infrastructure docs (includes JSONL format spec)

## Overview

A new CLI command `cargo run -- chart` that:
1. Scans `bench-results/` for all JSONL files
2. Groups runs by benchmark name (e.g., `two_spheres`, `cornell_box`)
3. Takes the most recent run per SHA for each benchmark
4. Generates a single SVG file with one chart per benchmark

Output: `bench-charts/chart.svg`

## Crate Structure

Create a new crate `rtx-bench` (`crates/rtx-bench/`) for all charting logic. The host crate will depend on it and call into it from the CLI. Later we can move more benchmarking infrastructure into this crate.

## Chart Design

- **X-axis**: Frame number (0 to N)
- **Y-axis**: Frame time in microseconds
- **Lines**: One line per SHA, colored by cycling through a palette
- **Scale**: Each chart has its own y-axis scale (benchmarks vary wildly in frame times)
- **Legend**: Shows 7-char SHA for each line color
- **Labels**: None for now (no axis tick labels)

## Phases

### Phase 1: CLI scaffolding

- [x] Add `Chart` variant to `Commands` enum in `crates/host/src/cli.rs`
- [x] Handle `Commands::Chart` in `main.rs` (placeholder that logs a message)
- [x] Verify it compiles

### Phase 2: Create rtx-bench crate and data loading

- [x] Create `crates/rtx-bench/` crate with `Cargo.toml`
- [x] Add `rtx-bench` to workspace and as dependency of `host`
- [x] Implement directory scanning for `bench-results/`
- [x] Parse JSONL metadata and frame records
- [x] Group by benchmark name, filter to most recent run per SHA
- [x] Export function returning `HashMap<String, HashMap<String, Vec<u64>>>`

### Phase 3: SVG generation

- [ ] Add SVG generation module to `rtx-bench`
- [ ] Calculate bounds for each benchmark (min/max frame time)
- [ ] Scale frame times to pixel coordinates
- [ ] Generate `<polyline>` for each SHA's data
- [ ] Generate legend with SHA + color
- [ ] Stack charts vertically in one SVG
- [ ] Return SVG as `String`

### Phase 4: Wire it up

- [ ] Call `rtx-bench` from `Commands::Chart` handler
- [ ] Create `bench-charts/` directory if needed
- [ ] Write `chart.svg` to disk
- [ ] Print path to stdout

## Future Work

- Axis labels and tick marks
- Interactive HTML version with tooltips
- Comparison mode (pick specific SHAs to compare)
- Statistical summary (avg/p95/p99 per SHA)
- **Consolidate benchmark directories**: Currently we have `benchmarks/`, `bench-results/`, and `bench-charts/` at the repo root. Consider reorganizing into a single `bench/` directory with subdirectories (`bench/definitions/`, `bench/results/`, `bench/charts/`).
- **Unify benchmark types**: `BenchmarkMetadata`, `FrameRecord`, `GpuInfo` are defined in `host/bench_app.rs` (Serialize, with lifetimes) and `rtx-bench` (Deserialize, owned). Consolidate into one set of owned types with `Serialize + Deserialize` in `rtx-bench`, and have `host` import them.
