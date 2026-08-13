# TODO

## Features (Ray Tracing in One Weekend series)

1. [x] Lighting (emissive materials, diffuse lights)
   - [x] Add `DiffuseLight` to `MaterialTable` (new variant in `MaterialKind`, new array field)
   - [x] Implement `emitted()` dispatch in `MaterialTable` (currently a `todo!()`)
   - [x] Fix `ray_color` in `cam.rs` to accumulate emission with throughput
   - [x] Create test scene with emissive quad (Cornell box)
2. [x] Quads (axis-aligned and arbitrary)
3. [x] Transforms (translate, rotate, scale)
4. [x] Finish Cornell box scene - add the two rotated boxes inside
   - Reference: `ray-tracing-in-one-weekend/src/main.rs:cornell_box`
5. [ ] Image textures
6. [ ] Noise textures (Perlin noise)
7. [ ] Volumes / participating media (smoke, fog)
8. [ ] Importance sampling / PDF sampling for lights

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
7. [x] `render` command for high-quality image output
   
   A dedicated command for rendering publication-quality images with configurable quality settings. Unlike `live` (interactive, low samples) or `test` (grid comparison), this produces a single high-quality image.
   
   - [x] `render <name>` loading TOML definitions from `configs/image/`
   - [x] High sample counts (hundreds or thousands per pixel), accumulated over multiple passes
   - [x] Configurable max bounce depth
   - [x] Camera overrides (position, look_at, vup, fov, defocus angle, focus distance)
   - [x] Progress reporting (percentage complete, throughput, ETA)
   - [ ] Video output via animated camera paths (the video config format exists; rendering its frames does not)

8. [x] TOML-based scene definitions
   
   Scenes, images and videos are all TOML now. A scene in `scenes/` says what exists; an image or video config in `configs/` says where it is viewed from and what to produce. See [docs/tasks/scene-definitions.md](docs/tasks/scene-definitions.md).
   
   - [x] Define scene file format (materials, objects, background)
   - [x] Parse TOML on host, build scene data structures
   - [x] Upload scene data to GPU as buffers (replacing hardcoded arrays)
   - [x] Single generic shader entry point that reads scene from buffers
   - [x] Image and video configs reference scenes by name

## Optimizations

1. [ ] Bounding Volume Hierarchy (BVH)
   - [ ] Implement AABB (Axis-Aligned Bounding Box) with ray intersection test
   - [ ] Add `bbox()` method to hittable primitives (Sphere, Quad)
   - [ ] Implement linearized BVH structure (flat array, no recursion/pointers for GPU)
   - [ ] Build BVH on CPU, upload as buffer to GPU
   - [ ] Iterative BVH traversal in shader (stack-based or stackless)

2. [ ] Sample accumulation in live mode
   
   When the camera is stationary, accumulate samples over multiple frames to progressively refine the image. This gives high-quality results without requiring many samples per frame. When the camera moves, reset the accumulator and start fresh.
   
   - [ ] Track camera position/direction, detect when it changes
   - [ ] Accumulation buffer (separate from display buffer)
   - [ ] Blend new samples with accumulated samples (running average)
   - [ ] Reset accumulator on camera movement or scene change
   - [ ] Display sample count somewhere (optional, for debugging)

3. [ ] GPU coherence for ray tracing
   
   Currently, each GPU thread traces a single ray through its entire lifecycle: generate ray, intersect scene, scatter, repeat until max bounces. This is the simplest approach but can be inefficient on GPUs because threads in the same warp/wavefront diverge quickly—some rays hit nothing, others hit different materials, some bounce many times while others terminate early. This divergence means threads sit idle waiting for their neighbors.
   
   **Wavefront path tracing** is an alternative architecture that improves GPU utilization:
   - Instead of one thread per ray, organize work into queues/buffers by operation type
   - All rays needing intersection go into one queue, processed together
   - All rays needing material evaluation go into another queue, grouped by material type
   - This keeps threads in a warp doing the same work, reducing divergence
   
   **Trade-offs:**
   - More complex implementation (multiple kernel launches, queue management)
   - Memory overhead for intermediate buffers
   - May not help for simple scenes where divergence is low
   - Biggest wins come with complex scenes, many materials, and variable-depth paths
   
   **References:**
   - "Megakernels Considered Harmful" (Laine et al., 2013) - foundational wavefront paper
   - NVIDIA OptiX uses this approach internally

## Technical Debt

1. [ ] **Share types from `rtx-prim` with host**: The `rtx-prim` crate contains elementary types (`Vec3`, `Point3`, `Color`, etc.) that should be usable on both GPU and CPU. Currently `host/src/spline.rs` imports `Vec3` directly from `glam`, but ideally it would use the re-exports from `rtx-prim` to keep types consistent across the codebase. This requires making `rtx-prim` compilable for non-SPIR-V targets.

2. [x] **Remove rejection sampling loops**: The `rand_unit()` function in `rtx-prim/src/traits.rs` uses rejection sampling with an unbounded loop. If the xorshift RNG state ever becomes 0 (which can happen for certain pixel coordinates), it stays 0 forever, causing an infinite loop. Replace with direct sampling methods (e.g., spherical coordinates) that don't require rejection. This is a potential cause of GPU hangs.

3. [ ] **Detect and warn when SPIR-V passthrough is unavailable**: without a Vulkan driver, wgpu falls back to the GL backend and the shader goes through naga instead of being passed through as SPIR-V. On that path only the first couple of fragment entry points render correctly - the rest silently reuse an earlier program or hang the GPU (`test` produced six byte-identical grid tiles). Installing a Vulkan driver fixes it, but nothing warns the user, so the output just looks wrong. `render` now fails with a clear error because it needs an `Rgba32Float` target the GL backend does not offer; `live`, `test` and `bench` still fail silently.

4. [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
