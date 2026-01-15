# TODO

## Benchmarking Infrastructure (Priority)

**No optimization code changes until this is complete.**

- [ ] Implement benchmark mode CLI subcommand (`cargo run --release -- bench`)
- [ ] Camera spline system - move camera along a path over time
- [ ] Look-at spline - animate what the camera looks at for realistic movement
- [ ] Frame time recording - capture timing for each frame
- [ ] Output timing data to file for analysis
- [ ] Consistent benchmark runs (fixed frame count, deterministic camera path)

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

- [x] Add camera fields to `ShaderConstants` (cam_pos, cam_dir, cam_fov, etc.)
- [x] Modify shader to construct `Camera` from `ShaderConstants` instead of hardcoded values
- [ ] Remove or bypass per-scene camera construction

### Phase 3: Fix gimbal lock with quaternions

The camera crashes when looking straight up or down because `vup` becomes parallel to the view direction, causing a zero-length cross product that produces NaN values. Tests in `crates/rtx-util/tests/camera_tests.rs` demonstrate this issue.

**Stage 1: Add `vup` to data path**
- [x] Add `vup: [f32; 3]` to `ShaderConstants`
- [x] Thread `vup` through `two_spheres()` and `CameraParams`
- [x] Host computes `vup` dynamically (switch reference vector when pitch exceeds ±45°)
- [x] Update tests to pass with dynamic `vup`

**Stage 2: Switch host to quaternions**
- [x] Add `glam` crate dependency to host (for `Quat`)
- [x] Replace `cam_yaw: f32` / `cam_pitch: f32` with `cam_orientation: Quat`
- [x] Rewrite `update_camera()`:
  - Apply yaw rotation in world space (around world Y axis)
  - Apply pitch rotation in local space (around camera's right axis)
  - Multiply rotations into orientation quaternion
- [x] Extract `cam_dir` and `vup` from quaternion for shader:
  - `cam_dir` = quaternion rotated `-Z` (forward)
  - `vup` = quaternion rotated `+Y` (up)
- [x] Remove pitch clamping (quaternions handle full rotation)
- [x] Tune mouse sensitivity for natural feel

### Future enhancements (not for first pass)

- [ ] **Dynamic ray sampling**: Lower samples per pixel when camera/world is moving for faster feedback, then accumulate rays over time when stationary for higher quality. Requires tracking frame-to-frame camera changes and maintaining an accumulation buffer.
- [ ] Screenshot with current camera position

## Technical Debt

1. [ ] **Crash when camera enters an object**: The program sometimes crashes, likely when the camera position moves inside geometry. This may cause issues with ray-object intersection tests (e.g., negative t values, NaN from inside-surface calculations). Needs investigation.

2. [x] **Remove rejection sampling loops**: The `rand_unit()` function in `rtx-prim/src/traits.rs` uses rejection sampling with an unbounded loop. If the xorshift RNG state ever becomes 0 (which can happen for certain pixel coordinates), it stays 0 forever, causing an infinite loop. Replace with direct sampling methods (e.g., spherical coordinates) that don't require rejection. This is a potential cause of GPU hangs.

3. [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
