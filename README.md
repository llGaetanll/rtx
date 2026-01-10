# `rtx`

A ray tracing sandbox written in Rust, compiled to SPIR-V shaders using [rust-gpu](https://github.com/rust-gpu/rust-gpu). The ray tracing logic runs entirely on the GPU as a compute shader.

## Crates

- **host** - The host application that sets up the GPU, manages buffers, and displays the rendered image.
- **shader** - The GPU shader entry point compiled to SPIR-V via rust-gpu.
- **shared** - Types shared between host and shader (e.g., push constants).
- **rtx-prim** - Primitive types: `Array`, `List`, `Ray`, `HitRecord`, `Interval`, etc.
- **rtx-obj** - Scene objects that can be hit by rays (e.g., `Sphere`).
- **rtx-mat** - Materials (Lambertian, Metal, Dielectric) and the material dispatch table.
- **rtx-tex** - Textures (solid colors, checkerboard) and the texture table.
- **rtx-util** - Utilities including the camera and random number generation.

## Running

Open a live window rendering a scene:

```sh
cargo run --release -- live --scene cornell_box_fs
```

Available scenes: `cornell_box_fs`, `quads_fs`, `metal_test_fs`, `dielectric_test_fs`, `two_spheres_fs`, `three_spheres_fs`, `many_spheres_fs`

Render all scenes to a grid image (`renders/render.png`):

```sh
cargo run --release -- test
```

## Testing

```sh
cargo test
```
