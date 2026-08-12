use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_tex::Texture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureTable;

use crate::Material;

#[derive(Clone, Copy, Default, Pod, Zeroable)]
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

    fn emitted_impl(&self, tex_table: &TextureTable<'_>, u: F, v: F, point: Point3) -> Color {
        tex_table.value(self.tex, u, v, point)
    }
}

impl Material for DiffuseLight {
    fn emitted(
        &self,
        _state: &mut RandState,
        tex_table: &TextureTable<'_>,
        _rec: &crate::HitRecord,
        u: F,
        v: F,
        point: Point3,
    ) -> Color {
        self.emitted_impl(tex_table, u, v, point)
    }
}
