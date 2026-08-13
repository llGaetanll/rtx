use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::Vec3Ext;
use rtx_tex::TextureTable;

use crate::HitRecord;
use crate::Material;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Metal {
    /// Plain floats rather than a `Vec3`, so the host and the shader agree on the
    /// layout. See `SolidTexture` for the reason.
    albedo: [F; 3],
    fuzz: F,
}

// SAFETY: `repr(C)` over three floats of color and one of fuzz, so every bit
// pattern is valid and there is no implicit padding. See `Instance` for why this
// is not derived.
unsafe impl Zeroable for Metal {}
unsafe impl Pod for Metal {}

const _: () = assert!(core::mem::size_of::<Metal>() == 16);

impl Metal {
    pub fn new(albedo: Color, fuzz: F) -> Self {
        Self {
            albedo: [albedo.x, albedo.y, albedo.z],
            fuzz,
        }
    }
}

impl Material for Metal {
    /// A fuzzed metal is really a narrow glossy lobe rather than a mirror, so
    /// this is a simplification. Calling it specular costs a little accuracy on
    /// very fuzzy metal and keeps the direct lighting estimate from having to
    /// evaluate a lobe it has no formula for.
    fn is_specular(&self, _rec: &HitRecord) -> bool {
        true
    }

    fn scatter(
        &self,
        state: &mut RandState,
        _tex_table: &TextureTable<'_>,
        incoming: &Ray,
        hit: &HitRecord,
        scattered: &mut Ray,
        attenuation: &mut Color,
    ) -> bool {
        let reflected = incoming.dir().reflect(hit.norm);
        let reflected = reflected.normalize() + self.fuzz * Vec3::rand_unit(state);

        *scattered = Ray::new(hit.p, reflected, incoming.time());
        *attenuation = Color::new(self.albedo[0], self.albedo[1], self.albedo[2]);

        scattered.dir().dot(hit.norm) > 0.
    }
}
