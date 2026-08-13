use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_mat::MaterialInfo;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Vec3;
use rtx_prim::rand;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

/// Discriminants for `Light::kind`.
///
/// Plain integers rather than an enum, for the same reason as `Instance::kind`:
/// lights are uploaded to the GPU, and only types where every bit pattern is
/// valid can be reinterpreted as bytes.
pub mod light_kind {
    pub const QUAD: u32 = 0;
}

/// The value of `Instance::light_index` on an instance that is not an emitter.
///
/// Defined alongside the hit record that also carries it, so the two cannot drift
/// apart into two different sentinels.
pub use rtx_mat::NOT_A_LIGHT;

/// Below this cosine the area to solid angle conversion divides by roughly
/// nothing, and the sample is worthless anyway, so it is dropped rather than
/// turned into a firefly.
const EPSILON_COS: F = 1e-6;

/// An emitter the renderer can aim a ray at.
///
/// This is world space geometry the host precomputes, not a reference back into
/// the instance buffer. An `Instance` only stores the inverse of its transform,
/// so sampling a point on one would mean inverting a matrix per shadow ray.
///
/// The fields are named for the quad that is the only kind so far. A sphere light
/// would reuse the same slots rather than growing the type: `q` as its centre and
/// `area` as its radius, leaving `u`, `v` and `norm` unread. Nothing outside
/// `quad_sample` and `quad_pdf` may read a field without first checking `kind`.
///
/// Each three component vector is followed by one scalar, because a vector is
/// aligned to sixteen bytes in a GPU buffer and the type is handed over as raw
/// bytes. `SolidTexture` pads for the same reason.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Light {
    /// Quad: the corner the two edges lead away from.
    pub q: [F; 3],

    /// Total surface area, whose reciprocal is the density of picking a point on
    /// this light uniformly.
    pub area: F,

    /// Quad: the first edge leading away from `q`.
    pub u: [F; 3],

    /// Which primitive this is, from `light_kind`.
    pub kind: u32,

    /// Quad: the second edge leading away from `q`.
    pub v: [F; 3],

    /// The `kind` of the emitter's material. Stored as two loose words rather
    /// than a `MaterialInfo` so each vector above keeps its scalar companion and
    /// the type lands on 64 bytes with nothing implicit between the fields.
    /// `Light::material` puts them back together.
    pub mat_kind: u32,

    /// Unit normal of the emitting face. A light emits on this side only, so this
    /// also says which half of space can see it.
    pub norm: [F; 3],

    /// The `index` of the emitter's material. See `mat_kind`.
    pub mat_index: u32,
}

// SAFETY: `repr(C)` over plain floats and integers, every slot written down and
// each vector followed by a scalar, so there is no implicit padding and every bit
// pattern is valid. Not derived because the shader builds glam without its
// bytemuck feature. See `Instance` for the same argument.
unsafe impl Zeroable for Light {}
unsafe impl Pod for Light {}

const _: () = assert!(core::mem::size_of::<Light>() == 64);

impl Light {
    /// A quad light over the parallelogram at `q` spanned by `u` and `v`. The
    /// order of the edges decides which way the light faces, exactly as it does
    /// for a quad instance.
    pub fn quad(q: Point3, u: Vec3, v: Vec3, material: MaterialInfo) -> Self {
        let cross = u.cross(v);
        let norm = cross.normalize();

        Self {
            q: [q.x, q.y, q.z],
            area: cross.length(),
            u: [u.x, u.y, u.z],
            kind: light_kind::QUAD,
            v: [v.x, v.y, v.z],
            mat_kind: material.kind,
            norm: [norm.x, norm.y, norm.z],
            mat_index: material.index,
        }
    }

    pub fn material(&self) -> MaterialInfo {
        MaterialInfo {
            kind: self.mat_kind,
            index: self.mat_index,
        }
    }

    pub fn norm(&self) -> Vec3 {
        vec(self.norm)
    }
}

/// A point picked on a light, as seen from somewhere else.
#[derive(Clone, Copy, Default)]
pub struct LightSample {
    /// Unit vector from the shading point towards the sampled point.
    pub dir: Vec3,

    /// How far away the sampled point is, so a shadow ray knows where to stop.
    pub dist: F,

    /// Density of having picked this direction, per unit solid angle, including
    /// the chance of having picked this light out of the array. Zero when the
    /// sample cannot contribute, which the caller must check before dividing.
    pub pdf: F,

    /// Which light was picked, so the caller can read its material.
    pub index: usize,

    /// Surface coordinates of the sampled point.
    pub u: F,
    pub v: F,
}

/// The emitters of a scene.
///
/// The host builds these and uploads them; the shader borrows the uploaded buffer
/// rather than constructing anything of its own, exactly like `Scene`.
///
/// Every density this returns is per unit solid angle and already includes the
/// uniform chance of picking one light out of the array. A caller applying that
/// factor a second time is the classic way this goes wrong, so it lives here and
/// nowhere else.
#[derive(Default)]
pub struct Lights<'a> {
    pub lights: &'a [Light],

    /// How many entries of `lights` are real. See `bounded`.
    count: usize,
}

impl<'a> Lights<'a> {
    pub fn new(lights: &'a [Light]) -> Self {
        Self {
            lights,
            count: lights.len(),
        }
    }

    /// The shader's constructor, where the buffer is longer than the scene.
    ///
    /// An empty binding is not allowed, so a scene with no emitters still uploads
    /// one zeroed light and the buffer's length stops being the answer. Slicing it
    /// down is what a host would do, but rust-gpu cannot lower a range index, so
    /// the bound is carried alongside instead.
    pub fn bounded(lights: &'a [Light], count: usize) -> Self {
        let count = if count < lights.len() {
            count
        } else {
            lights.len()
        };

        Self { lights, count }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Pick a light uniformly and a point on it, as seen from `from`.
    ///
    /// The returned `pdf` is zero when the sample cannot carry light: `from` lies
    /// in the plane of the light, or behind the face that emits. An empty array
    /// returns the same, so a scene with no lights needs no special case.
    pub fn sample(&self, state: &mut RandState, from: Point3) -> LightSample {
        if self.is_empty() {
            return LightSample::default();
        }

        let index = rand::rand_u32(state) as usize % self.count;
        let light = &self.lights[index];

        // Only one kind so far, so this calls the quad directly. A second kind
        // turns this into a dispatch on `light.kind`, which is why the geometry
        // lives in free functions rather than inline
        let mut sample = quad_sample(light, state, from);

        sample.index = index;
        sample.pdf /= self.count as F;

        sample
    }

    /// The density `sample` would have had for a direction that arrived some
    /// other way, so the two strategies can be weighed against each other.
    ///
    /// `from` is where the direction was chosen and `dist` how far along it the
    /// light turned out to be. An `index` that is not a light, which is what a
    /// hit record carries for every ordinary surface, has no density at all.
    pub fn pdf(&self, index: usize, from: Point3, dir: Vec3, dist: F) -> F {
        if index >= self.count {
            return 0.;
        }

        quad_pdf(&self.lights[index], from, dir, dist) / self.count as F
    }
}

fn vec(v: [F; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

/// Uniformly pick a point on the parallelogram and convert the area density
/// `1 / area` into a density per unit solid angle at `from`.
fn quad_sample(light: &Light, state: &mut RandState, from: Point3) -> LightSample {
    let a = rand::rand_f(state);
    let b = rand::rand_f(state);

    let on_light = vec(light.q) + a * vec(light.u) + b * vec(light.v);
    let to_light = on_light - from;

    let dist_squared = to_light.length_squared();
    if dist_squared <= 0. {
        return LightSample::default();
    }

    let dist = dist_squared.sqrt();
    let dir = to_light / dist;

    LightSample {
        dir,
        dist,
        pdf: quad_pdf(light, from, dir, dist),
        index: 0,
        u: a,
        v: b,
    }
}

/// The density `quad_sample` would have produced for `dir`, given that it landed
/// on this light at distance `dist`.
///
/// Zero when the shading point is behind the emitting face or level with it: a
/// grazing angle sends the density to infinity, and neither case can carry light.
///
/// `from` goes unread for a quad, whose density depends only on how the light is
/// tilted and how far away it is. A sphere light would need it, which is why it is
/// asked for.
fn quad_pdf(light: &Light, _from: Point3, dir: Vec3, dist: F) -> F {
    let cos_light = light.norm().dot(-dir);

    if cos_light <= EPSILON_COS || light.area <= 0. {
        return 0.;
    }

    // The area to solid angle Jacobian: a patch of area `area` seen from `dist`
    // away and tilted by `cos_light` subtends `cos_light * area / dist^2`
    dist * dist / (cos_light * light.area)
}
