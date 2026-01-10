# TODO

## Features (Ray Tracing in One Weekend series)

- [x] Lighting (emissive materials, diffuse lights)
  - [x] Add `DiffuseLight` to `MaterialTable` (new variant in `MaterialKind`, new array field)
  - [x] Implement `emitted()` dispatch in `MaterialTable` (currently a `todo!()`)
  - [x] Fix `ray_color` in `cam.rs` to accumulate emission with throughput
  - [x] Create test scene with emissive quad (Cornell box)
- [x] Quads (axis-aligned and arbitrary)
- [ ] Transforms (translate, rotate, scale)
- [ ] Image textures
- [ ] Noise textures (Perlin noise)

## Testing Infrastructure

- [x] CLI with `live` and `test` subcommands
  - [x] Add clap dependency, `live` subcommand (current behavior), `test` placeholder
  - [x] `live --scene <name>` to select which scene to display (flag added, not yet functional)
- [ ] Refactor host rendering into reusable pieces
  - [ ] Extract wgpu setup (device, queue, pipeline) into reusable module
  - [ ] Create function to render to offscreen texture
- [ ] Offscreen rendering and image output
  - [ ] Render to texture, read pixels back to CPU
  - [ ] Save as PNG using `image` crate
- [ ] Multiple scenes via shader entry points
  - [ ] Multiple fragment entry points in shader (one per scene)
  - [ ] Host selects entry point at runtime
- [ ] Grid composition for `test` command
  - [ ] Render each scene at 1080p
  - [ ] Stitch into 4x4 grid image

## Technical Debt

- [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
