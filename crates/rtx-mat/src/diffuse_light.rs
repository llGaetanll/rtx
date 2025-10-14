use crate::Material;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::F;
use rtx_tex::Texture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureTable;

#[derive(Clone)]
#[repr(C)]
pub struct DiffuseLight {
    tex: TextureInfo,
}

impl DiffuseLight {
    // pub fn from_color(color: Color) -> Self {
    //     Self {
    //         tex: Arc::new(SolidTexture::from_color(color)),
    //     }
    // }

    pub fn from_texture(tex: TextureInfo) -> Self {
        Self { tex }
    }

    fn emitted<const NS: usize>(
        &self,
        tex_table: &TextureTable<NS>,
        u: F,
        v: F,
        point: Point3,
    ) -> Color {
        tex_table.value(self.tex, u, v, point)
    }
}

impl Material for DiffuseLight {
    fn emitted<const NS: usize>(
        &self,
        _state: &mut RandState,
        tex_table: &TextureTable<NS>,
        u: F,
        v: F,
        point: Point3,
    ) -> Color {
        self.emitted(tex_table, u, v, point)
    }
}
