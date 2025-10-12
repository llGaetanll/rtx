use crate::HitRecord;
use crate::Material;
use rtx_prim::Color;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::Vec3Ext;
use rtx_tex::Texture;
use rtx_tex::TextureType;

#[derive(Clone)]
pub struct Lambertian {
    tex: TextureType,
}

impl Lambertian {
    // pub const fn from_color(albedo: Color) -> Self {
    //     Self {
    //         tex: SolidTexture::from_color(albedo),
    //     }
    // }

    pub const fn from_texture(tex: TextureType) -> Self {
        Self { tex }
    }
}

impl Material for Lambertian {
    fn scatter(
        &self,
        state: &mut RandState,
        incoming: &Ray,
        hit: &HitRecord,
    ) -> Option<(Ray, Color)> {
        let mut scatter_dir = hit.norm + Vec3::rand_unit(state);

        if scatter_dir.near_zero() {
            scatter_dir = hit.norm;
        }

        let scattered = Ray::new(hit.p, scatter_dir, incoming.time());
        let attenuation: Color = self.tex.value(hit.u, hit.v, hit.p);

        Some((scattered, attenuation))
    }
}
