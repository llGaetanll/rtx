use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::F;

use crate::SolidTexture;
use crate::Texture;

#[repr(C)]
pub struct TextureTable<const NS: usize> {
    pub solids: [SolidTexture; NS],
}

impl<const NS: usize> Texture for TextureTable<NS> {
    fn value(&self, info: TextureInfo, u: F, v: F, point: Point3) -> Color {
        match info.kind {
            TextureKind::Solid => self.solids[info.index].value(info, u, v, point),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TextureInfo {
    pub kind: TextureKind,
    pub index: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub enum TextureKind {
    Solid,
}
