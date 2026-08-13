use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_mat::HitRecord;
use rtx_mat::MaterialInfo;
use rtx_prim::F;
use rtx_prim::Mat4;
use rtx_prim::PI;
use rtx_prim::Point3;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

/// Discriminants for `Instance::kind`.
///
/// Plain integers rather than an enum: instances are uploaded to the GPU, and
/// only types where every bit pattern is valid can be reinterpreted as bytes.
pub mod primitive_kind {
    pub const SPHERE: u32 = 0;
    pub const QUAD: u32 = 1;
}

/// An instance of a unit primitive with a transform.
/// The matrix comes first and the tags last so the type is 80 bytes with nothing
/// implicit between the fields, which is what lets an array of them be handed to
/// the GPU as raw bytes.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Instance {
    pub inv_transform: Mat4,
    pub material: MaterialInfo,
    pub kind: u32,
    /// Which entry of the light buffer this instance is, or `NOT_A_LIGHT`.
    ///
    /// A ray that lands on an emitter has to be able to ask what the odds were of
    /// having aimed at it instead, and that question is about the light's area
    /// and facing rather than the instance's. This says where to look them up.
    ///
    /// These four bytes exist whether or not they are written down: the matrix
    /// aligns the struct to sixteen, so its 76 bytes of fields round up to 80
    /// either way. They used to be an explicit pad for exactly that reason, and
    /// spelling them out is still what keeps them initialized.
    pub light_index: u32,
}

// SAFETY: `repr(C)` over a matrix of sixteen floats, two `u32`s of material
// reference, a `u32` tag and a `u32` light reference. Every field is a float or an
// integer, so all bit patterns are valid, and the assertion below pins the size
// so a new field cannot silently introduce padding. Not derived because the
// shader builds glam without its bytemuck feature, which cannot be enabled there.
unsafe impl Zeroable for Instance {}
unsafe impl Pod for Instance {}

const _: () = assert!(core::mem::size_of::<Mat4>() == 64);
const _: () = assert!(core::mem::size_of::<Instance>() == 80);

impl Default for Instance {
    fn default() -> Self {
        Self {
            inv_transform: Mat4::IDENTITY,
            material: MaterialInfo::default(),
            kind: primitive_kind::SPHERE,
            light_index: crate::NOT_A_LIGHT,
        }
    }
}

impl Instance {
    pub fn new(kind: u32, transform: Mat4, material: MaterialInfo) -> Self {
        Self {
            inv_transform: transform.inverse(),
            material,
            kind,
            light_index: crate::NOT_A_LIGHT,
        }
    }

    /// Create a sphere instance from center and radius.
    pub fn sphere(center: Point3, radius: F, material: MaterialInfo) -> Self {
        let transform = Mat4::from_translation(center) * Mat4::from_scale(Vec3::splat(radius));
        Self::new(primitive_kind::SPHERE, transform, material)
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
        Self::new(primitive_kind::QUAD, xform * basis, material)
    }
}

/// The six faces of a box spanning the two opposite corners `a` and `b`, each as
/// the corner and the two edges leading away from it, with `xform` already
/// applied so the geometry is where it will actually be.
///
/// An emissive box is six lights as well as six quads, and a light is described
/// by this geometry rather than by a transform. Returning the faces rather than
/// finished instances is what lets one table of them feed both, so the light a
/// ray is aimed at and the surface it eventually hits stay the same
/// parallelogram.
///
/// Applying the transform here rather than leaving it on each instance is the
/// same thing for the rotations and translations a scene file can ask for.
pub fn box_quads(a: Point3, b: Point3, xform: Mat4) -> [(Point3, Vec3, Vec3); 6] {
    let min = a.min(b);
    let max = a.max(b);

    let dx = Vec3::new(max.x - min.x, 0., 0.);
    let dy = Vec3::new(0., max.y - min.y, 0.);
    let dz = Vec3::new(0., 0., max.z - min.z);

    // Written out per face rather than mapped over an array of them: rust-gpu
    // cannot lower `array::map`, and this crate is compiled for the GPU whole
    let face = |q: Point3, u: Vec3, v: Vec3| {
        (
            xform.transform_point3(q),
            xform.transform_vector3(u),
            xform.transform_vector3(v),
        )
    };

    [
        // Front (+z)
        face(Point3::new(min.x, min.y, max.z), dx, dy),
        // Right (+x)
        face(Point3::new(max.x, min.y, max.z), -dz, dy),
        // Back (-z)
        face(Point3::new(max.x, min.y, min.z), -dx, dy),
        // Left (-x)
        face(Point3::new(min.x, min.y, min.z), dz, dy),
        // Top (+y)
        face(Point3::new(min.x, max.y, max.z), dx, -dz),
        // Bottom (-y)
        face(Point3::new(min.x, min.y, min.z), dx, dz),
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
pub fn transform_hit_to_world(rec: &mut HitRecord, inv_transform: &Mat4, world_ray: &Ray) {
    // The object ray is the world ray under an affine map, and its direction is
    // left unnormalized, so both rays reach the surface at the same t.
    rec.p = world_ray.at(rec.t);

    // Transform normal using transpose of inverse
    // Since we have inv_transform, its transpose transforms normals correctly
    let world_normal = (inv_transform.transpose() * rec.norm.extend(0.0))
        .truncate()
        .normalize();

    // Re-set the normal with the world ray to get correct front_face
    rec.set_norm(world_ray, world_normal);
}
