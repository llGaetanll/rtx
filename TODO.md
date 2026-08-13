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
5. [ ] Image textures - see [docs/tasks/image-textures.md](docs/tasks/image-textures.md)
6. [ ] Noise textures (Perlin noise) - see [docs/tasks/perlin-noise.md](docs/tasks/perlin-noise.md)
7. [ ] Volumes / participating media (smoke, fog) - see [docs/tasks/volumes.md](docs/tasks/volumes.md)
8. [x] Importance sampling / PDF sampling for lights

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

1. [x] Bounding Volume Hierarchy (BVH)

   A ray used to test every instance, so a frame cost the instance count times the
   ray count. It now walks a tree of boxes built on the host and uploaded as a
   buffer. See `crates/rtx-obj/src/bvh.rs`, which documents the layout and the
   constants the build is tuned to.

   - [x] Per instance world space boxes (`Instance::world_bbox`), from the corners
         of the unit primitive under the instance's transform
   - [x] Linearized structure: a flat array of nodes in depth first order
   - [x] Build on the host, upload as a buffer, reordering the instances so a leaf
         names a contiguous run of them
   - [x] Stackless traversal in the shader: entering a box carries on to the next
         node, missing one skips to the far end of its subtree. No per ray scratch
         space, which matters because the tracer is short of registers
   - [x] Both closest hit and `occluded`, since NEE makes shadow rays the most
         common kind
   - [x] `Hit::bbox` dropped rather than filled in. It was the `todo!()` waiting on
         this work, and nothing came to need it: the build asks an `Instance` for
         its box directly, and the shader only ever reads boxes off nodes
   - [x] Binned SAH partitioning, which also decides when *not* to split: where
         instances all overlap, as six walls of one room do, a leaf beats a tree
   - [x] `vertex_cubes` added as a benchmark - no existing one had the geometry to
         show an acceleration structure at all

   Measured, as the mean frame of each benchmark:

   | benchmark | instances | before | after |
   | --- | --- | --- | --- |
   | vertex_cubes | 499 | 169 ms | 18.5 ms |
   | cornell_box | 18 | 21.0 ms | 23.2 ms |
   | many_spheres | 9 | 5.4 ms | 6.5 ms |
   | two_spheres | 3 | 1.5 ms | 2.0 ms |

   The small scenes pay for a tree they are too small to use. A control that keeps
   the traversal but never splits measures slower than the old scan on every one of
   them, so the cost is having the walk in the kernel at all rather than the tree
   it walks; no build tuning recovers it.

   Left undone, in the order worth trying:

   - [ ] Front to back traversal. The walk visits children in build order, so the
         closest hit so far shrinks slowly and boxes a better order would cull get
         tested anyway. Needs a stack, which costs the registers the stackless walk
         was chosen to save, so it is worth measuring rather than assuming
   - [ ] Spatial splits (SBVH). The one partitioning idea left, and the only one
         that addresses the Cornell box: a huge thin wall's box overlaps everything,
         and no object partition can separate what overlaps. Splits a primitive's
         box across the plane and names it from both sides, which would break the
         invariant that leaves tile the instance buffer

2. [ ] Sample accumulation in live mode
   
   When the camera is stationary, accumulate samples over multiple frames to progressively refine the image. This gives high-quality results without requiring many samples per frame. When the camera moves, reset the accumulator and start fresh.
   
   - [ ] Track camera position/direction, detect when it changes
   - [ ] Accumulation buffer (separate from display buffer)
   - [ ] Blend new samples with accumulated samples (running average)
   - [ ] Reset accumulator on camera movement or scene change
   - [ ] Display sample count somewhere (optional, for debugging)

3. [ ] GPU coherence for ray tracing

   Every thread still traces one whole path: generate a ray, intersect, scatter,
   again until the bounce cap. Splitting that into stages that pass their state
   through buffers is planned but not built. The plan, the stages, what it would
   cost and how to tell early whether it is working are in
   [docs/tasks/wavefront.md](docs/tasks/wavefront.md).

   Two findings from the groundwork, which the doc explains in full:

   - A compute entry point was built and taken back out again. Dispatched over
     8x8 tiles it measured about 20% slower than the fragment one on this machine,
     which is the deficit any later stage splitting has to win back first. Mapping
     lanes so a subgroup covers a 4x4 block rather than an 8x2 strip was worth 10%
     of that. The doc says what to build again, and what to watch out for.
   - The reason to want this is register pressure rather than the divergence the
     literature leads with. Both entry points compile to 159 and 169 registers
     against the 128 an Intel thread gets before occupancy halves, and half
     occupancy fits every measurement in this project. Stages should each fall
     under that line, and Mesa will say whether they do before anything is timed.

   **References:**
   - "Megakernels Considered Harmful" (Laine et al., 2013) - foundational wavefront paper
   - NVIDIA OptiX uses this approach internally

## Technical Debt

1. [ ] **Share types from `rtx-prim` with host**: The `rtx-prim` crate contains elementary types (`Vec3`, `Point3`, `Color`, etc.) that should be usable on both GPU and CPU. Currently `host/src/spline.rs` imports `Vec3` directly from `glam`, but ideally it would use the re-exports from `rtx-prim` to keep types consistent across the codebase. This requires making `rtx-prim` compilable for non-SPIR-V targets.

2. [x] **Remove rejection sampling loops**: The `rand_unit()` function in `rtx-prim/src/traits.rs` uses rejection sampling with an unbounded loop. If the xorshift RNG state ever becomes 0 (which can happen for certain pixel coordinates), it stays 0 forever, causing an infinite loop. Replace with direct sampling methods (e.g., spherical coordinates) that don't require rejection. This is a potential cause of GPU hangs.

3. [ ] **Detect and warn when SPIR-V passthrough is unavailable**: without a Vulkan driver, wgpu falls back to the GL backend and the shader goes through naga instead of being passed through as SPIR-V. On that path only the first couple of fragment entry points render correctly - the rest silently reuse an earlier program or hang the GPU (`test` produced six byte-identical grid tiles). Installing a Vulkan driver fixes it, but nothing warns the user, so the output just looks wrong. `render` now fails with a clear error because it needs an `Rgba32Float` target the GL backend does not offer; `live`, `test` and `bench` still fail silently.

4. [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
