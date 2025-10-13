use crate::HitRecord;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::F;
use rtx_tex::TextureTable;

pub trait Material {
    /// Takes in an incoming `Ray` and `HitRecord` and computes, if applicable, an outgoing `Ray`,
    /// and an attenuation `Color`.
    fn scatter<const NS: usize>(
        &self,
        _state: &mut RandState,
        _tex_table: &TextureTable<NS>,
        _incoming: &Ray,
        _rec: &HitRecord,
        _scattered: &mut Ray,
        _attenuation: &mut Color,
    ) -> bool {
        false
    }

    /// The light emitted by this material. Defaults to no light.
    fn emitted<const NS: usize>(
        &self,
        _state: &mut RandState,
        _tex_table: &TextureTable<NS>,
        _u: F,
        _v: F,
        _point: Point3,
    ) -> Color {
        Color::new(0., 0., 0.)
    }
}
