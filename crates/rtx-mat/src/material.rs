use crate::HitRecord;
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
        state: &mut RandState,
        _incoming: &Ray,
        _rec: &HitRecord,
    ) -> Option<(Ray, Color)> {
        None
    }

    /// The light emitted by this material. Defaults to no light.
    fn emitted(&self, state: &mut RandState, _u: F, _v: F, _point: Point3) -> Color {
        Color::new(0., 0., 0.)
    }
}
