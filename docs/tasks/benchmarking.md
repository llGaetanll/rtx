# Benchmarking Infrastructure

Track rendering performance across commits to detect regressions.

## Relevant Files

- `crates/host/src/spline.rs` - `CatmullRomSpline` implementation
- `crates/host/src/camera_path.rs` - `CameraPath` and `CameraFrame` types (frame = position + look-at target)
- `crates/host/src/bench_app.rs` - Benchmark application with render loop
- `crates/host/src/cli.rs` - CLI definitions including `bench` subcommand

## Overview

Run `cargo run --release -- bench` to execute a benchmark. The camera follows a predefined spline path through a scene, recording frame timing data. Results are saved to `bench-results/<git-sha>/<datetime>-<scene-name>.jsonl`.

## Status

### Completed
- [x] `CatmullRomSpline` - interpolates through control points
- [x] `CameraPath` - combines position and look-at splines with duration
- [x] `bench` CLI subcommand - runs benchmark with animated camera
- [x] Benchmark exits after camera path completes
- [x] Benchmark output file (JSONL format with metadata + per-frame records)
- [x] `--scene` CLI argument
- [x] Git SHA baked in at build time (7 characters)
- [x] GPU info capture from wgpu adapter
- [x] Frame timing (wall-clock)
- [x] Datetime in output filename

### TODO
- [ ] Benchmark definition files in `benchmarks/` directory (TOML format with scene + camera path)

## Benchmark Definition Format

Benchmark definitions live in `benchmarks/<name>.toml`:

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

Single JSONL file per benchmark run: `bench-results/<git-sha>/<scene-name>.jsonl`

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

## Implementation Plan

### Step 1: Add serde support
- Add `serde` feature to `glam` dependency
- Derive `Serialize` on `CameraPath`, `CatmullRomSpline`
- Create metadata struct with GPU/CPU info

### Step 2: Capture GPU info
- Extract adapter info from wgpu (`adapter.get_info()`)
- Store name, driver, backend

### Step 3: Frame timing
- Measure wall-clock time between frame start and `frame.present()`
- Store in `Vec<FrameRecord>` during benchmark run
- Note: This measures CPU submission + GPU present latency, not pure GPU render time. For more accurate GPU-only timing, wgpu timestamp queries could be added later (see Future Considerations).

### Step 4: Git SHA
- Run `git rev-parse HEAD` at startup
- Create output directory `bench-results/<sha>/`

### Step 5: Write output
- After benchmark completes, write JSON file
- Metadata on line 1, then one frame per line (JSONL)

## Future Considerations

- **Automated regression detection**: Compare results across commits
- **Multiple runs**: Average multiple benchmark runs for stability
- **Warmup frames**: Discard first N frames to avoid startup costs
- **GPU timestamps**: Use wgpu timestamp queries for more accurate GPU timing
