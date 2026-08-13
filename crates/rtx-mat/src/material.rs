use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_tex::TextureTable;

use crate::HitRecord;

pub trait Material {
    /// Takes in an incoming `Ray` and `HitRecord` and computes, if applicable, an outgoing `Ray`,
    /// and an attenuation `Color`.
    fn scatter(
        &self,
        _state: &mut RandState,
        _tex_table: &TextureTable<'_>,
        _incoming: &Ray,
        _rec: &HitRecord,
        _scattered: &mut Ray,
        _attenuation: &mut Color,
    ) -> bool {
        false
    }

    /// Whether this material scatters into one direction rather than over a
    /// spread of them.
    ///
    /// A specular surface reflects light from exactly one incoming direction, so
    /// aiming a ray at a light from it is pointless: the odds of that being the
    /// one direction that reflects into the viewer are zero. Such a surface takes
    /// its light the only way it can, by following the reflection and seeing what
    /// is there.
    fn is_specular(&self, _rec: &HitRecord) -> bool {
        false
    }

    /// The light emitted by this material. Defaults to no light.
    fn emitted(
        &self,
        _state: &mut RandState,
        _tex_table: &TextureTable<'_>,
        _rec: &HitRecord,
        _u: F,
        _v: F,
        _point: Point3,
    ) -> Color {
        Color::new(0., 0., 0.)
    }
}
