# Image Textures

Mapping a decoded image onto a surface through the `u` and `v` a hit already
carries. Nothing below is built. This is the plan to pick up if and when it is.

## Relevant Files

- `docs/tasks/perlin-noise.md` - the six places any new texture kind touches,
  written out once there
- `crates/rtx-tex/src/texture_table.rs` - `TextureInfo` and the dispatch
- `crates/rtx-obj/src/instance.rs` - `hit_unit_sphere` and `hit_unit_quad`, which
  already compute the `u` and `v` this reads
- `crates/host/src/scene_data.rs` - parsing, and where the file would be decoded
- `crates/host/Cargo.toml` - `image` is already a dependency, for writing renders

## Two Buffers

An image is pixels plus the shape of them, and the two want different bindings:
every image's pixels concatenated into one flat array, and a small entry per image
saying where in it to look.

```rust
#[repr(C)]
pub struct ImageTexture {
    /// Index into the pixel buffer of this image's first component.
    offset: u32,
    width: u32,
    height: u32,
    _pad: u32,
}

// One more binding: pixels: &[F], rgb triples, every image end to end
let base = (tex.offset + (y * tex.width + x) * 3) as usize;
let texel = Color::new(pixels[base], pixels[base + 1], pixels[base + 2]);
```

That is two of the six steps in
[perlin-noise.md](perlin-noise.md#adding-a-texture-kind) done twice, since the
pixel buffer is a binding and a `Vec<f32>` on `SceneData` without being a texture
kind of its own.

Floats rather than packed bytes because the shader wants linear values and the
file holds sRGB ones, so the conversion has to happen somewhere. Doing it on the
host, once per texel at load, is cheaper than doing it in the shader once per
lookup, and it costs memory that these scenes do not miss. A 2048x1024 earth map
is 24 MB as floats. If that ever matters, pack to `[u8; 4]` in a `&[u32]` buffer
and pay for the decode per lookup instead.

## The Lookup

`u` and `v` arrive in 0..1 from `hit_unit_sphere` and `hit_unit_quad`, both of
which already produce them correctly - the sphere's from its spherical
coordinates, the quad's as its two edge parameters. What remains is the
convention, and it is the usual place this goes wrong: `v` is flipped, because
image row zero is the top and `v = 0` is the bottom.

Bilinear filtering is four texel fetches and three lerps, on `u * width - 0.5`.
Worth having from the start rather than as a later improvement: nearest-neighbour
is visibly blocky at the resolutions these renders run at, and the book's earth
sphere is the image where it shows.

Out of range coordinates clamp rather than wrap. Nothing in the book needs tiling,
and clamping is what keeps a coordinate that lands slightly outside from sampling
the far edge.

## Scene Format

As with a noise texture, this is a thing a material's surface can be rather than a
new material:

```toml
[materials.globe]
type = "lambertian"
image = "textures/earthmap.jpg"
```

The path is relative to the scene file, not to the working directory, so a scene
stays a file that can live anywhere - the same property the config format is built
around. Decoding happens in `scene_data::build`, which means `build` needs the
scene's directory passed in, and that a missing or undecodable file becomes one
more error in the style already there: named object or material, what was wrong,
what was expected.

Two materials naming the same path should share one `ImageTexture` entry. A
`BTreeMap<PathBuf, u32>` beside the materials map is the whole of it, and without
it the earth appears twice in the buffer the moment a scene has two globes.

## Milestones

1. A quad with a small test image, unfiltered. Wrong `v` convention and wrong
   stride both show immediately here and nowhere else as clearly.
2. Bilinear, then the book's earth sphere as `scenes/earth.toml`. The seam at
   `u = 0` and the poles are where a spherical mapping goes wrong.
