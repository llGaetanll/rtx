use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_prim::Aabb;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::F;

use crate::Sphere;

#[repr(C)]
pub struct List<const N: usize> {
    objects: [Sphere; N],
    bbox: Aabb,
}

impl<const N: usize> List<N> {
    // pub fn new() -> Self {
    //     List {
    //         objects: Vec::new(),
    //         bbox: Aabb::empty(),
    //     }
    // }

    pub fn from_objects(objects: [Sphere; N]) -> Self {
        let bbox = objects
            .iter()
            .fold(Aabb::empty(), |bbox, object| bbox.union(object.bbox()));

        Self { objects, bbox }
    }

    // pub fn add(&mut self, object: Object) {
    //     self.bbox.union_mut(object.bbox());
    //
    //     self.objects.push(object);
    // }
}

impl<const N: usize> Hit for List<N> {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        let mut temp_rec = Default::default();
        let mut hit_anything = false;
        let mut closest = t_int.end;

        for object in self.objects.iter() {
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
