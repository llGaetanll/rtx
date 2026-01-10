use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_prim::Aabb;
use rtx_prim::F;
use rtx_prim::Range;
use rtx_prim::Ray;

use crate::Quad;
use crate::Sphere;

#[repr(C)]
pub struct List<const NS: usize, const NQ: usize> {
    spheres: [Sphere; NS],
    quads: [Quad; NQ],
    bbox: Aabb,
}

impl<const NS: usize, const NQ: usize> List<NS, NQ> {
    pub fn from_objects(spheres: [Sphere; NS], quads: [Quad; NQ]) -> Self {
        let mut bbox = Aabb::empty();

        for i in 0..NS {
            bbox = bbox.union(spheres[i].bbox());
        }

        for i in 0..NQ {
            bbox = bbox.union(quads[i].bbox());
        }

        Self {
            spheres,
            quads,
            bbox,
        }
    }
}

impl<const NS: usize, const NQ: usize> Hit for List<NS, NQ> {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        let mut temp_rec = Default::default();
        let mut hit_anything = false;
        let mut closest = t_int.end;

        for i in 0..NS {
            let object = &self.spheres[i];
            let mut range = Range::new(t_int.start, closest);
            if object.hit(ray, &mut range, &mut temp_rec) {
                hit_anything = true;
                closest = temp_rec.t;
                *rec = temp_rec.clone();
            }
        }

        for i in 0..NQ {
            let object = &self.quads[i];
            let mut range = Range::new(t_int.start, closest);
            if object.hit(ray, &mut range, &mut temp_rec) {
                hit_anything = true;
                closest = temp_rec.t;
                *rec = temp_rec.clone();
            }
        }

        hit_anything
    }

    fn bbox(&self) -> &Aabb {
        &self.bbox
    }
}
