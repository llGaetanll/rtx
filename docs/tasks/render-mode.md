# Render Mode

Generate high-quality images (and eventually videos) from scene definitions.

## Relevant Files

- `crates/host/src/cli.rs` - CLI definitions (will add `render` subcommand)
- `crates/host/src/gpu.rs` - `render_to_image()` for offscreen rendering
- `renders/*.toml` - Render definition files (proposed)
- `renders/` - Output directory for rendered images

## Overview

Run `cargo run --release -- render [name]` to generate an image. Without a name, renders all definitions in `renders/`. Output saved to `renders/<name>.png`.

## Status

- [ ] Basic render subcommand
- [ ] TOML definition parsing
- [ ] Single image output
- [ ] Pass quality settings to shader

### Future Enhancements

- [ ] Video output (frame sequence or encoded video)
- [ ] Camera animation via spline paths (reuse `CameraPath` from benchmarking)
- [ ] Progress indicator for long renders

## Render Definition Format

Render definitions live in `renders/<name>.toml`:

```toml
scene = "cornell_box_fs"

[camera]
position = [278.0, 278.0, -800.0]
look_at = [278.0, 278.0, 0.0]

[quality]
samples = 100        # rays per pixel (px_samples)
bounces = 50         # max ray bounces (max_ray_bounce)

[output]
width = 1920
height = 1080
```

### Comparison with Benchmark Format

Benchmarks and renders share similar structure. A unified format could support both use cases:

```toml
scene = "cornell_box_fs"

[camera]
# Static camera (render) - just position + look_at
position = [278.0, 278.0, -800.0]
look_at = [278.0, 278.0, 0.0]

# OR animated camera (benchmark/video) - spline paths
# position = [[...], [...], ...]
# look_at = [[...], [...], ...]
# duration = 10.0

[quality]
samples = 100
bounces = 50

[output]
width = 1920
height = 1080

# Optional: what to produce
# mode = "image"      # single frame render
# mode = "benchmark"  # timing data, no image saved
# mode = "video"      # frame sequence (future)
```

The key difference: benchmarks use animated camera paths and record timing; renders use static cameras (or animated for video) and save images. Could potentially merge into one format where:
- Static camera + image mode = render
- Animated camera + benchmark mode = benchmark
- Animated camera + video mode = video render

## Open Questions

- Should render definitions live in `renders/` or a separate `scenes/` directory?
- Should output go to `renders/` or `output/` or `renders/output/`?
- Unified format vs separate formats for benchmark/render?
