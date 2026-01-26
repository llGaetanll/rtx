# Benchmarking Infrastructure

Track rendering performance across commits to detect regressions.

## Relevant Files

- `crates/rtx-bench/src/spline.rs` - `CatmullRomSpline` implementation
- `crates/rtx-bench/src/camera_path.rs` - `CameraPath` and `CameraFrame` types (frame = position + look-at target)
- `crates/rtx-bench/src/types.rs` - `BenchmarkMetadata`, `FrameRecord`, `GpuInfo` types
- `crates/host/src/bench_app.rs` - Benchmark application with render loop
- `crates/host/src/cli.rs` - CLI definitions including `bench` subcommand
- `bench/configs/*.toml` - Benchmark definition files

## Overview

Run `cargo run --release -- bench` to execute all benchmarks. The camera follows a predefined spline path through a scene, recording frame timing data. Results are saved to `bench/results/<git-sha>/<datetime>-<benchmark-name>.jsonl`.

## Status

### Core Infrastructure

- [x] `CatmullRomSpline` - interpolates through control points
- [x] `CameraPath` - combines position and look-at splines with duration
- [x] `bench` CLI subcommand - runs benchmark with animated camera
- [x] Benchmark exits after camera path completes

### Output & Metadata

- [x] Benchmark output file (JSONL format with metadata + per-frame records)
- [x] Git SHA baked in at build time (7 characters)
- [x] GPU info capture from wgpu adapter
- [x] Frame timing (wall-clock)
- [x] Datetime in output filename

### Benchmark Definitions

- [x] Benchmark definition files in `bench/configs/` directory (TOML format with scene + camera path)
- [x] `--scene` CLI argument for single benchmark
- [x] Output filename uses benchmark name (TOML filename) instead of scene name
- [x] Run all benchmarks when no name specified

### Future Enhancements

- [ ] Automated regression detection: compare results across commits
- [ ] Warmup frames: discard first N frames to avoid startup costs
- [ ] GPU timestamps: use wgpu timestamp queries for more accurate GPU timing

## Benchmark Definition Format

Benchmark definitions live in `bench/configs/<name>.toml`. The `<name>` is used in the output filename (`bench/results/<git-sha>/<datetime>-<name>.jsonl`):

```toml
scene = "two_spheres_fs"
duration = 10.0

position = [
    [5.0, 2.0, 5.0],
    [5.0, 1.5, 0.0],
    [5.0, 2.0, -5.0],
    # ... more control points
]

look_at = [
    [0.0, 0.5, 0.0],
    [0.0, 0.3, 0.0],
    [0.0, 0.5, 0.0],
    # ... more control points
]
```

Both `position` and `look_at` require at least 4 control points for the Catmull-Rom spline.

## Output Format

Single JSONL file per benchmark run: `bench/results/<git-sha>/<datetime>-<benchmark-name>.jsonl`

**Every line is a single JSON object, including the first line (metadata).** This allows streaming writes and easy parsing.

### File Structure

**Line 1: Benchmark metadata**
```json
{"version": 1, "timestamp": "2024-01-15T10:30:00Z", "git_sha": "abc123def456", "scene": "two_spheres", "resolution": [800, 600], "gpu": {"name": "NVIDIA GeForce RTX 3080", "driver": "535.154.05", "backend": "Vulkan"}, "camera_path": {"duration_secs": 10.0, "position_points": [[5.0, 2.0, 5.0], ...], "look_at_points": [[0.0, 0.5, 0.0], ...]}}
```

**Lines 2+: Per-frame data**
```json
{"frame": 0, "t": 0.0, "time_us": 16234, "cam_pos": [5.0, 2.0, 5.0], "cam_dir": [-0.7, -0.1, -0.7], "cam_vup": [0.0, 1.0, 0.0]}
{"frame": 1, "t": 0.003, "time_us": 15891, "cam_pos": [5.1, 2.0, 4.9], "cam_dir": [-0.7, -0.1, -0.7], "cam_vup": [0.0, 1.0, 0.0]}
...
```

### Field Descriptions

**Metadata:**
- `version` - format version for future compatibility
- `timestamp` - ISO 8601 when benchmark started
- `git_sha` - commit hash (from `git rev-parse HEAD`)
- `scene` - shader entry point name
- `resolution` - window dimensions `[width, height]`
- `gpu.name` - adapter name from wgpu
- `gpu.driver` - driver version string
- `gpu.backend` - Vulkan/Metal/DX12/etc
- `camera_path` - full spline data for reproducibility

**Per-frame:**
- `frame` - frame number (0-indexed)
- `t` - spline parameter (0.0 to 1.0)
- `time_us` - frame render time in microseconds
- `cam_pos` - camera position `[x, y, z]`
- `cam_dir` - camera direction `[x, y, z]`
- `cam_vup` - camera up vector `[x, y, z]`
