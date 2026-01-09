use rtx_prim::Array;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;

use crate::SolidTexture;
use crate::Texture;

pub const LEN_TEX_TBL: usize = 32;

#[repr(C)]
pub struct TextureTable {
    pub solids: Array<SolidTexture, LEN_TEX_TBL>,
}

impl TextureTable {
    pub fn new() -> Self {
        Self {
            solids: Array::new(),
        }
    }
}

impl Default for TextureTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Texture for TextureTable {
    fn value(&self, info: TextureInfo, u: F, v: F, point: Point3) -> Color {
        match info.kind {
            TextureKind::Solid => self.solids[info.index].value(info, u, v, point),
        }
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct TextureInfo {
    pub kind: TextureKind,
    pub index: usize,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub enum TextureKind {
    #[default]
    Solid,
}
