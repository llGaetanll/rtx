use rtx_prim::F;
use rtx_prim::Range;
use rtx_prim::Ray;

use crate::HitRecord;

pub trait Hit {
    /// An object can figure out for itself whether it was hit by a `Ray`.
    /// We only check this over a range of `t` for optimization purposes.
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool;
}
