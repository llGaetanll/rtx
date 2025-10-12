use crate::Material;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::F;
use rtx_tex::Texture;
use rtx_tex::TextureType;

#[derive(Clone)]
pub struct DiffuseLight {
    tex: TextureType,
}

impl DiffuseLight {
    // pub fn from_color(color: Color) -> Self {
    //     Self {
    //         tex: Arc::new(SolidTexture::from_color(color)),
    //     }
    // }

    pub fn from_texture(tex: TextureType) -> Self {
        Self { tex }
    }

    fn emitted(&self, u: F, v: F, point: Point3) -> Color {
        self.tex.value(u, v, point)
    }
}

impl Material for DiffuseLight {
    fn emitted(&self, _state: &mut RandState, u: F, v: F, point: Point3) -> Color {
        self.emitted(u, v, point)
    }
}
