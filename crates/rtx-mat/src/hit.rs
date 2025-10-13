use crate::HitRecord;
use rtx_prim::Aabb;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::F;

pub trait Hit {
    /// An object can figure out for itself whether it was hit by a `Ray`.
    /// We only check this over a range of `t` for optimization purposes.
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool;

    /// The bounding box of an object. If the object is in motion, this
    /// bounding box contains the entire range of motion.
    fn bbox(&self) -> &Aabb;
}
