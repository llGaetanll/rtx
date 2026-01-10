# RTX Architecture Overview

A GPU-accelerated ray tracing sandbox written in Rust, compiled to SPIR-V via rust-gpu and rendered with wgpu.

## High-Level Architecture

The project splits into two layers:

**Host (CPU)**: The `host` crate manages window creation, wgpu initialization, and the render loop. It compiles the shader to SPIR-V at build time and passes frame data via push constants through the `shared` crate.

**Shader (GPU)**: The `shader` crate is the SPIR-V entry point. It builds on the ray tracing library crates (`rtx-prim`, `rtx-obj`, `rtx-mat`, `rtx-tex`, `rtx-util`) to trace rays per-pixel.

## Crate Purposes

### `host`
CPU-side application with CLI interface:
- `live --scene <name>` - Opens window and renders scene live (default: `cornell_box_fs`)
- `test` - Renders all scenes to a 4x4 grid image at 720p each, saved to `renders/render.png`

Manages window creation, wgpu device/queue/surface initialization, and the render loop. GPU setup is factored into `gpu.rs` with `GpuContext` struct providing:
- `create_instance()` / `new()` - wgpu initialization
- `create_pipeline(format, entry_point)` - render pipeline creation
- `render_to_image(width, height, entry_point)` - offscreen rendering

Compiles the shader crate to SPIR-V at build time via `spirv-builder`. Uses `ouroboros` for self-referential window/surface lifetime management.

### `shared`
Types shared between host and shader. Currently just `ShaderConstants` (frame dimensions, time, cursor position). Uses bytemuck for safe GPU memory mapping.

### `shader`
GPU entry points compiled to SPIR-V. Contains multiple fragment shader entry points for different scenes:
- `cornell_box_fs` - Cornell box with colored walls and ceiling light
- `quads_fs` - Five colored quads in a room-like arrangement
- `metal_test_fs` - Metal spheres with varying fuzz
- `dielectric_test_fs` - Glass spheres demonstrating refraction
- `two_spheres_fs` - Two spheres on checkered ground
- `three_spheres_fs` - Glass, lambertian, and metal spheres on ground
- `many_spheres_fs` - Classic final scene with many small spheres

Scene setup is factored into `scene.rs` with functions like `cornell_box()`, `quads()`, etc. that return `(Camera, MaterialTable, TextureTable, List<NS, NQ>)`. Each entry point calls its scene function, then traces rays per-pixel.

### `rtx-prim`
Core primitives and utilities:
- `Ray` - origin, direction, time (for motion blur)
- `Aabb` - axis-aligned bounding box with slab intersection
- `Array<T, N>` - fixed-size dynamic array for GPU
- `Range<T>` - interval type for t-parameter bounds
- `RandState` - PCG-based xorshift PRNG
- `Vec3Ext` - extension trait for random vector generation
- Type aliases: `F` (f32), `Vec3`, `Point3`, `Color`

### `rtx-obj`
Scene objects implementing the `Hit` trait:
- `Sphere` - with UV mapping, motion blur support
- `Quad` - arbitrary parallelogram defined by corner point and two edge vectors
- `List<NS, NQ>` - fixed-size collection of spheres and quads with bounding box
- `HitRecord` - intersection data (point, normal, material, UV, t)

### `rtx-mat`
Materials implementing the `Material` trait:
- `Lambertian` - diffuse scattering
- `Metal` - reflection with optional fuzz
- `Dielectric` - refraction with Schlick approximation
- `MaterialTable` - dispatch table routing by material kind

### `rtx-tex`
Textures implementing the `Texture` trait:
- `SolidTexture` - constant color
- `TextureTable` - dispatch table for texture sampling

### `rtx-util`
High-level utilities:
- `Camera` - ray generation, depth of field, anti-aliasing
- `CameraParams` - configuration (FOV, focus, samples, bounces)
- `render()` - per-pixel multi-sample rendering
- `ray_color()` - recursive ray tracing with material scattering

## Crate Dependencies

- `host` depends on `shared` and `spirv-builder` (build-time)
- `shader` depends on `shared`, `rtx-prim`, `rtx-obj`, `rtx-mat`, `rtx-tex`, `rtx-util`
- `rtx-util` depends on `rtx-prim`, `rtx-obj`, `rtx-mat`, `rtx-tex`
- `rtx-obj` depends on `rtx-prim`, `rtx-mat`
- `rtx-mat` depends on `rtx-prim`, `rtx-tex`
- `rtx-tex` depends on `rtx-prim`
- `rtx-prim` depends on `spirv-std`, `bytemuck`

## Key Traits

### `Hit` (rtx-obj)
```rust
fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool;
fn bbox(&self) -> &Aabb;
```

### `Material` (rtx-mat)
```rust
fn scatter(&self, state: &mut RandState, tex_table: &TextureTable,
           incoming: &Ray, rec: &HitRecord, 
           scattered: &mut Ray, attenuation: &mut Color) -> bool;
fn emitted(&self, ...) -> Color;
```

### `Texture` (rtx-tex)
```rust
fn value(&self, info: TextureInfo, u: F, v: F, point: Point3) -> Color;
```

## Design Patterns

**Dispatch Tables**: `MaterialTable` and `TextureTable` use arrays of concrete types instead of trait objects. GPU-friendly static dispatch via kind discriminant + index.

**Const Generics**: `Array<T, N>` and `List<N>` use const generics for compile-time known sizes without heap allocation.

**no_std Compatibility**: All GPU crates use `#![cfg_attr(target_arch = "spirv", no_std)]` with `spirv-std` replacing std.

**Stateless Abstractions**: Camera and ray tracing logic thread state through parameters. No mutable global state.

## Build Process

1. `host/build.rs` invokes spirv-builder
2. Compiles `shader` crate to `spirv-unknown-vulkan1.2`
3. Outputs SPIR-V binary
4. Host includes via `include_spirv!()` macro

## Execution Flow

1. Host creates window and wgpu pipeline with compiled SPIR-V
2. Per frame: update ShaderConstants, issue render command
3. Fragment shader runs per-pixel:
   - Initialize tables, camera, scene
   - Generate deterministic RNG seed from pixel coords
   - Trace N samples per pixel
   - Accumulate, gamma-correct, return color
4. Ray tracing loop: intersect scene, scatter via material, attenuate, repeat until max bounces or miss
