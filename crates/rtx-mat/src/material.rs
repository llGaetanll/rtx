use crate::Dielectric;
use crate::DiffuseLight;
use crate::HitRecord;
use crate::Lambertian;
use crate::Metal;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::F;

pub trait Material {
    /// Takes in an incoming `Ray` and `HitRecord` and computes, if applicable, an outgoing `Ray`,
    /// and an attenuation `Color`.
    fn scatter(
        &self,
        _state: &mut RandState,
        _incoming: &Ray,
        _rec: &HitRecord,
    ) -> Option<(Ray, Color)> {
        None
    }

    /// The light emitted by this material. Defaults to no light.
    fn emitted(&self, _state: &mut RandState, _u: F, _v: F, _point: Point3) -> Color {
        Color::new(0., 0., 0.)
    }
}

#[derive(Clone)]
pub enum MaterialType {
    Lambertian(Lambertian),
    Metal(Metal),
    Dielectric(Dielectric),
    DiffuseLight(DiffuseLight),
}

impl Material for MaterialType {
    fn scatter(
        &self,
        state: &mut RandState,
        incoming: &Ray,
        rec: &HitRecord,
    ) -> Option<(Ray, Color)> {
        match self {
            MaterialType::Lambertian(lambertian) => lambertian.scatter(state, incoming, rec),
            MaterialType::Metal(metal) => metal.scatter(state, incoming, rec),
            MaterialType::Dielectric(dielectric) => dielectric.scatter(state, incoming, rec),
            MaterialType::DiffuseLight(diffuse_light) => {
                diffuse_light.scatter(state, incoming, rec)
            }
        }
    }

    fn emitted(&self, state: &mut RandState, u: F, v: F, point: Point3) -> Color {
        match self {
            MaterialType::Lambertian(lambertian) => lambertian.emitted(state, u, v, point),
            MaterialType::Metal(metal) => metal.emitted(state, u, v, point),
            MaterialType::Dielectric(dielectric) => dielectric.emitted(state, u, v, point),
            MaterialType::DiffuseLight(diffuse_light) => diffuse_light.emitted(state, u, v, point),
        }
    }
}
