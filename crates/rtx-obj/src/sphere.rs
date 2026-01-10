use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_mat::MaterialInfo;
use rtx_prim::Aabb;
use rtx_prim::F;
use rtx_prim::PI;
use rtx_prim::Point3;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

#[derive(Default)]
#[repr(C)]
pub struct Sphere {
    center: Ray,
    radius: F,
    mat: MaterialInfo,

    bbox: Aabb,
}

impl Sphere {
    pub fn fixed(center: Point3, radius: F, mat: MaterialInfo) -> Self {
        let rvec = Vec3::new(radius, radius, radius);
        let bbox = Aabb::from_points(center - rvec, center + rvec);

        Self {
            center: Ray::new(center, Vec3::new(0., 0., 0.), 0.),
            radius: radius.max(0.),
            mat,
            bbox,
        }
    }

    /// Given a point `p` on the sphere of radius one, centered at
    /// the origin, returns surface coordinates `(u, v)` both in
    /// `[0, 1]` where:
    /// - `u` is the angle around the Y axis from x = -1
    /// - `v` is the angle from y = -1 to y = +1
    ///
    /// <1 0 0> yields <0.50 0.50>       <-1  0  0> yields <0.00 0.50>
    /// <0 1 0> yields <0.50 1.00>       < 0 -1  0> yields <0.50 0.00>
    /// <0 0 1> yields <0.25 0.50>       < 0  0 -1> yields <0.75 0.50>
    fn get_sphere_uv(p: Point3) -> (F, F) {
        let theta = (-p.y).acos();
        let phi = (-p.z).atan2(p.x) + PI;

        let u = phi / (2. * PI);
        let v = theta / PI;

        (u, v)
    }
}

impl Hit for Sphere {
    fn hit(&self, ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
        let curr_center = self.center.at(ray.time());
        let oc: Vec3 = curr_center - ray.orig();

        let a = ray.dir().length_squared();
        let h = ray.dir().dot(oc);
        let c = oc.length_squared() - (self.radius * self.radius);

        let disc = h * h - a * c;
        if disc < 0. {
            return false;
        }

        let sqrtd = disc.sqrt();

        // Find the nearest root that lies in the acceptable range
        let mut root = (h - sqrtd) / a;
        if !t_int.contains(&root) {
            root = (h + sqrtd) / a;
            if !t_int.contains(&root) {
                return false;
            }
        }

        let t = root;
        let p = ray.at(t);
        let norm = (p - curr_center) / self.radius; // The outward normal

        let (u, v) = Sphere::get_sphere_uv(norm);

        rec.t = root;
        rec.u = u;
        rec.v = v;
        rec.p = p;
        rec.mat = self.mat;

        rec.set_norm(ray, norm);

        true
    }

    fn bbox(&self) -> &Aabb {
        &self.bbox
    }
}
