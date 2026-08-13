// use crate::image::Image;
// use crate::image::Pixel;
use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_mat::Material;
use rtx_mat::MaterialTable;
use rtx_obj::Lights;
use rtx_obj::Scene;
use rtx_prim::Color;
use rtx_prim::F;
use rtx_prim::PI;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::Vec3Ext;
use rtx_prim::rand;
use rtx_tex::TextureTable;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

/// Number of bounces that always survive before russian roulette kicks in. Killing early bounces
/// would add a lot of variance for very little saved work
const RR_MIN_BOUNCE: u32 = 3;

/// Upper bound on the survival probability, so a bright path still terminates eventually
const RR_MAX_SURVIVAL: F = 0.95;

/// How far along a ray a hit has to be to count, so that a ray leaving a surface
/// does not immediately find the surface it left.
const RAY_EPSILON: F = 0.001;

/// The fraction of the way to a light a shadow ray is allowed to travel. The rest
/// is the margin that keeps it from hitting the light it is testing for.
const SHADOW_REACH: F = 1.0 - 1e-4;

/// How much of a contribution to credit to one of two sampling strategies that
/// could both have produced it, given the density each assigns it.
///
/// Squaring is Veach's power heuristic: it commits harder to whichever strategy
/// was more likely to find this particular path than weighting by the densities
/// alone would. Whatever this gives one strategy, it gives the other the rest, so
/// between them they count the path exactly once.
fn power_heuristic(pdf: F, other: F) -> F {
    let (a, b) = (pdf * pdf, other * other);

    if a + b <= 0. { 0. } else { a / (a + b) }
}

#[repr(C)]
pub struct CameraParams {
    /// The position of the camera
    pub lookfrom: Point3,

    /// The point that the camera should be looking at
    pub lookat: Point3,

    /// The view up vector for the camera
    pub vup: Vec3,

    /// The vertical field of view of the camera. The horizontal field of view is computed as a
    /// result
    pub fov_v: F,

    /// Variation angle of rays through each pixel
    pub defocus_angle: F,

    /// Distance from camera `lookfrom` to the plane of perfect focus
    pub focus_dist: F,

    /// For sub-pixel sampling. This is the number of rays to shoot for each pixel. Produces
    /// higher-quality images
    pub px_samples: u32,

    /// The maximum number of times a ray should bounce off a surface before it stops emitting any
    /// light
    pub max_ray_bounce: u32,

    /// The width of the image in pixels
    pub img_width: usize,

    /// The height of the image in pixels
    pub img_height: usize,

    /// Background color of the scene
    pub background: Color,
}

#[repr(C)]
pub struct Camera {
    /// Position of the `Camera`
    pos: Point3,

    /// Width of the output image
    img_width: usize,

    /// Height of the output image
    img_height: usize,

    // Delta between pixels
    px_du: Vec3,
    px_dv: Vec3,

    /// Used for Anti-aliasing
    pub px_samples: u32,
    px_sample_scale: F,

    pub max_ray_bounce: u32,

    /// 3D position of the top left pixel
    px00_loc: Point3,

    defocus_angle: F,

    /// Defocus disk horizontal radius
    defocus_disk_u: Vec3,

    /// Defocus disk vertical radius
    defocus_disk_v: Vec3,

    /// Background color of the scene
    background: Color,
}

impl Camera {
    #[allow(clippy::too_many_arguments)]
    pub fn new(params: CameraParams) -> Self {
        let CameraParams {
            lookfrom,
            lookat,
            vup,
            fov_v,
            defocus_angle,
            focus_dist,
            px_samples,
            max_ray_bounce,
            img_width,
            img_height,
            background,
        } = params;

        let aspect_ratio = img_width as F / img_height as F;

        let px_sample_scale = 1.0 / px_samples as F;

        let theta = fov_v * 2. * PI / 360.; // Convert degrees to radians
        let h = (theta / 2.).tan();
        let viewport_height = 2.0 * h * focus_dist;
        let viewport_width = viewport_height * aspect_ratio;

        let w = (lookfrom - lookat).normalize();
        let u = w.cross(vup).normalize();
        let v = u.cross(w);

        // Calculate the vectors across the horizontal and down the vertical viewport edges.
        let viewport_u = viewport_width * -u;
        let viewport_v = viewport_height * -v;

        // Delta vectors from pixel to pixel
        let px_du = viewport_u / img_width as F;
        let px_dv = viewport_v / img_height as F;

        // This makes the camera appear as though it's at the center of the screen
        let viewport_upper_left =
            lookfrom - (focus_dist * w) - (viewport_u / 2.0) - (viewport_v / 2.0);
        let px00_loc = viewport_upper_left + 0.5 * (px_du + px_dv);

        // Calculate the camera defocus disk basis vectors.
        let defocus_radius = focus_dist * ((defocus_angle / 2.) * 2. * PI / 360.).tan();
        let defocus_disk_u = u * defocus_radius;
        let defocus_disk_v = v * defocus_radius;

        Self {
            pos: lookfrom,
            img_width,
            img_height,
            px_du,
            px_dv,
            px_samples,
            px_sample_scale,
            max_ray_bounce,
            px00_loc,
            defocus_angle,
            defocus_disk_u,
            defocus_disk_v,
            background,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        state: &mut RandState,
        i: usize,
        j: usize,
        mat_table: &MaterialTable<'_>,
        tex_table: &TextureTable<'_>,
        world: &Scene<'_>,
        lights: &Lights<'_>,
    ) -> Color {
        let mut color = Color::new(0., 0., 0.);

        for _ in 0..self.px_samples {
            let ray = self.get_ray(state, i, j);
            color += self.ray_color(state, mat_table, tex_table, ray, world, lights, 0);
        }

        // TODO: One can also use Self::modify_color here but I prefer to not alter it
        self.px_sample_scale * color
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ray_color(
        &self,
        state: &mut RandState,
        mat_table: &MaterialTable<'_>,
        tex_table: &TextureTable<'_>,
        mut ray: Ray,
        world: &Scene<'_>,
        lights: &Lights<'_>,
        mut depth: u32,
    ) -> Color {
        let mut rec: HitRecord = Default::default();

        let mut accumulated = Color::new(0., 0., 0.);
        let mut throughput = Color::new(1., 1., 1.);

        // How the ray currently being traced was chosen, which decides how much of
        // any light it lands on it is entitled to claim. A camera ray was not
        // sampled off a surface at all, so no direct lighting estimate was
        // competing with it and it takes what it finds whole.
        let mut prev_specular = true;
        let mut prev_pdf_bsdf = 0.;

        while depth <= self.max_ray_bounce {
            // Start of range is not zero to avoid floating point errors
            let mut range = Range::new(RAY_EPSILON, F::MAX);

            if !world.hit(&ray, &mut range, &mut rec) {
                // Nothing more to hit, so whatever is at infinity is the last
                // thing this path sees. A path that stops any other way sees
                // nothing more at all, which is why this is the only place the
                // background is added
                accumulated += throughput * self.background;
                break;
            }

            // `rec.t` is in units of the ray's direction, which is not normalized
            let dir = ray.dir();
            let dist = rec.t * dir.length();

            // Light found by bouncing into it. The direct lighting estimate below
            // could have found this same point from the previous surface, so the
            // two strategies split it between them rather than both claiming it.
            // Off a specular bounce there was no such estimate and no split
            let emission = mat_table.emitted(state, tex_table, &rec, rec.u, rec.v, rec.p);
            let weight = if prev_specular {
                1.
            } else {
                let pdf_light =
                    lights.pdf(rec.light_index as usize, ray.orig(), dir.normalize(), dist);

                power_heuristic(prev_pdf_bsdf, pdf_light)
            };
            accumulated += throughput * emission * weight;

            // If material doesn't scatter, we're done (hit a pure light)
            let mut scattered = Default::default();
            let mut attenuation = Default::default();
            if !mat_table.scatter(
                state,
                tex_table,
                &ray,
                &rec,
                &mut scattered,
                &mut attenuation,
            ) {
                break;
            }

            let specular = mat_table.is_specular(&rec);

            // Light found by aiming at it. Pointless off a specular surface,
            // where the one direction that reflects into the viewer is not one
            // this gets to choose
            if !specular {
                accumulated += throughput
                    * Self::direct_light(
                        state,
                        mat_table,
                        tex_table,
                        world,
                        lights,
                        &rec,
                        attenuation,
                        ray.time(),
                    );
            }

            // Update throughput and continue bouncing
            throughput *= attenuation;

            // Russian roulette. Past a few bounces, kill paths with probability `1 - p` and scale
            // the survivors by `1 / p`. The expected contribution is unchanged, so the image stays
            // unbiased, but dim paths stop costing traversals.
            if depth >= RR_MIN_BOUNCE {
                let p = throughput.max_element().min(RR_MAX_SURVIVAL);

                if rand::rand_f(state) >= p {
                    // A killed path contributes nothing more
                    break;
                }

                throughput /= p;
            }

            // What the next iteration needs to know about how it was aimed.
            // Lambertian scatters `norm + rand_unit`, which is already exactly
            // cosine distributed, so its density is recovered here rather than
            // returned from `scatter`
            prev_specular = specular;
            prev_pdf_bsdf = if specular {
                0.
            } else {
                rec.norm.dot(scattered.dir().normalize()).max(0.) / PI
            };

            ray = scattered;
            depth += 1;
        }

        accumulated
    }

    /// What the lights of the scene deliver straight to this surface, before the
    /// path's throughput is applied.
    ///
    /// This is the whole point of the exercise. Left to itself a bounce finds the
    /// Cornell box's ceiling panel about one time in a hundred, and the ninety
    /// nine paths that miss are the noise. Aiming at the panel on purpose and
    /// paying for the privilege in the density finds it every time.
    ///
    /// `albedo` is the attenuation the material returned, which for a Lambertian
    /// is exactly its albedo. That is only true because Lambertian is the one
    /// material that reaches here; a glossy material would need its lobe evaluated
    /// for this direction rather than assumed.
    #[allow(clippy::too_many_arguments)]
    fn direct_light(
        state: &mut RandState,
        mat_table: &MaterialTable<'_>,
        tex_table: &TextureTable<'_>,
        world: &Scene<'_>,
        lights: &Lights<'_>,
        rec: &HitRecord,
        albedo: Color,
        time: F,
    ) -> Color {
        let sample = lights.sample(state, rec.p);

        // No lights, or none that can be seen from this side of one
        if sample.pdf <= 0. {
            return Color::ZERO;
        }

        // The light is below this surface's horizon
        let cos_surf = rec.norm.dot(sample.dir);
        if cos_surf <= 0. {
            return Color::ZERO;
        }

        // Stopping short of the light keeps the shadow ray from finding the
        // surface it is aimed at. Short by a fraction rather than by a fixed
        // amount, because a scene measured in hundreds of units has no use for an
        // epsilon chosen for one measured in ones
        let shadow = Ray::new(rec.p, sample.dir, time);
        let range = Range::new(RAY_EPSILON, sample.dist * SHADOW_REACH);
        if world.occluded(&shadow, &range) {
            return Color::ZERO;
        }

        // The light's own radiance, read through the material table like any
        // other surface. The sampled point is on the face that emits, which is
        // what a positive density above already established
        let mut light_rec = HitRecord {
            mat: lights.lights[sample.index].material(),
            front_face: true,
            ..Default::default()
        };
        light_rec.p = rec.p + sample.dir * sample.dist;

        let emitted = mat_table.emitted(
            state,
            tex_table,
            &light_rec,
            sample.u,
            sample.v,
            light_rec.p,
        );

        // Weighed against the bounce that could have found the same light on its
        // own, so the two estimates of it add up to one estimate
        let pdf_bsdf = cos_surf / PI;
        let weight = power_heuristic(sample.pdf, pdf_bsdf);

        albedo * emitted * (cos_surf / PI * weight / sample.pdf)
    }

    pub fn modify_color(color: Color) -> Color {
        let (x, y, z) = (color.x, color.y, color.z);

        Vec3::new(
            Self::linear_to_gamma(x),
            Self::linear_to_gamma(y),
            Self::linear_to_gamma(z),
        )
    }

    /// Gamma corrected color
    fn linear_to_gamma(linear_channel: F) -> F {
        if linear_channel > 0. {
            linear_channel.sqrt()
        } else {
            0.
        }
    }

    /// Construct a camera ray from the defocus disk and directed at randomly sampled points around
    /// the pixel (i, j).
    pub fn get_ray(&self, state: &mut RandState, i: usize, j: usize) -> Ray {
        let offset = Self::sample_square(state);

        let px_sample =
            self.px00_loc + ((i as F + offset.x) * self.px_dv) + ((j as F + offset.y) * self.px_du);

        let ray_origin = if self.defocus_angle <= 0. {
            self.pos
        } else {
            self.defocus_disk_sample(state)
        };

        let ray_direction = px_sample - ray_origin;
        let ray_time = rand::rand_f(state);

        Ray::new(ray_origin, ray_direction, ray_time)
    }

    fn defocus_disk_sample(&self, state: &mut RandState) -> Point3 {
        let p = Vec3::rand_unit_disk(state);

        self.pos + (p.x * self.defocus_disk_u) + (p.y * self.defocus_disk_v)
    }

    /// Returns a random point centered about (0,0) in the unit square
    fn sample_square(state: &mut RandState) -> Point3 {
        let x: F = rand::rand_f(state);
        let y: F = rand::rand_f(state);

        Point3::new(x - 0.5, y - 0.5, 0.)
    }
}
