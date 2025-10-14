use crate::HitRecord;
use crate::Material;
use rtx_prim::Color;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::Vec3Ext;
use rtx_prim::F;
use rtx_tex::TextureTable;

#[derive(Clone)]
#[repr(C)]
pub struct Metal {
    albedo: Color,
    fuzz: F,
}

impl Metal {
    pub fn new(albedo: Color, fuzz: F) -> Self {
        Self { albedo, fuzz }
    }
}

impl Material for Metal {
    fn scatter<const NS: usize>(
        &self,
        state: &mut RandState,
        _tex_table: &TextureTable<NS>,
        incoming: &Ray,
        hit: &HitRecord,
        scattered: &mut Ray,
        attenuation: &mut Color,
    ) -> bool {
        let reflected = incoming.dir().reflect(hit.norm);
        let reflected = reflected.normalize() + self.fuzz * Vec3::rand_unit(state);

        if scattered.dir().dot(hit.norm) > 0. {
            *scattered = Ray::new(hit.p, reflected, incoming.time());
            *attenuation = self.albedo;

            return true;
        }

        false
    }
}
