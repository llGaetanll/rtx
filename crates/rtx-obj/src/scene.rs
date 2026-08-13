use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_prim::Aabb;
use rtx_prim::F;
use rtx_prim::Range;
use rtx_prim::Ray;

use crate::Instance;
use crate::hit_unit_quad;
use crate::hit_unit_sphere;
use crate::primitive_kind;
use crate::transform_hit_to_world;
use crate::transform_ray_to_object;

/// The instances making up a scene.
///
/// The host builds these and uploads them; the shader borrows the uploaded buffer
/// rather than constructing anything of its own.
#[derive(Default)]
pub struct Scene<'a> {
    pub instances: &'a [Instance],
}

impl<'a> Scene<'a> {
    pub fn new(instances: &'a [Instance]) -> Self {
        Self { instances }
    }

    /// Whether anything at all lies within `range` along `ray`.
    ///
    /// A shadow ray asks this rather than what `hit` answers: it wants to know if
    /// the way to a light is blocked, not what the nearest thing in the way is.
    /// Stopping at the first blocker rather than scanning on for the closest one
    /// matters because direct lighting makes this the most common ray in the
    /// renderer.
    pub fn occluded(&self, ray: &Ray, range: &Range<F>) -> bool {
        for i in 0..self.instances.len() {
            let inst = &self.instances[i];

            let obj_ray = transform_ray_to_object(ray, &inst.inv_transform);
            let mut obj_rec = HitRecord::default();
            let mut range = *range;

            let hit = match inst.kind {
                primitive_kind::SPHERE => hit_unit_sphere(&obj_ray, &mut range, &mut obj_rec),
                _ => hit_unit_quad(&obj_ray, &mut range, &mut obj_rec),
            };

            if hit {
                return true;
            }
        }

        false
    }
}

impl Hit for Scene<'_> {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        let mut hit_anything = false;
        let mut closest = t_int.end;

        for i in 0..self.instances.len() {
            let inst = &self.instances[i];

            // Transform ray to object space
            let obj_ray = transform_ray_to_object(ray, &inst.inv_transform);
            let mut obj_rec = HitRecord::default();
            let mut range = Range::new(t_int.start, closest);

            // Dispatch to unit primitive. Two arms rather than one per kind plus a
            // fallback: this is the innermost loop of the tracer, and a third
            // branch here measured 18% slower on the cornell box
            let hit = match inst.kind {
                primitive_kind::SPHERE => hit_unit_sphere(&obj_ray, &mut range, &mut obj_rec),
                _ => hit_unit_quad(&obj_ray, &mut range, &mut obj_rec),
            };

            if hit {
                hit_anything = true;
                // Transform hit back to world space
                transform_hit_to_world(&mut obj_rec, &inst.inv_transform, ray);
                obj_rec.mat = inst.material;
                obj_rec.light_index = inst.light_index;
                closest = obj_rec.t;
                *rec = obj_rec;
            }
        }

        hit_anything
    }

    fn bbox(&self) -> &Aabb {
        // TODO: compute proper bounding box when BVH is implemented
        todo!()
    }
}
