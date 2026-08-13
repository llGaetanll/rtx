# `rtx`

A ray tracing sandbox written in Rust, compiled to SPIR-V shaders using [rust-gpu](https://github.com/rust-gpu/rust-gpu). The ray tracing logic runs entirely on the GPU as a compute shader.

## Requirements

A Vulkan-capable driver (for example `vulkan-intel` or `vulkan-radeon` on Arch
derivatives). Without one, wgpu falls back to its GL backend, where the shader
is translated by naga instead of being passed through as SPIR-V - on that path
most scene entry points silently misrender or hang the GPU.

## Crates

- **host** - The host application that sets up the GPU, manages buffers, and displays the rendered image.
- **shader** - The GPU shader entry point compiled to SPIR-V via rust-gpu.
- **shared** - Types shared between host and shader (e.g., push constants).
- **rtx-prim** - Primitive types: `Array`, `List`, `Ray`, `HitRecord`, `Interval`, etc.
- **rtx-obj** - Scene objects that can be hit by rays (e.g., `Sphere`).
- **rtx-mat** - Materials (Lambertian, Metal, Dielectric) and the material dispatch table.
- **rtx-tex** - Textures (solid colors) and the texture table.
- **rtx-util** - Utilities including the camera and random number generation.

## Scenes and configs

A scene is a TOML file listing materials and objects. It says what exists and
nothing about where it is viewed from, so the same scene can back a still image,
a benchmark fly-through and a video without being copied.

Cameras live in configs, which come in two kinds:

- an image config - a fixed camera and what to produce. Read by `render`, and by
  `live` for its starting view.
- a video config - the same, with a camera that moves. Read by `bench`.

Neither file names the other. Every command takes both as paths:

```sh
cargo run --release -- render --scene scenes/cornell_box.toml --config configs/image/cornell_box.toml
```

The scenes and configs that come with the repository live in `scenes/`,
`configs/image/` and `configs/video/`, but that is a convention: nothing
resolves a name against those directories, so a scene and a config can live
anywhere and any camera can be pointed at any scene.

See [docs/tasks/scene-definitions.md](docs/tasks/scene-definitions.md) for both
formats.

## Commands

### `live` - Interactive rendering

Open a window and fly around a scene, starting from an image config's camera.

```sh
cargo run --release -- live -s scenes/cornell_box.toml -c configs/image/cornell_box.toml
```

Controls:
- **WASD** - Move horizontally (forward/back/strafe)
- **Space/C** - Move up/down
- **Mouse** - Look around
- **Q/Escape** - Quit

### `render` - High-quality still images

Render a single image, accumulating samples over many passes. Output is saved to
`renders/<config>-<timestamp>.png`, named after the config file.

```sh
cargo run --release -- render -s scenes/cornell_box.toml -c configs/image/cornell_box.toml
```

Pass `--preview` to watch the image accumulate in a window; closing it early
saves what has been rendered so far.

A config holds the camera, the quality and the output size. Every setting is
required - the shader has no defaults of its own:

```toml
type = "image"

[camera]
position = [278.0, 278.0, -800.0]
look_at = [278.0, 278.0, 0.0]
vup = [0.0, 1.0, 0.0]
fov = 40.0           # vertical field of view in degrees
defocus_angle = 0.0  # aperture angle in degrees, 0 disables depth of field
focus_dist = 10.0

[quality]
samples = 500     # rays per pixel
bounces = 50      # maximum ray bounce depth

[output]
width = 1920
height = 1080
```

Samples are split into passes of 8 so no single draw call runs long enough to
trip the GPU watchdog, which means the requested sample count rounds up to the
next multiple of the pass size. Progress is logged per pass with throughput and
an ETA.

### `bench` - Performance benchmarking

Run benchmarks with an animated camera path. Results are saved to `bench/results/<git-sha>/<timestamp>-<name>.jsonl`.

Run a specific benchmark:

```sh
cargo run --release -- bench -s scenes/two_spheres.toml -c configs/video/two_spheres.toml
```

Run every benchmark listed in `bench.toml`, which is where a benchmark camera is
paired with the scene it flies through:

```sh
cargo run --release -- bench
```

Benchmarks are video configs: a benchmark is a video whose frames are timed and
thrown away rather than saved. See [docs/tasks/benchmarking.md](docs/tasks/benchmarking.md)
for the output format.

## Testing

```sh
cargo test
```
