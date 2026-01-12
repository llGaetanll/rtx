# TODO

## Features (Ray Tracing in One Weekend series)

1. [x] Lighting (emissive materials, diffuse lights)
   - [x] Add `DiffuseLight` to `MaterialTable` (new variant in `MaterialKind`, new array field)
   - [x] Implement `emitted()` dispatch in `MaterialTable` (currently a `todo!()`)
   - [x] Fix `ray_color` in `cam.rs` to accumulate emission with throughput
   - [x] Create test scene with emissive quad (Cornell box)
2. [x] Quads (axis-aligned and arbitrary)
3. [x] Transforms (translate, rotate, scale)
4. [ ] Image textures
5. [ ] Noise textures (Perlin noise)
6. [ ] Bounding Volume Hierarchy (BVH)
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

## Interactive Camera Controls (Live Mode)

Add keyboard and mouse controls to move the camera in real-time during `live` mode.

### Overview

Currently the camera is constructed entirely on the GPU side from hardcoded parameters in each scene function. The host only passes frame dimensions, elapsed time, and cursor position via `ShaderConstants`. To enable interactive controls, camera state must live on the CPU and be passed to the shader each frame.

### Phase 1: CPU-side input and logging (no shader changes)

Get input handling working and validate the camera math before touching the shader.

**Controls:**
- WASD: horizontal movement (forward/back/strafe)
- Space/C: up/down
- Mouse: look direction (no cursor grab - just track movement while window focused)
- Q: quit (in addition to Escape)

**Implementation:**
- [ ] Add camera state to app struct (position, yaw, pitch)
- [ ] Track held keys (simple bools for W/A/S/D/Space/C)
- [ ] Track mouse delta for look direction
- [ ] Each frame: update position based on held keys and orientation, update orientation from mouse
- [ ] Log computed camera params to stdout
- [ ] Q key exits the loop

### Phase 2: Wire up to shader

Once CPU-side camera is working:

- [ ] Add camera fields to `ShaderConstants` (cam_pos, cam_dir, cam_fov, etc.)
- [ ] Modify shader to construct `Camera` from `ShaderConstants` instead of hardcoded values
- [ ] Remove or bypass per-scene camera construction

### Future enhancements (not for first pass)

- Sample accumulation when stationary
- Screenshot with current camera position

## Technical Debt

1. [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
