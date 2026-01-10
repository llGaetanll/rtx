# TODO

## Features (Ray Tracing in One Weekend series)

- [ ] Lighting (emissive materials, diffuse lights)
- [x] Quads (axis-aligned and arbitrary)
- [ ] Image textures
- [ ] Noise textures (Perlin noise)

## Technical Debt

- [ ] `Array<T, N>` currently requires `T: Copy + Default` because we use `[Default::default(); N]` for initialization. This is overly restrictive. Ideally, `T` would only need to be "zeroable" (all zero bytes is a valid default). This would allow types like `Sphere` that aren't `Copy` but can be safely zero-initialized. The goal is something like `core::array::from_fn(|_| Default::default())` but in a form rust-gpu accepts.
