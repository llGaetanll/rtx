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

**1. Add `Default` impls for texture types:** ✅
- `TextureInfo` - default to `{ kind: TextureKind::Solid, index: 0 }`
- `TextureKind` - default to `Solid`
- `SolidTexture` - default to black `Color::new(0., 0., 0.)`

**2. Add `Default` impls for material types:** ✅
- `MaterialInfo` - default to `{ kind: MaterialKind::Lambertian, index: 0 }`
- `MaterialKind` - default to `Lambertian`
- `Lambertian` - uses default `TextureInfo`
- `Metal` - default albedo black, fuzz 0
- `Dielectric` - default refraction index 1.0

**3. Add `Default` impls for object types:** ✅
- `Aabb` - empty/zero bounding box
- `Sphere` - zero radius sphere at origin

**4. Add `Copy` impls for types stored in arrays:** ✅
- `SolidTexture`, `Lambertian`, `Metal`, `Dielectric`
- (Sphere excluded due to large size - 72 bytes)

**5. Create `Array<T, N>` type in `rtx-prim`:** ✅
- Fixed-size array with runtime length tracking
- Requires `T: Copy + Default` for `new()`
- Implements `From<[T; N]>`, `Deref`, `DerefMut`

**6. Convert tables to fixed-size:** 🚧 BLOCKED
- `rtx-tex/src/texture_table.rs`: Remove `<const NS: usize>`, add `solid_count: usize`
- `rtx-mat/src/material_table.rs`: Remove `<const NL, NM, ND>`, add count fields
- `rtx-obj/src/list.rs`: Keep `<const N: usize>` (Sphere is not Copy)

**7. Update `Material` trait and impls (remove `<const NS: usize>`):**
- `rtx-mat/src/material.rs` - trait definition
- `rtx-mat/src/lambertian.rs`
- `rtx-mat/src/metal.rs`
- `rtx-mat/src/dielectric.rs`
- `rtx-mat/src/diffuse_light.rs`
- `rtx-mat/src/material_table.rs`

**8. Update `Texture` trait and impls:**
- `rtx-tex/src/texture.rs`
- `rtx-tex/src/solid.rs`

**9. Update Camera:**
- `rtx-util/src/cam.rs`: Remove all const generics from `render()` and `ray_color()`

**10. Update shader:**
- `shader/src/lib.rs`: Initialize fixed-size arrays (unused slots get `Default::default()`)

---

## SPIR-V Limitation: No Runtime Slice Indexing

The `Array<T, N>` type implements `Deref` to `&[T]` using `&self.data[..self.len]`. This works
on native Rust but **fails on SPIR-V** because rust-gpu cannot cast from a fixed-size array
pointer (`*[T; N]`) to a dynamically-sized slice pointer (`*[T]`) at runtime.

SPIR-V requires all sizes to be known at compile time. A slice `&[T]` is a fat pointer
(pointer + length), and the conversion from a fixed-size array to a slice with a runtime-
computed length is not supported.

### Potential workarounds

1. **Avoid `Deref` to slice** - provide direct index access methods (`fn get(&self, i: usize) -> &T`)
   that work on the fixed-size array internally
2. **Return full array** - expose `&[T; N]` and let callers track `len` separately
3. **Const generic length** - keep const generics (defeats the purpose of this refactor)
4. **Unroll loops at compile time** - use const generics for iteration bounds
