# Render Mode

Generate high-quality images (and eventually videos) from scene definitions.

## Relevant Files

- `crates/host/src/cli.rs` - CLI definitions (will add `render` subcommand)
- `crates/host/src/gpu.rs` - `render_to_image()` for offscreen rendering
- `configs/image/*.toml` - Image configs, which is what a render definition is
- `crates/host/src/config.rs` - The config formats
- `renders/` - Output directory for rendered images

## Overview

Run `cargo run --release -- render <name>` to generate an image from `configs/image/<name>.toml`. Output saved to `renders/<name>-<timestamp>.png`.

## Status

- [x] Basic render subcommand
- [x] TOML definition parsing
- [x] Single image output
- [x] Pass quality settings to shader
- [x] Progress indicator for long renders

### Future Enhancements

- [x] Camera animation via spline paths, in a video config
- [ ] Video output: render a video config to frames rather than only timing them
- [ ] Tiled rendering for output larger than the maximum texture dimension

## Implementation Notes

Definitions live in `configs/image/<name>.toml` and images are written to
`renders/<name>-<timestamp>.png`. Definitions are tracked in git; rendered
images are not.

Camera and quality settings reach the shader through `ShaderConstants`, which
grew fields for field of view, defocus angle, focus distance, sample count,
bounce depth and an RNG seed. The shader holds no camera defaults: the host
supplies every setting on every draw, and the scene contributes only its
background color. `live` and `test` take their cameras from the same image
configs `render` uses, and `bench` from a video config; every one of them states
its camera in full.

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

Render definitions are the image configs in `configs/image/<name>.toml`:

```toml
type = "image"
scene = "cornell_box"

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

### Relationship to Videos and Benchmarks

The three commands read one format in two shapes. An image config is a fixed
camera, a video config is the same camera with keyframes in its fields, and a
benchmark is a video whose frames are timed rather than saved. `type` at the top
of the file says which it is. See
[scene-definitions.md](scene-definitions.md) for both formats.
