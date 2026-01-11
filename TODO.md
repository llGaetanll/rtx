# TODO

## Features (Ray Tracing in One Weekend series)

1. [x] Lighting (emissive materials, diffuse lights)
   - [x] Add `DiffuseLight` to `MaterialTable` (new variant in `MaterialKind`, new array field)
   - [x] Implement `emitted()` dispatch in `MaterialTable` (currently a `todo!()`)
   - [x] Fix `ray_color` in `cam.rs` to accumulate emission with throughput
   - [x] Create test scene with emissive quad (Cornell box)
2. [x] Quads (axis-aligned and arbitrary)
3. [ ] Transforms (translate, rotate, scale) - see design notes at bottom of file
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

## Technical Debt

1. [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.

---

## Design: Instanced Transforms

### Current Architecture

The ray tracer currently uses `List` which contains arrays of world-space primitives:

```rust
// rtx-obj/src/list.rs (current)
struct List {
    spheres: Array<Sphere, N>,
    quads: Array<Quad, M>,
}
```

Each `Sphere` stores its center, radius, material, and precomputed bounding box.
Each `Quad` stores its corner, edge vectors, normal, material, and bounding box.
The `hit()` method iterates both arrays, testing each primitive in world space.

### New Architecture

Instead of storing concrete primitives with world-space coordinates, we use
canonical unit primitives (implicit/hardcoded) and transform them via instances.

**Canonical primitives (not stored, just hardcoded in hit functions):**
- Unit sphere: radius 1, centered at origin
- Unit quad: 1x1, at origin (corners at (0,0,0) and (1,1,0), normal +Z)

**New types:**

```rust
enum PrimitiveKind { Sphere, Quad }

struct Instance {
    kind: PrimitiveKind,
    transform: Mat4,      // object space -> world space
    inv_transform: Mat4,  // world space -> object space
    material: MaterialInfo,
}

struct Scene {
    instances: Array<Instance, N>,
}
```

**Hit logic:**
1. For each instance, transform ray to object space via `inv_transform`
2. Test against hardcoded unit sphere or unit quad
3. Transform hit point/normal back to world space via `transform`

**Mat4 capabilities:**
- Translation: move objects in space
- Rotation: rotate around any axis (start with rotate_y)
- Non-uniform scale: `scale(4, 1, 1)` stretches only on X axis
- Compose as: `translate * rotate * scale` (apply right to left)

**Mat4 structure (column-major):**
```
[ Rx Ry Rz | Tx ]     R = rotation/scale (upper-left 3x3)
[ Rx Ry Rz | Ty ]     T = translation (fourth column)
[ Rx Ry Rz | Tz ]
[  0  0  0 |  1 ]
```

**Transforming Vec3 with Mat4:**
glam's `Mat4` multiplies with `Vec4`, not `Vec3`. Use the `w` component to control behavior:
- **Points** (positions): use `w=1`, affected by translation
  `transformed_point = (mat * vec4(point, 1.0)).xyz`
- **Directions** (ray dir, not normalized): use `w=0`, NOT affected by translation
  `transformed_dir = (mat * vec4(dir, 0.0)).xyz`
- **Normals**: use transpose of inverse matrix, or since we store `inv_transform`:
  `world_normal = (inv_transform.transpose() * vec4(local_normal, 0.0)).xyz`
  (then normalize)

### Implementation Plan

Phase 1: Add new types (non-breaking, ray tracer still works)
- [x] Re-export `Mat4` from `spirv_std::glam` in `rtx-prim/src/types.rs`
      (glam already provides identity, from_translation, from_rotation_y, from_scale,
      inverse, and all the Mul impls we need)
- [x] Create `PrimitiveKind` enum in `rtx-obj`
- [x] Create `Instance` struct in `rtx-obj`
- [x] Implement hardcoded `hit_unit_sphere()` and `hit_unit_quad()` functions

Phase 2: Refactor ray tracer to use instances
- [x] Create new `Scene` type that holds `Array<Instance, N>`
- [x] Update `hit()` logic to iterate instances, transform rays, dispatch to unit primitives
- [x] Migrate existing scenes to use `Instance` with appropriate transforms
- [ ] Remove old `Sphere`, `Quad`, and `List` types once migration is complete

**Note:** The `t` parameter may need adjustment for non-uniform scale. When the ray
is transformed to object space, direction length changes, so the same `t` value
maps to different world-space distances. May need to recompute `t` in world space
after hit, or be careful about comparing `t` values across differently-scaled instances.
