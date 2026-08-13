# Volumes

Constant-density participating media: smoke, fog, and the two boxes of the book's
Cornell smoke scene. Nothing below is built. This is the plan to pick up if and
when it is.

This is the largest of the three remaining book features, because a medium is the
first thing in the renderer that is not a surface, and three things that assume
surfaces have to learn otherwise.

## Relevant Files

- `crates/rtx-obj/src/instance.rs` - the unit primitives and `world_bbox`
- `crates/rtx-obj/src/scene.rs` - `hit` and `occluded`
- `crates/rtx-mat/src/material_table.rs` - the kinds and their dispatch
- `crates/rtx-util/src/cam.rs` - `ray_color` and `direct_light`, both written
  around a surface normal
- `crates/rtx-mat/src/hit_record.rs` - `HitRecord`, host-side only, so it is free
  to grow

## How a Medium Fits the Flat Array

The book makes `ConstantMedium` a hittable wrapping another hittable. There is no
wrapping here: instances are a flat array of unit primitives with transforms, and
the hierarchy is built over that array. So a medium is not a wrapper but an
ordinary instance whose material happens to be isotropic, and the scattering
happens inside the primitive's hit test.

Two new kinds, one of each sort:

**`primitive_kind::CUBE`** - a solid unit cube spanning 0 to 1, with a hit test
that returns where the ray enters and where it leaves. The existing `box` object
is six independent quads, which is fine for a solid surface and useless here: a
medium needs both crossings from one primitive, and six quads give six unrelated
hits. The slab test is the same arithmetic `bvh::node_hit` already does for a
node's box, kept in object space where the box is the unit cube.

**`material_kind::ISOTROPIC`** - a texture and a density.

```rust
#[repr(C)]
pub struct Isotropic {
    tex: TextureInfo,
    density: F,
    _pad: F,
}
```

Its `scatter` is a uniformly random direction (`Vec3::rand_unit`) with the
texture's value as attenuation. Nothing about it is specular.

Note that `Instance` is full: 80 bytes with every field spoken for. The density
therefore lives on the material, not the instance, which is also where the book
puts it.

## Scattering Inside the Hit Test

A medium instance's hit test is the crossing interval and a coin flip on it:

```
t_enter, t_exit from the solid primitive, clamped to the open range
d = (t_exit - t_enter) * ray.dir().length()          // world distance through it
hit_dist = -(1 / density) * ln(rand_f(state))
if hit_dist > d: the ray passes straight through, no hit
else: t = t_enter + hit_dist / ray.dir().length()
```

The record it fills is unlike a surface's. There is no normal - the scatter is
uniform, so nothing needs one - and `front_face` is meaningless. What the record
gains is a flag:

```rust
/// This hit is a scattering event inside a medium rather than a surface.
/// The normal is not meaningful and no cosine applies to it.
pub medium: bool,
```

`HitRecord` is host-and-shader scratch, never uploaded, so a `bool` costs nothing
in layout.

**Two consequences worth planning for.** The first is that `Hit::hit` needs a
`&mut RandState`, which it does not take today - the density sample is random, and
it happens inside the traversal. Every caller of `hit` and `occluded` already has
a state in hand, so this is a signature change rather than a design problem.

The second is that the hot dispatch in `Scene::hit` grows an arm. The comment
there records that a third branch measured 18% slower on the Cornell box, from
back when the loop scanned every instance; the hierarchy has changed what that
loop looks like, so the figure is worth re-measuring rather than trusting. If it
still bites, `CUBE` and the medium test are one arm, not two: a solid cube is only
useful here.

`world_bbox` also needs its `lo`/`hi` for the cube - 0 to 1 on every axis, the
same corners the quad uses in x and y.

## What Breaks in the Integrator

`ray_color` and `direct_light` are written around a Lambertian's cosine lobe.
Three places assume it:

| Place | For a surface | For a medium |
| --- | --- | --- |
| `direct_light`'s horizon test | `rec.norm.dot(dir) <= 0` rejects | nothing is below a horizon; do not reject |
| `direct_light`'s lobe | `cos_surf / PI` | `1 / (4 * PI)`, no cosine |
| `ray_color`'s `prev_pdf_bsdf` | `cos / PI` | `1 / (4 * PI)` |

All three key off `rec.medium`. Getting the phase function's density wrong here
does not produce an obviously broken image - it produces smoke that is too bright
or too dark by a constant, which is why it is worth checking against a render with
direct lighting disabled, where MIS cannot be wrong because it is not running.

**Shadow rays ignore media.** `occluded` returns true for anything in the way, so
a medium instance left in it would block light completely rather than dim it. The
fix that keeps this task small is to skip isotropic instances there: fog casts no
shadow, and a surface inside fog is lit as if the fog were not there. That is what
the book does too, and it is visibly wrong only for dense media between a surface
and a light. Doing it properly is ratio tracking - accumulate transmittance along
the shadow ray instead of answering yes or no - and is a separate task.

## Scene Format

A medium is an object with a density, and the boundary is what the shape says:

```toml
[[objects]]
name = "smoke"
type = "cube"
material = "smoke"      # type = "isotropic", density on the material
min = [0.0, 0.0, 0.0]
max = [165.0, 330.0, 165.0]
rotate_y = 15.0
translate = [265.0, 0.0, 295.0]
```

`cube` is a solid box and takes the same fields as the existing `box`, which stays
as it is - a hollow six-quad shell is still what a solid-surface box wants.
A sphere with an isotropic material is a spherical medium and needs no new object
type at all.

## Milestones

1. `CUBE` as an ordinary solid primitive with a lambertian on it. It should render
   exactly like the six-quad `box` it duplicates, which is a free correctness test
   of the slab code before any medium depends on it.
2. Isotropic scattering with direct lighting switched off. A grey cube of smoke in
   the Cornell box, noisy but the right brightness.
3. The three MIS cases above, then `scenes/cornell_smoke.toml` as the book has it:
   the two boxes replaced by media, one dark and one light.
