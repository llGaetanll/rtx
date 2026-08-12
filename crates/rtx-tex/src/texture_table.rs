use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;

use crate::SolidTexture;
use crate::Texture;

/// Discriminants for `TextureInfo::kind`.
///
/// Plain integers rather than an enum: this type is uploaded to the GPU, and only
/// types where every bit pattern is valid can be reinterpreted as bytes.
pub mod texture_kind {
    pub const SOLID: u32 = 0;
}

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
pub struct TextureInfo {
    pub kind: u32,
    pub index: u32,
}

impl TextureInfo {
    pub const fn solid(index: u32) -> Self {
        Self {
            kind: texture_kind::SOLID,
            index,
        }
    }
}

/// Every texture in a scene, grouped by kind. The host builds these and uploads
/// them; the shader only reads them.
#[derive(Default)]
pub struct TextureTable<'a> {
    pub solids: &'a [SolidTexture],
}

impl Texture for TextureTable<'_> {
    fn value(&self, info: TextureInfo, u: F, v: F, point: Point3) -> Color {
        self.solids[info.index as usize].value(info, u, v, point)
    }
}
