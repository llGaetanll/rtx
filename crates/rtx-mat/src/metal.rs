use crate::HitRecord;
use crate::Material;
use rtx_prim::Color;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::Vec3Ext;
use rtx_prim::F;

#[derive(Clone)]
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
    fn scatter(
        &self,
        state: &mut RandState,
        incoming: &Ray,
        hit: &HitRecord,
    ) -> Option<(Ray, Color)> {
        let reflected = incoming.dir().reflect(hit.norm);
        let reflected = reflected.normalize() + self.fuzz * Vec3::rand_unit(state);

        let scattered = Ray::new(hit.p, reflected, incoming.time());
        let attenuation = self.albedo;

        if scattered.dir().dot(hit.norm) > 0. {
            Some((scattered, attenuation))
        } else {
            None
        }
    }
}
