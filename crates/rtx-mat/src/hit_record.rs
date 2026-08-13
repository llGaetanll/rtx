use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::Ray;
use rtx_prim::Vec3;

use crate::MaterialInfo;

/// The value of `HitRecord::light_index` for a surface that does not emit.
///
/// Kept here rather than with the light buffer because a hit record is written
/// long before anything asks whether it landed on a light, and this crate cannot
/// see the one that owns the lights.
pub const NOT_A_LIGHT: u32 = u32::MAX;

/// Contains information about a `Ray` hitting a surface
#[derive(Clone)]
#[repr(C)]
pub struct HitRecord {
    /// The point at which there is a hit
    pub p: Point3,

    /// The normal vector on the object, at that point
    pub norm: Vec3,

    /// The material of the hit record
    pub mat: MaterialInfo,

    /// The `t` for which the ray `P(t)` hits the object
    pub t: F,

    /// First surface coordinate of the hit point
    pub u: F,

    /// Second surface coordinate of the hit point
    pub v: F,

    /// Helps us determine the inside of an object from the outside
    pub front_face: bool,

    /// Which entry of the light buffer was hit, or `NOT_A_LIGHT`. Copied from the
    /// instance that was hit, so a ray landing on an emitter can ask what the
    /// odds were of having aimed at it deliberately.
    pub light_index: u32,
}

/// Written out rather than derived: a derived default would leave `light_index`
/// at zero, which is not "no light" but "the first one".
impl Default for HitRecord {
    fn default() -> Self {
        Self {
            p: Point3::default(),
            norm: Vec3::default(),
            mat: MaterialInfo::default(),
            t: 0.,
            u: 0.,
            v: 0.,
            front_face: false,
            light_index: NOT_A_LIGHT,
        }
    }
}

impl HitRecord {
    pub fn set_norm(&mut self, ray: &Ray, norm: Vec3) {
        self.front_face = ray.dir().dot(norm) < 0.;
        self.norm = if self.front_face { norm } else { -norm };
    }
}
