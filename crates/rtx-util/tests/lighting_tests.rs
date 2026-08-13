//! Direct lighting, pinned down.
//!
//! `ray_color` splits the light reaching a surface between two estimates: one
//! that aims at the light on purpose and one that finds it by bouncing. Between
//! them they have to count it exactly once. Too much and the room is bright, too
//! little and it is dim, and either way the image still looks like a Cornell box,
//! so the error is only visible against a number computed some other way.
//!
//! The renderer is deterministic given a seed, so these pin the numbers down as
//! well as check them against an independent integral.

use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialInfo;
use rtx_mat::MaterialTable;
use rtx_mat::Metal;
use rtx_obj::Instance;
use rtx_obj::Light;
use rtx_obj::Lights;
use rtx_obj::Scene;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureTable;
use rtx_util::Camera;
use rtx_util::CameraParams;

/// A floor lit by a panel above it, and nothing else.
///
/// Deliberately small: with one surface and one light the answer can be worked
/// out independently, which a Cornell box's interreflections rule out.
struct Fixture {
    instances: Vec<Instance>,
    lights: Vec<Light>,
    lambertians: Vec<Lambertian>,
    metals: Vec<Metal>,
    diffuse_lights: Vec<DiffuseLight>,
    solids: Vec<SolidTexture>,

    /// The floor's albedo and the light's radiance, kept so the expected value
    /// can be worked out from the same numbers the scene was built from.
    albedo: Color,
    emitted: Color,

    /// The light, as the corner and edges it was built from.
    light_q: Point3,
    light_u: Vec3,
    light_v: Vec3,
}

const FLOOR_SIZE: f32 = 40.0;

impl Fixture {
    /// A floor on the XZ plane and a panel `height` above the origin, `size`
    /// square. `specular_floor` swaps the floor's Lambertian for a mirror.
    fn new(height: f32, size: f32, specular_floor: bool) -> Self {
        let albedo = Color::new(0.8, 0.6, 0.4);
        let emitted = Color::new(7.0, 5.0, 3.0);

        let solids = vec![
            SolidTexture::from_color(albedo),
            SolidTexture::from_color(emitted),
        ];
        let lambertians = vec![Lambertian::from_texture(TextureInfo::solid(0))];
        let metals = vec![Metal::new(albedo, 0.0)];
        let diffuse_lights = vec![DiffuseLight::from_texture(TextureInfo::solid(1))];

        let floor_material = if specular_floor {
            MaterialInfo::metal(0)
        } else {
            MaterialInfo::lambertian(0)
        };
        let light_material = MaterialInfo::diffuse_light(0);

        // Floor: centred on the origin, facing up
        let floor = Instance::quad(
            Point3::new(-FLOOR_SIZE / 2., 0., -FLOOR_SIZE / 2.),
            Vec3::new(FLOOR_SIZE, 0., 0.),
            Vec3::new(0., 0., FLOOR_SIZE),
            floor_material,
        );

        // Panel: centred above the origin, facing down. The edge order is what
        // decides that, exactly as in a scene file
        let light_q = Point3::new(-size / 2., height, -size / 2.);
        let light_u = Vec3::new(size, 0., 0.);
        let light_v = Vec3::new(0., 0., size);

        // `u` then `v`, since +X cross +Z is -Y: the panel has to face down at
        // the floor, and the edge order is the only thing that says so
        let mut panel = Instance::quad(light_q, light_u, light_v, light_material);
        panel.light_index = 0;

        Self {
            instances: vec![floor, panel],
            lights: vec![Light::quad(light_q, light_u, light_v, light_material)],
            lambertians,
            metals,
            diffuse_lights,
            solids,
            albedo,
            emitted,
            light_q,
            light_u,
            light_v,
        }
    }

    fn tables(&self) -> (MaterialTable<'_>, TextureTable<'_>, Scene<'_>, Lights<'_>) {
        (
            MaterialTable {
                lambertians: &self.lambertians,
                metals: &self.metals,
                dielectrics: &[],
                diffuse_lights: &self.diffuse_lights,
            },
            TextureTable {
                solids: &self.solids,
            },
            Scene::new(&self.instances),
            Lights::new(&self.lights),
        )
    }

    /// The light leaving a point on the floor, worked out by quadrature over the
    /// panel rather than by sampling it.
    ///
    /// `albedo / PI * integral of Le * cos_surface * cos_light / dist^2 dA`, which
    /// is the rendering equation for one bounce off a Lambertian with nothing else
    /// in the scene. Nothing the renderer computes takes part in this.
    fn direct_light_at(&self, p: Point3) -> Color {
        const STEPS: usize = 400;

        let up = Vec3::new(0., 1., 0.);
        let light_norm = Vec3::new(0., -1., 0.);
        let cell = self.light_u.cross(self.light_v).length() / (STEPS * STEPS) as f32;

        let mut total = Vec3::ZERO;

        for i in 0..STEPS {
            for j in 0..STEPS {
                let a = (i as f32 + 0.5) / STEPS as f32;
                let b = (j as f32 + 0.5) / STEPS as f32;

                let on_light = self.light_q + a * self.light_u + b * self.light_v;
                let to_light = on_light - p;
                let dist_squared = to_light.length_squared();
                let dir = to_light / dist_squared.sqrt();

                let cos_surface = up.dot(dir);
                let cos_light = light_norm.dot(-dir);

                if cos_surface > 0. && cos_light > 0. {
                    total += self.emitted * (cos_surface * cos_light / dist_squared * cell);
                }
            }
        }

        self.albedo * total / core::f32::consts::PI
    }
}

/// A camera is only needed for the settings `ray_color` reads off it, so it looks
/// at the floor from nowhere in particular.
fn camera(max_ray_bounce: u32) -> Camera {
    Camera::new(CameraParams {
        lookfrom: Point3::new(0., 10., 30.),
        lookat: Point3::ZERO,
        vup: Vec3::new(0., 1., 0.),
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 30.0,
        px_samples: 1,
        max_ray_bounce,
        img_width: 100,
        img_height: 100,
        background: Color::ZERO,
    })
}

/// Average `ray_color` over many samples of one ray, which is what a pixel is.
fn trace(fixture: &Fixture, ray: Ray, bounces: u32, samples: u32, seed: RandState) -> Color {
    let (mat_table, tex_table, world, lights) = fixture.tables();
    let cam = camera(bounces);

    let mut state = seed;
    let mut total = Vec3::ZERO;

    for _ in 0..samples {
        total += cam.ray_color(&mut state, &mat_table, &tex_table, ray, &world, &lights, 0);
    }

    total / samples as f32
}

/// Straight down at the origin, where the floor is. Low enough to start under
/// every panel these tests hang above it.
fn down() -> Ray {
    Ray::new(Point3::new(0., 1., 0.), Vec3::new(0., -1., 0.), 0.)
}

/// The headline invariant: the two estimates of the light add up to one estimate.
///
/// One bounce of depth is exactly enough for both to be in play. The aimed
/// estimate fires at the first hit, and the bounced one gets its single chance to
/// land on the panel. Their weighted sum has to be the integral, and if the
/// weights do not partition it will be off by a wide margin rather than a subtle
/// one.
#[test]
fn direct_lighting_matches_the_integral_over_the_light() {
    let fixture = Fixture::new(20.0, 12.0, false);

    let expected = fixture.direct_light_at(Point3::ZERO);
    let actual = trace(&fixture, down(), 1, 200_000, 0x5eed);

    for c in 0..3 {
        let (e, a) = (expected[c], actual[c]);
        assert!(
            (a - e).abs() < e * 0.01,
            "channel {c}: rendered {a}, integral says {e}"
        );
    }
}

/// The same, with the panel close and wide so it covers most of the sky.
///
/// Worth its own case because it swings the balance between the two estimates:
/// a large light is one the bouncing estimate finds easily, so the weights land
/// somewhere quite different than for a small distant one. Getting both right is
/// what says the weights are doing their job rather than one of them being
/// harmlessly near zero.
#[test]
fn direct_lighting_matches_the_integral_for_a_large_close_light() {
    let fixture = Fixture::new(6.0, 30.0, false);

    let expected = fixture.direct_light_at(Point3::ZERO);
    let actual = trace(&fixture, down(), 1, 200_000, 0xabcd);

    for c in 0..3 {
        let (e, a) = (expected[c], actual[c]);
        assert!(
            (a - e).abs() < e * 0.01,
            "channel {c}: rendered {a}, integral says {e}"
        );
    }
}

/// A mirror has no aimed estimate to weigh against, so it must take what it finds
/// whole. Halving it here would be invisible in the Cornell box and obvious in the
/// mirrored one.
///
/// The floor reflects straight back up into the panel, so one bounce returns the
/// panel's radiance times the mirror's albedo, exactly and with no sampling
/// involved.
#[test]
fn a_mirror_takes_the_light_it_reflects_whole() {
    let fixture = Fixture::new(20.0, 12.0, true);

    let actual = trace(&fixture, down(), 1, 16, 0x1234);
    let expected = fixture.emitted * fixture.albedo;

    for c in 0..3 {
        assert!(
            (actual[c] - expected[c]).abs() < expected[c] * 1e-4,
            "channel {c}: mirror returned {}, panel emits {}",
            actual[c],
            expected[c]
        );
    }
}

/// Something in the way means no light gets through. The aimed estimate has to
/// ask, and a shadow ray that stops short of its own target or starts inside the
/// surface it left would quietly answer wrong.
#[test]
fn a_blocker_between_surface_and_light_casts_a_shadow() {
    let mut fixture = Fixture::new(20.0, 12.0, false);

    // A wide slab just under the panel, blocking every path to it
    fixture.instances.push(Instance::quad(
        Point3::new(-15., 15., -15.),
        Vec3::new(30., 0., 0.),
        Vec3::new(0., 0., 30.),
        MaterialInfo::lambertian(0),
    ));

    let actual = trace(&fixture, down(), 1, 20_000, 0x77);

    assert_eq!(
        actual,
        Color::ZERO,
        "light reached a fully shadowed point: {actual:?}"
    );
}

/// A scene with no emitters must come out exactly as it did before any of this
/// existed, which is what keeps the sky lit scenes from drifting.
#[test]
fn a_scene_with_no_lights_is_untouched_by_the_light_sampling() {
    let fixture = Fixture::new(20.0, 12.0, false);

    let (mat_table, tex_table, world, _) = fixture.tables();
    let cam = camera(4);

    // Same rays, once against the scene's lights and once against none at all.
    // The second is the estimator this whole change exists to replace, so it
    // needs a great many samples to say anything at all
    const SAMPLES: u32 = 400_000;

    let with_lights = trace(&fixture, down(), 4, SAMPLES, 0x333);

    let none: [Light; 0] = [];
    let mut state: RandState = 0x333;
    let mut total = Vec3::ZERO;
    for _ in 0..SAMPLES {
        total += cam.ray_color(
            &mut state,
            &mat_table,
            &tex_table,
            down(),
            &world,
            &Lights::new(&none),
            0,
        );
    }
    let without = total / SAMPLES as f32;

    // Not equal: with no lights only the bouncing estimate is left, so this is
    // the same integral reached the slow way. It must agree, and it must not be
    // the identical number, or the light sampling is doing nothing at all
    assert_ne!(with_lights, without);

    for c in 0..3 {
        let (a, b) = (with_lights[c], without[c]);
        assert!(
            (a - b).abs() < a * 0.02,
            "channel {c}: {a} with lights, {b} without"
        );
    }
}
