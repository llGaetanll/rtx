use rtx_mat::HitRecord;
use rtx_mat::MaterialInfo;
use rtx_mat::MaterialKind;
use rtx_prim::F;
use rtx_prim::Mat4;
use rtx_prim::PI;
use rtx_prim::Point3;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

/// The kind of unit primitive an instance refers to.
#[derive(Clone, Copy, Default)]
#[repr(u32)]
pub enum PrimitiveKind {
    #[default]
    Sphere,
    Quad,
}

/// An instance of a unit primitive with a transform.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Instance {
    pub kind: PrimitiveKind,
    pub transform: Mat4,
    pub inv_transform: Mat4,
    pub material: MaterialInfo,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            kind: PrimitiveKind::default(),
            transform: Mat4::IDENTITY,
            inv_transform: Mat4::IDENTITY,
            material: MaterialInfo {
                kind: MaterialKind::Lambertian,
                index: 0,
            },
        }
    }
}

impl Instance {
    pub fn new(kind: PrimitiveKind, transform: Mat4, material: MaterialInfo) -> Self {
        let inv_transform = transform.inverse();
        Self {
            kind,
            transform,
            inv_transform,
            material,
        }
    }

    /// Create a sphere instance from center and radius.
    pub fn sphere(center: Point3, radius: F, material: MaterialInfo) -> Self {
        let transform = Mat4::from_translation(center) * Mat4::from_scale(Vec3::splat(radius));
        Self::new(PrimitiveKind::Sphere, transform, material)
    }

    /// Create a quad instance from corner point and two edge vectors.
    /// This matches the current Quad constructor signature.
    pub fn quad(q: Point3, u: Vec3, v: Vec3, material: MaterialInfo) -> Self {
        Self::quad_transformed(Mat4::IDENTITY, q, u, v, material)
    }

    /// Create a quad instance from corner point and two edge vectors, with an
    /// extra transform applied on top (rotation, translation, scaling).
    pub fn quad_transformed(
        xform: Mat4,
        q: Point3,
        u: Vec3,
        v: Vec3,
        material: MaterialInfo,
    ) -> Self {
        // The unit quad has corners at (0,0,0) and (1,1,0) with edges (1,0,0) and (0,1,0).
        // We need a matrix that maps:
        //   (1,0,0) -> u
        //   (0,1,0) -> v
        //   (0,0,1) -> normal direction (for consistent handedness)
        //   origin  -> q
        let normal = u.cross(v).normalize();
        let basis = Mat4::from_cols(
            u.extend(0.0),
            v.extend(0.0),
            normal.extend(0.0),
            q.extend(1.0),
        );
        Self::new(PrimitiveKind::Quad, xform * basis, material)
    }
}

/// The six quads of a box spanning the two opposite corners `a` and `b`, with
/// `xform` applied on top so the box can be rotated, translated or scaled.
pub fn make_box(a: Point3, b: Point3, xform: Mat4, material: MaterialInfo) -> [Instance; 6] {
    let min = a.min(b);
    let max = a.max(b);

    let dx = Vec3::new(max.x - min.x, 0., 0.);
    let dy = Vec3::new(0., max.y - min.y, 0.);
    let dz = Vec3::new(0., 0., max.z - min.z);

    [
        // Front (+z)
        Instance::quad_transformed(xform, Point3::new(min.x, min.y, max.z), dx, dy, material),
        // Right (+x)
        Instance::quad_transformed(xform, Point3::new(max.x, min.y, max.z), -dz, dy, material),
        // Back (-z)
        Instance::quad_transformed(xform, Point3::new(max.x, min.y, min.z), -dx, dy, material),
        // Left (-x)
        Instance::quad_transformed(xform, Point3::new(min.x, min.y, min.z), dz, dy, material),
        // Top (+y)
        Instance::quad_transformed(xform, Point3::new(min.x, max.y, max.z), dx, -dz, material),
        // Bottom (-y)
        Instance::quad_transformed(xform, Point3::new(min.x, min.y, min.z), dx, dz, material),
    ]
}

/// Hit test against a unit sphere (radius 1, centered at origin).
/// Returns true if hit, and populates rec with hit info in object space.
pub fn hit_unit_sphere(ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
    let oc: Vec3 = -ray.orig(); // Vector from sphere center (origin) to ray origin

    let a = ray.dir().length_squared();
    let h = ray.dir().dot(oc);
    let c = oc.length_squared() - 1.0; // radius = 1

    let disc = h * h - a * c;
    if disc < 0.0 {
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
    let norm = p; // For unit sphere at origin, normal = point (normalized by definition)

    // UV mapping for unit sphere
    let theta = (-p.y).acos();
    let phi = (-p.z).atan2(p.x) + PI;
    let u = phi / (2.0 * PI);
    let v = theta / PI;

    rec.t = t;
    rec.u = u;
    rec.v = v;
    rec.p = p;
    rec.norm = norm;

    true
}

/// Hit test against a unit quad (1x1, at origin in XY plane, normal +Z).
/// Corners at (0,0,0) and (1,1,0), edges (1,0,0) and (0,1,0).
/// Returns true if hit, and populates rec with hit info in object space.
pub fn hit_unit_quad(ray: &Ray, t_int: &mut Range<F>, rec: &mut HitRecord) -> bool {
    const EPSILON: F = 1e-8;

    // Unit quad normal is +Z
    let normal = Vec3::new(0.0, 0.0, 1.0);
    let denom = normal.dot(ray.dir());

    // If the ray is parallel to the plane
    if denom.abs() < EPSILON {
        return false;
    }

    // Plane equation: z = 0, so d = 0
    // t = (d - normal.dot(origin)) / denom = -origin.z / denom
    let t = -ray.orig().z / denom;
    if !t_int.contains(&t) {
        return false;
    }

    let intersect = ray.at(t);

    // Check if hit point is within unit quad [0,1] x [0,1]
    let alpha = intersect.x;
    let beta = intersect.y;

    // Not `(0.0..=1.0).contains(..)`: `RangeInclusive::contains` takes its operands
    // by reference, which rust-gpu cannot lower ("is not a logical pointer")
    #[allow(clippy::manual_range_contains)]
    if alpha < 0.0 || alpha > 1.0 || beta < 0.0 || beta > 1.0 {
        return false;
    }

    rec.t = t;
    rec.u = alpha;
    rec.v = beta;
    rec.p = intersect;
    rec.norm = normal;

    true
}

/// Transform a ray from world space to object space.
pub fn transform_ray_to_object(ray: &Ray, inv_transform: &Mat4) -> Ray {
    // Transform origin as a point (w=1), direction as a vector (w=0)
    let orig = (*inv_transform * ray.orig().extend(1.0)).truncate();
    let dir = (*inv_transform * ray.dir().extend(0.0)).truncate();
    Ray::new(orig, dir, ray.time())
}

/// Transform a hit record from object space back to world space.
pub fn transform_hit_to_world(
    rec: &mut HitRecord,
    transform: &Mat4,
    inv_transform: &Mat4,
    world_ray: &Ray,
) {
    // Transform hit point as a point (w=1)
    rec.p = (*transform * rec.p.extend(1.0)).truncate();

    // Transform normal using transpose of inverse
    // Since we have inv_transform, its transpose transforms normals correctly
    let world_normal = (inv_transform.transpose() * rec.norm.extend(0.0))
        .truncate()
        .normalize();

    // Re-set the normal with the world ray to get correct front_face
    rec.set_norm(world_ray, world_normal);
}
