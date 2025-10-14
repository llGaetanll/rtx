use crate::HitRecord;
use crate::Material;
use rtx_prim::Color;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::Vec3Ext;
use rtx_tex::Texture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureTable;

#[derive(Clone)]
#[repr(C)]
pub struct Lambertian {
    tex: TextureInfo,
}

impl Lambertian {
    // pub const fn from_color(albedo: Color) -> Self {
    //     Self {
    //         tex: SolidTexture::from_color(albedo),
    //     }
    // }

    pub const fn from_texture(tex: TextureInfo) -> Self {
        Self { tex }
    }
}

impl Material for Lambertian {
    fn scatter<const NS: usize>(
        &self,
        state: &mut RandState,
        tex_table: &TextureTable<NS>,
        incoming: &Ray,
        hit: &HitRecord,
        scattered: &mut Ray,
        attenuation: &mut Color,
    ) -> bool {
        let mut scatter_dir = hit.norm + Vec3::rand_unit(state);

        if scatter_dir.near_zero() {
            scatter_dir = hit.norm;
        }

        *scattered = Ray::new(hit.p, scatter_dir, incoming.time());
        *attenuation = tex_table.value(self.tex, hit.u, hit.v, hit.p);

        true
    }
}
