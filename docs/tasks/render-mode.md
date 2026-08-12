# Render Mode

Generate high-quality images (and eventually videos) from scene definitions.

## Relevant Files

- `crates/host/src/cli.rs` - CLI definitions (will add `render` subcommand)
- `crates/host/src/gpu.rs` - `render_to_image()` for offscreen rendering
- `renders/*.toml` - Render definition files (proposed)
- `renders/` - Output directory for rendered images

## Overview

Run `cargo run --release -- render <name>` to generate an image from `renders/configs/<name>.toml`. Output saved to `renders/<name>-<timestamp>.png`.

## Status

- [x] Basic render subcommand
- [x] TOML definition parsing
- [x] Single image output
- [x] Pass quality settings to shader
- [x] Progress indicator for long renders

### Future Enhancements

- [ ] Video output (frame sequence or encoded video)
- [ ] Camera animation via spline paths (reuse `CameraPath` from benchmarking)
- [ ] Tiled rendering for output larger than the maximum texture dimension

## Implementation Notes

Definitions live in `renders/configs/<name>.toml` and images are written to
`renders/<name>-<timestamp>.png`. Definitions are tracked in git; rendered
images are not.

Camera and quality settings reach the shader through `ShaderConstants`, which
grew fields for field of view, defocus angle, focus distance, sample count,
bounce depth and an RNG seed. The shader holds no camera defaults: the host
supplies every setting on every draw, and the scene contributes only its
background color. The cameras `live`, `test` and `bench` view each scene from
live in `crates/host/src/scenes.rs`, and render definitions must state theirs in
full.

High sample counts cannot run in a single draw call without tripping the GPU
watchdog, so a render is split into passes of `SAMPLES_PER_PASS` rays per pixel.
Each pass uses a different seed and adds its samples into an `Rgba32Float`
target with additive blending, so the image stays on the GPU until every pass is
done and is read back exactly once. The WebGPU spec forbids blending 32 bit
float targets, so this needs the `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`
feature, which tells wgpu validation to consult the real adapter capabilities;
`render` fails with a clear error where that is unavailable. Summing each pass on
the CPU instead cost about 10% of total render time.

The float target is also why the sRGB transfer is applied by hand when the PNG is
written, while `test` gets it free from its `Rgba8UnormSrgb` target.

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
