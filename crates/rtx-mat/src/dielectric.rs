use crate::HitRecord;
use crate::Material;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::rand;

use rtx_tex::TextureTable;
use spirv_std::num_traits::Float;

#[derive(Clone)]
#[repr(C)]
pub struct Dielectric {
    /// The refraction index of the material
    r: F,
}

impl Default for Dielectric {
    fn default() -> Self {
        Self { r: 1.0 }
    }
}

impl Dielectric {
    pub fn new(r: F) -> Self {
        Self { r }
    }

    /// Compute Schlick's approximation of reflectance
    fn schlick(cos: F, r: F) -> F {
        let r0 = (1. - r) / (1. + r);
        let r0 = r0 * r0;

        r0 + (1. - r0) * (1. - cos).powi(5)
    }
}

impl Material for Dielectric {
    fn scatter<const NS: usize>(
        &self,
        state: &mut RandState,
        _tex_table: &TextureTable<NS>,
        incoming: &Ray,
        hit: &HitRecord,
        scattered: &mut Ray,
        attenuation: &mut Color,
    ) -> bool {
        let r = if hit.front_face { 1.0 / self.r } else { self.r };

        let unit_dir = incoming.dir().normalize();

        let cos_theta = (-unit_dir).dot(hit.norm).min(1.);
        let sin_theta = 1. - cos_theta * cos_theta;

        let cannot_refract = r * sin_theta > 1.0;
        let d: F = rand::rand_f(state);
        let dir = if cannot_refract || Self::schlick(cos_theta, r) > d {
            unit_dir.reflect(hit.norm)
        } else {
            unit_dir.refract(hit.norm, r)
        };

        *scattered = Ray::new(hit.p, dir, incoming.time());
        *attenuation = Color::new(1., 1., 1.);

        true
    }
}
