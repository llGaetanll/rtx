# Known Bugs

## Dielectric Rendering Broken After Instance Transform Migration

### Background

We migrated from storing world-space primitives (`Sphere`, `Quad` in a `List`) to an instanced transform system (`Instance` with `Mat4` transforms in a `Scene`). The new system:

1. Stores unit primitives (sphere at origin with radius 1, quad at origin 1x1)
2. Each `Instance` has a `transform` (object -> world) and `inv_transform` (world -> object)
3. Hit testing transforms the ray to object space, tests against the unit primitive, then transforms results back to world space

### Symptoms

Dielectric (glass) materials render incorrectly after the migration:
- In `three_spheres`: The brown sphere behind the glass sphere is now clearly visible inside the glass, whereas before it was invisible (the glass showed an inverted/refracted view of the world)
- The glass sphere no longer shows the characteristic "world flipped upside down" refraction effect
- Light appears to be bending somewhat, but not correctly

Non-dielectric materials (Lambertian, Metal) appear to render correctly.

### Primary Suspect: Double Normal Flip

The most likely cause is incorrect normal handling in the hit testing pipeline.

**Current flow:**

1. `hit_unit_sphere()` calls `rec.set_norm(object_ray, outward_normal)`
2. `set_norm()` checks if ray is hitting front or back face:
   - If `ray.dir().dot(norm) < 0`: front face, stores `norm` as-is
   - If `ray.dir().dot(norm) >= 0`: back face, stores `-norm` (flipped)
3. `transform_hit_to_world()` reads `rec.norm` (potentially already flipped), transforms it to world space, then calls `set_norm(world_ray, world_normal)` **again**

**The bug:** If `set_norm` was called in object space and flipped the normal (because we hit from inside), then `rec.norm` contains a negated normal. We then transform this negated normal and pass it to `set_norm` again, which may flip it back or make an incorrect front_face determination.

**Why this affects dielectrics specifically:** Dielectric scattering relies heavily on `front_face` to determine whether light is entering or exiting the material, which determines the refraction ratio. If `front_face` is wrong, refraction calculations are inverted.

**Proposed fix:** In unit primitive hit functions, store the geometric outward normal directly in `rec.norm` without calling `set_norm`. Then call `set_norm` only once in `transform_hit_to_world` after transforming the geometric normal to world space.

### Secondary Suspects

1. **Normal transformation formula:** We use `inv_transform.transpose() * normal` which is mathematically correct for transforming normals. However, if the transform has negative scale (reflection), this could flip the normal orientation unexpectedly.

2. **Ray direction scaling:** When the ray is transformed to object space, its direction vector changes length (but not direction) due to scaling. The `t` parameter from the hit test is in object space. We recalculate the world hit point correctly, but there could be subtle issues with how this interacts with normal orientation.

3. **Object-space vs world-space ray for front_face:** The determination of front vs back face might need to be consistent about which ray (object or world) is used. Currently we check once with object ray, then again with world ray.

### Testing Ideas

1. **Normal visualization sphere:**
   Create a debug scene with a material that outputs `(normal.x * 0.5 + 0.5, normal.y * 0.5 + 0.5, normal.z * 0.5 + 0.5)` as the color (the classic RGB normal sphere). Place two spheres:
   - One at origin (no translation in transform, just scale)
   - One offset from origin (translation + scale in transform)
   
   If normals are correct, both spheres should show identical RGB gradients. If the offset sphere looks different, the transform is corrupting normals.

2. **Front-face visualization:**
   Create a material that outputs red for `front_face = true` and blue for `front_face = false`. Render a glass sphere from outside - should be all red on the outer surface. Then position camera inside a large glass sphere - should be all blue.

3. **Compare with old code path:**
   Temporarily restore the old `Sphere` hit function (not using instances) for a single test sphere and compare the `HitRecord` values at the same ray to see what differs.

4. **Single ray trace logging:**
   Add debug output for a specific pixel that hits the glass sphere, logging:
   - Object-space ray direction
   - Object-space hit normal (before any set_norm call)
   - `front_face` after object-space set_norm
   - World-space transformed normal
   - `front_face` after world-space set_norm
   - Final refraction direction

### Files Involved

- `crates/rtx-obj/src/instance.rs` - Instance struct, `hit_unit_sphere`, `hit_unit_quad`, `transform_hit_to_world`
- `crates/rtx-mat/src/hit_record.rs` - `HitRecord::set_norm()`
- `crates/rtx-mat/src/dielectric.rs` - Dielectric scattering (uses `front_face` for refraction ratio)
