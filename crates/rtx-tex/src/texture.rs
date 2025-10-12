use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::F;

use crate::SolidTexture;

pub trait Texture {
    fn value(&self, u: F, v: F, point: Point3) -> Color;
}

#[derive(Clone)]
pub enum TextureType {
    SolidTexture(SolidTexture),
}

impl Texture for TextureType {
    fn value(&self, u: F, v: F, point: Point3) -> Color {
        match self {
            TextureType::SolidTexture(solid_texture) => solid_texture.value(u, v, point),
        }
    }
}
