use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_mat::MaterialInfo;
use rtx_prim::Aabb;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;

#[repr(C)]
pub struct Quad {
    q: Point3,
    u: Vec3,
    v: Vec3,

    /// Normal vector to the `Quad`
    norm: Vec3,
    d: F,
    w: Vec3,

    mat: MaterialInfo,

    bbox: Aabb,
}

impl Quad {
    pub fn new(q: Point3, u: Vec3, v: Vec3, mat: MaterialInfo) -> Self {
        let bbox_diag1 = Aabb::from_points(q, q + u + v);
        let bbox_diag2 = Aabb::from_points(q + u, q + v);

        let bbox = Aabb::from_aabbs(&bbox_diag1, &bbox_diag2);

        let n = u.cross(v);
        let norm = n.normalize();
        let d = norm.dot(q);
        let w = n / n.dot(n);

        Self {
            q,
            u,
            v,
            norm,
            d,
            w,
            mat,
            bbox,
        }
    }

    /// Given the hit point in plane coordinates, returns whether it is inside the `Quad`. If so,
    /// updates the `HitRecord`.
    fn is_interior(a: F, b: F, rec: &mut HitRecord) -> bool {
        let unit = 0.0..=1.0;

        if !unit.contains(&a) || !unit.contains(&b) {
            return false;
        }

        rec.u = a;
        rec.v = b;

        true
    }
}

impl Hit for Quad {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        const EPSILON: F = 1e-8;

        let denom = self.norm.dot(ray.dir());

        // If the ray is parallel to the plane
        if denom.abs() < EPSILON {
            return false;
        }

        let t = (self.d - self.norm.dot(ray.orig())) / denom;
        if !t_int.contains(&t) {
            return false;
        }

        let intersect = ray.at(t);

        // Determine if the hit point lies within the quadrilateral
        let planar_hit_point = intersect - self.q;
        let alpha = self.w.dot(planar_hit_point.cross(self.v));
        let beta = self.w.dot(self.u.cross(planar_hit_point));

        if !Self::is_interior(alpha, beta, rec) {
            return false;
        }

        rec.t = t;
        rec.p = intersect;
        rec.mat = self.mat.clone();

        rec.set_norm(ray, self.norm);

        true
    }

    fn bbox(&self) -> &Aabb {
        &self.bbox
    }
}
