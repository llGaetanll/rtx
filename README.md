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
- **rtx-tex** - Textures (solid colors, checkerboard) and the texture table.
- **rtx-util** - Utilities including the camera and random number generation.

## Commands

### `live` - Interactive rendering

Open a window and render a scene in real-time with interactive camera controls.

```sh
cargo run --release -- live --scene cornell_box_fs
```

Available scenes: `cornell_box_fs`, `quads_fs`, `metal_test_fs`, `dielectric_test_fs`, `two_spheres_fs`, `three_spheres_fs`, `many_spheres_fs`

Controls:
- **WASD** - Move horizontally (forward/back/strafe)
- **Space/C** - Move up/down
- **Mouse** - Look around
- **Q/Escape** - Quit

### `test` - Render all scenes

Render all scenes to a 4x4 grid image saved to `renders/render.png`.

```sh
cargo run --release -- test
```

### `render` - High-quality still images

Render a single image from a definition file, accumulating samples over many
passes. Output is saved to `renders/<name>-<timestamp>.png`.

```sh
cargo run --release -- render cornell_box
```

Render definitions are TOML files in `renders/configs/`. Every setting is
required - the shader has no defaults of its own:

```toml
scene = "cornell_box_fs"

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
cargo run --release -- bench two_spheres
```

Run all benchmarks in the `bench/configs/` directory:

```sh
cargo run --release -- bench
```

Benchmark definitions are TOML files in `bench/configs/`. See [docs/tasks/benchmarking.md](docs/tasks/benchmarking.md) for details on the format and output.

## Testing

```sh
cargo test
```
