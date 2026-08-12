use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::Ray;
use rtx_prim::Vec3;

use crate::MaterialInfo;

/// Contains information about a `Ray` hitting a surface
#[derive(Clone, Default)]
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
}

impl HitRecord {
    pub fn set_norm(&mut self, ray: &Ray, norm: Vec3) {
        self.front_face = ray.dir().dot(norm) < 0.;
        self.norm = if self.front_face { norm } else { -norm };
    }
}
