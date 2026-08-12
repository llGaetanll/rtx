use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;

use crate::Texture;
use crate::TextureInfo;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct SolidTexture {
    /// Plain floats, not a `Vec3`. glam's vectors are `repr(simd)` when compiled
    /// for SPIR-V and occupy sixteen bytes there against twelve on the host, so a
    /// vector in an uploaded type would have the two disagree about the layout.
    color: [F; 3],
    /// Rounds the type up to 16 bytes, the alignment a three component vector has
    /// in a GPU buffer.
    _pad: F,
}

// SAFETY: `repr(C)` over three floats and an explicit pad, so every bit pattern
// is valid and there is no implicit padding. See `Instance` for why this is not
// derived.
unsafe impl Zeroable for SolidTexture {}
unsafe impl Pod for SolidTexture {}

const _: () = assert!(core::mem::size_of::<SolidTexture>() == 16);

impl SolidTexture {
    pub const fn from_color(color: Color) -> Self {
        Self::rgb(color.x, color.y, color.z)
    }

    pub const fn rgb(r: F, g: F, b: F) -> Self {
        Self {
            color: [r, g, b],
            _pad: 0.,
        }
    }
}

impl Texture for SolidTexture {
    fn value(&self, _info: TextureInfo, _u: F, _v: F, _point: Point3) -> Color {
        Color::new(self.color[0], self.color[1], self.color[2])
    }
}
