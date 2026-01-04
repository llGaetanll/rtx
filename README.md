# TODO
- dynamic dispatch does not work in rust-gpu
- const generics from MaterialTable/TextureTable spread virally through the codebase (see fix below)

## Planned: Remove const generics by using fixed-size arrays

The const generics on `MaterialTable<NL, NM, ND>`, `TextureTable<NS>`, and `List<N>` spread
virally through the codebase (e.g., `Camera::render` has 5 const generic parameters). The fix
is to use fixed-size arrays with runtime counts instead.

### The change

Replace generic-sized arrays with fixed-size arrays plus a count field:

```rust
// Before
pub struct MaterialTable<const NL: usize, const NM: usize, const ND: usize> {
    pub lambertians: [Lambertian; NL],
    pub metals: [Metal; NM],
    pub dielectrics: [Dielectric; ND],
}

// After
pub struct MaterialTable {
    pub lambertians: [Lambertian; 32],
    pub metals: [Metal; 32],
    pub dielectrics: [Dielectric; 32],
    pub lambertian_count: usize,
    pub metal_count: usize,
    pub dielectric_count: usize,
}
```

### Required changes

**1. Add `Default` impls for texture types:**
- `TextureInfo` - default to `{ kind: TextureKind::Solid, index: 0 }`
- `TextureKind` - default to `Solid`
- `SolidTexture` - default to black `Color::new(0., 0., 0.)`

**2. Add `Default` impls for material types:**
- `MaterialInfo` - default to `{ kind: MaterialKind::Lambertian, index: 0 }`
- `MaterialKind` - default to `Lambertian`
- `Lambertian` - uses default `TextureInfo`
- `Metal` - default albedo black, fuzz 0
- `Dielectric` - default refraction index 1.0

**3. Add `Default` impls for object types:**
- `Aabb` - empty/zero bounding box
- `Sphere` - zero radius sphere at origin

**4. Convert tables to fixed-size:**
- `rtx-tex/src/texture_table.rs`: Remove `<const NS: usize>`, add `solid_count: usize`
- `rtx-mat/src/material_table.rs`: Remove `<const NL, NM, ND>`, add count fields
- `rtx-obj/src/list.rs`: Remove `<const N: usize>`, add `count: usize`

**5. Update `Material` trait and impls (remove `<const NS: usize>`):**
- `rtx-mat/src/material.rs` - trait definition
- `rtx-mat/src/lambertian.rs`
- `rtx-mat/src/metal.rs`
- `rtx-mat/src/dielectric.rs`
- `rtx-mat/src/diffuse_light.rs`
- `rtx-mat/src/material_table.rs`

**6. Update `Texture` trait and impls:**
- `rtx-tex/src/texture.rs`
- `rtx-tex/src/solid.rs`

**7. Update Camera:**
- `rtx-util/src/cam.rs`: Remove all const generics from `render()` and `ray_color()`

**8. Update shader:**
- `shader/src/lib.rs`: Initialize fixed-size arrays (unused slots get `Default::default()`)
