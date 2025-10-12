use core::ops::Range;

use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_prim::Aabb;
use rtx_prim::Ray;
use rtx_prim::F;

// use crate::List;
use crate::Quad;
use crate::Sphere;

pub enum Object {
    // List(List),
    Sphere(Sphere),
    Quad(Quad),
}

impl Hit for Object {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        match self {
            Object::Sphere(sphere) => sphere.hit(ray, t_int, rec),
            Object::Quad(quad) => quad.hit(ray, t_int, rec),
        }
    }

    fn bbox(&self) -> &Aabb {
        todo!()
    }
}
