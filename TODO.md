# TODO

## Features (Ray Tracing in One Weekend series)

1. [x] Lighting (emissive materials, diffuse lights)
   - [x] Add `DiffuseLight` to `MaterialTable` (new variant in `MaterialKind`, new array field)
   - [x] Implement `emitted()` dispatch in `MaterialTable` (currently a `todo!()`)
   - [x] Fix `ray_color` in `cam.rs` to accumulate emission with throughput
   - [x] Create test scene with emissive quad (Cornell box)
2. [x] Quads (axis-aligned and arbitrary)
3. [x] Transforms (translate, rotate, scale)
4. [ ] Finish Cornell box scene - add the two rotated boxes inside
   - Reference: `ray-tracing-in-one-weekend/src/main.rs:cornell_box`
5. [ ] Image textures
6. [ ] Noise textures (Perlin noise)
7. [ ] Bounding Volume Hierarchy (BVH)
   - [ ] Implement AABB (Axis-Aligned Bounding Box) with ray intersection test
   - [ ] Add `bbox()` method to hittable primitives (Sphere, Quad)
   - [ ] Implement linearized BVH structure (flat array, no recursion/pointers for GPU)
   - [ ] Build BVH on CPU, upload as buffer to GPU
   - [ ] Iterative BVH traversal in shader (stack-based or stackless)

## Testing Infrastructure

1. [x] CLI with `live` and `test` subcommands
   - [x] Add clap dependency, `live` subcommand (current behavior), `test` placeholder
   - [x] `live --scene <name>` to select which scene to display
2. [x] Refactor host rendering into reusable pieces
   - [x] Extract wgpu setup (device, queue, pipeline) into reusable module (`gpu.rs`)
   - [x] Create function to render to offscreen texture (`render_to_image`)
3. [x] Offscreen rendering and image output
   - [x] Render to texture, read pixels back to CPU
   - [x] Save as PNG using `image` crate
4. [x] Multiple scenes via shader entry points
   - [x] Multiple fragment entry points in shader (one per scene)
   - [x] Host selects entry point at runtime
5. [x] Grid composition for `test` command
   - [x] Render each scene at 720p
   - [x] Stitch into 4x4 grid image
6. [x] Unit tests for ray-material interactions
   - [x] Shoot specific rays at specific materials, verify resulting scattered ray
   - [x] Test cases: Lambertian diffuse, Metal reflection, Dielectric refraction (entering/exiting)
   - [x] Helps catch regressions like the normal-flip bug from instance transforms

## Technical Debt

1. [ ] **Share types from `rtx-prim` with host**: The `rtx-prim` crate contains elementary types (`Vec3`, `Point3`, `Color`, etc.) that should be usable on both GPU and CPU. Currently `host/src/spline.rs` imports `Vec3` directly from `glam`, but ideally it would use the re-exports from `rtx-prim` to keep types consistent across the codebase. This requires making `rtx-prim` compilable for non-SPIR-V targets.

2. [x] **Remove rejection sampling loops**: The `rand_unit()` function in `rtx-prim/src/traits.rs` uses rejection sampling with an unbounded loop. If the xorshift RNG state ever becomes 0 (which can happen for certain pixel coordinates), it stays 0 forever, causing an infinite loop. Replace with direct sampling methods (e.g., spherical coordinates) that don't require rejection. This is a potential cause of GPU hangs.

3. [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
