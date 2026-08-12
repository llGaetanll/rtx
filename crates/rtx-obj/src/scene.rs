use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_prim::Aabb;
use rtx_prim::Array;
use rtx_prim::F;
use rtx_prim::Range;
use rtx_prim::Ray;

use crate::Instance;
use crate::PrimitiveKind;
use crate::hit_unit_quad;
use crate::hit_unit_sphere;
use crate::transform_hit_to_world;
use crate::transform_ray_to_object;

pub const INST_LEN: usize = 32;

#[repr(C)]
pub struct Scene {
    instances: Array<Instance, INST_LEN>,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        Self {
            instances: Array::new(),
        }
    }

    pub fn push(&mut self, instance: Instance) {
        self.instances.push(instance);
    }
}

impl Hit for Scene {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        let mut hit_anything = false;
        let mut closest = t_int.end;

        for i in 0..self.instances.len() {
            let inst = &self.instances[i];

            // Transform ray to object space
            let obj_ray = transform_ray_to_object(ray, &inst.inv_transform);
            let mut obj_rec = HitRecord::default();
            let mut range = Range::new(t_int.start, closest);

            // Dispatch to unit primitive
            let hit = match inst.kind {
                PrimitiveKind::Sphere => hit_unit_sphere(&obj_ray, &mut range, &mut obj_rec),
                PrimitiveKind::Quad => hit_unit_quad(&obj_ray, &mut range, &mut obj_rec),
            };

            if hit {
                hit_anything = true;
                // Transform hit back to world space
                transform_hit_to_world(&mut obj_rec, &inst.transform, &inst.inv_transform, ray);
                obj_rec.mat = inst.material;
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
