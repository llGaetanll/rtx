# Benchmark Charts

Generate SVG charts from benchmark results to visualize frame times across commits.

## Relevant Files

- `crates/host/src/cli.rs` - CLI command definitions
- `crates/host/src/main.rs` - Command dispatch
- `bench/results/` - Benchmark output directory (JSONL files organized by git SHA)
- `docs/tasks/benchmarking.md` - Benchmark infrastructure docs (includes JSONL format spec)

## Overview

A new CLI command `cargo run -- chart` that:
1. Scans `bench/results/` for all JSONL files
2. Groups runs by benchmark name (e.g., `two_spheres`, `cornell_box`)
3. Takes the most recent run per SHA for each benchmark
4. Generates a single SVG file with one chart per benchmark

Output: `bench/charts/chart.svg`

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
- [x] Implement directory scanning for `bench/results/`
- [x] Parse JSONL metadata and frame records
- [x] Group by benchmark name, filter to most recent run per SHA
- [x] Export function returning `HashMap<String, HashMap<String, Vec<u64>>>`

### Phase 3: SVG generation

- [x] Add SVG generation module to `rtx-bench`
- [x] Calculate bounds for each benchmark (min/max frame time)
- [x] Scale frame times to pixel coordinates
- [x] Generate `<polyline>` for each SHA's data
- [x] Generate legend with SHA + color
- [x] Stack charts vertically in one SVG
- [x] Return SVG as `String`

### Phase 4: Wire it up

- [x] Call `rtx-bench` from `Commands::Chart` handler
- [x] Create `bench/charts/` directory if needed
- [x] Write `chart.svg` to disk
- [x] Print path to stdout

## Future Work

- [x] Axis labels and tick marks
- [x] **Consolidate benchmark directories**: Reorganized `benchmarks/`, `bench-results/`, and `bench-charts/` into a single `bench/` directory with subdirectories (`bench/configs/`, `bench/results/`, `bench/charts/`).
- [ ] **Unify benchmark types**: `BenchmarkMetadata`, `FrameRecord`, `GpuInfo` are defined in `host/bench_app.rs` (Serialize, with lifetimes) and `rtx-bench` (Deserialize, owned). Consolidate into one set of owned types with `Serialize + Deserialize` in `rtx-bench`, and have `host` import them.
- [x] **Interactive SVG with JavaScript**: Embed JS in the SVG for browser-based interactivity:
  - [x] Toggle commits on/off by clicking legend items (simpler - add IDs to polylines, toggle visibility)
  - [x] Rectangular selection to zoom into a region (mouse tracking, selection box, viewBox transform, reset button)
  - [x] Cursor line snapping to frames with intersection markers and value display
- [x] **Color by recency**: Each benchmark gets a base color, with shades varying by commit age. Darker = newer, lighter = older. Draw older commits first so newer ones render on top. Requires:
  - [x] Preserve timestamps in data loading (currently discarded)
  - [x] Sort SHAs by timestamp before rendering
  - [x] Generate color shades from base hue (HSL with varying lightness)
  - [x] Cap lightness range so light shades are still visible against background
