# Noise Textures

A procedural texture whose value comes from a lattice of pseudo-random gradients,
for marble and terrain-like surfaces. Nothing below is built. This is the plan to
pick up if and when it is.

It is also the smallest possible second texture kind, so the checklist under
[Adding a Texture Kind](#adding-a-texture-kind) is written here and referred to
from [image-textures.md](image-textures.md).

## Relevant Files

- `crates/rtx-tex/src/solid.rs` - the one texture kind there is, and the shape a
  second one copies
- `crates/rtx-tex/src/texture_table.rs` - `TextureInfo`, `texture_kind` and the
  dispatch that currently is not one
- `crates/host/src/scene_data.rs` - `MaterialDef`, and the `solid` helper that
  interns a texture
- `crates/shader/src/lib.rs` - the bindings, listed once per entry point
- `crates/host/src/gpu.rs` - `SCENE_BINDINGS` and `upload_scene`

## The Texture

Perlin's permutation and gradient tables are replaced by an integer hash of the
lattice coordinate, so a noise texture is a scale and a seed and needs no buffer
of its own beyond the array of textures:

```rust
#[repr(C)]
pub struct NoiseTexture {
    scale: F,
    /// How many octaves of turbulence are summed. Zero is plain noise.
    depth: u32,
    seed: u32,
    _pad: u32,
}
```

The gradient at a lattice corner is derived from `(ix, iy, iz, seed)` through the
same kind of integer mix `gen_state` in the shader already uses, mapped onto the
unit sphere. Three hashes give the three components; normalizing them is what
makes it a gradient rather than value noise, and is the difference between the
book's first noise image and its last.

The value at a point is then the usual trilinear blend of the eight corner
gradients dotted with the offsets to them, smoothed by `t*t*(3-2*t)` per axis, and
turbulence is `depth` octaves of that at doubling frequency and halving weight.
The book's marble is `0.5 * (1 + sin(scale * p.z + 10 * turbulence))`.

This is not bit-identical to the book's output, because the book's tables are a
shuffled permutation and this is a hash. It is visually the same thing, and it
costs no binding and no upload.

## Adding a Texture Kind

The texture table is the material table's smaller twin, and grows the same way.
Six places:

1. **The type** in `crates/rtx-tex/`, `repr(C)` and `Pod`, padded to a multiple of
   16 bytes. `SolidTexture` explains why in its comments: the shader's `glam` is
   `repr(simd)`, so uploaded types hold plain `[F; 3]` rather than a `Vec3`, and a
   three-component vector is 16-byte aligned in a buffer.
2. **A discriminant** in `texture_kind` and a constructor on `TextureInfo`.
3. **A field on `TextureTable`, and a real dispatch in its `value`.** Today it
   indexes `solids` unconditionally, because with one kind the `kind` field is
   dead. The second kind is what turns that line into a `match` on `info.kind`,
   with the last arm as the fallback for an unknown value read out of a buffer -
   the same shape as `MaterialTable::scatter`.
4. **A binding** in the shader, added to *both* `trace_fs` and `trace_cs`, whose
   signatures list the same bindings twice.
5. **`SCENE_BINDINGS`** bumped in `gpu.rs`, and a `storage_buffer(...)` line in
   `upload_scene` in the matching position. Empty arrays are handled there
   already: an unused kind uploads one zeroed element, since a zero sized binding
   is not allowed.
6. **A `Vec` on `SceneData`**, an interning helper beside `solid`, and the TOML.

## Scene Format

Materials currently take a `color` and intern a solid texture behind it. A
texture kind is a second thing a material's surface can be, so the field becomes
either-or rather than a new material type:

```toml
[materials.ground]
type = "lambertian"
noise = { scale = 4.0, depth = 7 }
```

`#[serde(untagged)]` over a two-variant enum of `{ color }` and `{ noise }` keeps
every existing scene file parsing unchanged. Note that only `lambertian` and
`diffuse_light` hold a `TextureInfo` at all; `metal` and `dielectric` store plain
values, so a noise-textured metal is a separate change to `Metal`.

The seed is not written in the file. Deriving it from the material's name keeps a
scene reproducible without asking anyone to pick a number.

## Milestones

1. `NoiseTexture` with `depth = 0`, wired through all six places above, on a
   ground quad. Grey lumps at the right scale means the lattice and the hash are
   right.
2. Turbulence, then the marble sine. `scenes/perlin_spheres.toml` as the book has
   it: two large spheres, one noise, over a noise ground.
