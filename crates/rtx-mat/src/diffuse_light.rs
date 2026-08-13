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
    /// A light shines out of its front face only.
    ///
    /// The direct lighting estimate samples that one face and its density
    /// describes that one face, so a light that also radiated out of its back
    /// would be brighter than any estimate of it. The two have to agree about
    /// what exists, and the front face is the side the edge order already says
    /// the surface points towards.
    fn emitted(
        &self,
        _state: &mut RandState,
        tex_table: &TextureTable<'_>,
        rec: &crate::HitRecord,
        u: F,
        v: F,
        point: Point3,
    ) -> Color {
        if !rec.front_face {
            return Color::new(0., 0., 0.);
        }

        self.emitted_impl(tex_table, u, v, point)
    }
}
