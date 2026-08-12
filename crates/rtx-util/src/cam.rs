// use crate::image::Image;
// use crate::image::Pixel;
use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_mat::Material;
use rtx_mat::MaterialTable;
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

    pub fn render(
        &self,
        state: &mut RandState,
        i: usize,
        j: usize,
        mat_table: &MaterialTable,
        tex_table: &TextureTable,
        world: &Scene,
    ) -> Color {
        let mut color = Color::new(0., 0., 0.);

        for _ in 0..self.px_samples {
            let ray = self.get_ray(state, i, j);
            color += self.ray_color(state, mat_table, tex_table, ray, world, 0);
        }

        // TODO: One can also use Self::modify_color here but I prefer to not alter it
        self.px_sample_scale * color
    }

    pub fn ray_color(
        &self,
        state: &mut RandState,
        mat_table: &MaterialTable,
        tex_table: &TextureTable,
        mut ray: Ray,
        world: &Scene,
        mut depth: u32,
    ) -> Color {
        // Start of range is not zero to avoid floating point errors
        let mut range = Range::new(0.001, F::MAX);
        let mut rec: HitRecord = Default::default();

        let mut accumulated = Color::new(0., 0., 0.);
        let mut throughput = Color::new(1., 1., 1.);

        while world.hit(&ray, &mut range, &mut rec) {
            if depth > self.max_ray_bounce {
                break;
            }

            // Add emission at this hit point, scaled by current throughput
            let emission = mat_table.emitted(state, tex_table, &rec, rec.u, rec.v, rec.p);
            accumulated += throughput * emission;

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

            // Update throughput and continue bouncing
            throughput *= attenuation;
            ray = scattered;
            depth += 1;
        }

        // Ray escaped or stopped - add background scaled by remaining throughput
        accumulated + throughput * self.background
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
