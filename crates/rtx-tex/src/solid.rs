use crate::Texture;
use crate::TextureInfo;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::F;

#[derive(Clone)]
pub struct SolidTexture {
    color: Color,
}

impl SolidTexture {
    pub const fn from_color(color: Color) -> Self {
        Self { color }
    }

    pub const fn rgb(r: F, g: F, b: F) -> Self {
        Self {
            color: Color::new(r, g, b),
        }
    }
}

impl Texture for SolidTexture {
    fn value(&self, _info: TextureInfo, _u: F, _v: F, _point: Point3) -> Color {
        self.color
    }
}
