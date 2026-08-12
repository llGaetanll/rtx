//! Scene definitions, in the form the GPU reads them.
//!
//! These used to live in the shader and were rebuilt by every pixel before it
//! traced a single ray. The host now builds each scene once and uploads it.

use rtx_mat::Dielectric;
use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialInfo;
use rtx_mat::Metal;
use rtx_obj::Instance;
use rtx_obj::make_box;
use rtx_prim::Color;
use rtx_prim::Mat4;
use rtx_prim::PI;
use rtx_prim::Point3;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureInfo;

/// Everything about a scene that the shader reads from a buffer.
#[derive(Default)]
pub struct SceneData {
    pub background: [f32; 3],
    pub instances: Vec<Instance>,
    pub lambertians: Vec<Lambertian>,
    pub metals: Vec<Metal>,
    pub dielectrics: Vec<Dielectric>,
    pub diffuse_lights: Vec<DiffuseLight>,
    pub solids: Vec<SolidTexture>,
}

impl SceneData {
    /// Add a solid color texture and return its index.
    fn solid(&mut self, color: Color) -> u32 {
        self.solids.push(SolidTexture::from_color(color));
        (self.solids.len() - 1) as u32
    }

    /// Add a lambertian material over a solid color and return a reference to it.
    fn lambertian(&mut self, color: Color) -> MaterialInfo {
        let tex = self.solid(color);
        self.lambertians
            .push(Lambertian::from_texture(TextureInfo::solid(tex)));

        MaterialInfo::lambertian((self.lambertians.len() - 1) as u32)
    }

    fn metal(&mut self, albedo: Color, fuzz: f32) -> MaterialInfo {
        self.metals.push(Metal::new(albedo, fuzz));

        MaterialInfo::metal((self.metals.len() - 1) as u32)
    }

    fn dielectric(&mut self, r: f32) -> MaterialInfo {
        self.dielectrics.push(Dielectric::new(r));

        MaterialInfo::dielectric((self.dielectrics.len() - 1) as u32)
    }

    fn diffuse_light(&mut self, color: Color) -> MaterialInfo {
        let tex = self.solid(color);
        self.diffuse_lights
            .push(DiffuseLight::from_texture(TextureInfo::solid(tex)));

        MaterialInfo::diffuse_light((self.diffuse_lights.len() - 1) as u32)
    }
}

const SKY: [f32; 3] = [0.7, 0.8, 1.0];
const DARK: [f32; 3] = [0.0, 0.0, 0.0];

/// Build a scene by name. Names match the entries in `scenes::SCENES`.
pub fn build(name: &str) -> Option<SceneData> {
    let scene = match name {
        "cornell_box" => cornell_box(),
        "quads" => quads(),
        "metal_test" => metal_test(),
        "dielectric_test" => dielectric_test(),
        "two_spheres" => two_spheres(),
        "glass_debug" => glass_debug(),
        "three_spheres" => three_spheres(),
        "many_spheres" => many_spheres(),
        _ => return None,
    };

    Some(scene)
}

fn cornell_box() -> SceneData {
    let mut s = SceneData {
        background: DARK,
        ..Default::default()
    };

    let red = s.lambertian(Color::new(0.65, 0.05, 0.05));
    let green = s.lambertian(Color::new(0.12, 0.45, 0.15));
    let white = s.lambertian(Color::new(0.73, 0.73, 0.73));
    let light = s.diffuse_light(Color::new(15., 15., 15.));

    // Left wall (green)
    s.instances.push(Instance::quad(
        Point3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        green,
    ));
    // Right wall (red)
    s.instances.push(Instance::quad(
        Point3::new(0., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        red,
    ));
    // Floor
    s.instances.push(Instance::quad(
        Point3::new(0., 0., 0.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 0., 555.),
        white,
    ));
    // Ceiling
    s.instances.push(Instance::quad(
        Point3::new(555., 555., 555.),
        Vec3::new(-555., 0., 0.),
        Vec3::new(0., 0., -555.),
        white,
    ));
    // Back wall
    s.instances.push(Instance::quad(
        Point3::new(0., 0., 555.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        white,
    ));
    // Light
    s.instances.push(Instance::quad(
        Point3::new(343., 554., 332.),
        Vec3::new(-130., 0., 0.),
        Vec3::new(0., 0., -105.),
        light,
    ));

    // Tall box, rotated 15 degrees about Y
    let tall = make_box(
        Point3::ZERO,
        Point3::new(165., 330., 165.),
        Mat4::from_translation(Vec3::new(265., 0., 295.)) * Mat4::from_rotation_y(15. * PI / 180.),
        white,
    );
    s.instances.extend_from_slice(&tall);

    // Short box, rotated -18 degrees about Y
    let short = make_box(
        Point3::ZERO,
        Point3::new(165., 165., 165.),
        Mat4::from_translation(Vec3::new(130., 0., 65.)) * Mat4::from_rotation_y(-18. * PI / 180.),
        white,
    );
    s.instances.extend_from_slice(&short);

    s
}

fn quads() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let red = s.lambertian(Color::new(1.0, 0.2, 0.2));
    let green = s.lambertian(Color::new(0.2, 1.0, 0.2));
    let blue = s.lambertian(Color::new(0.2, 0.2, 1.0));
    let orange = s.lambertian(Color::new(1.0, 0.5, 0.0));
    let teal = s.lambertian(Color::new(0.2, 0.8, 0.8));

    // Left wall (red)
    s.instances.push(Instance::quad(
        Point3::new(-3., -2., 5.),
        Vec3::new(0., 0., -4.),
        Vec3::new(0., 4., 0.),
        red,
    ));
    // Back wall (green)
    s.instances.push(Instance::quad(
        Point3::new(-2., -2., 0.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 4., 0.),
        green,
    ));
    // Right wall (blue)
    s.instances.push(Instance::quad(
        Point3::new(3., -2., 1.),
        Vec3::new(0., 0., 4.),
        Vec3::new(0., 4., 0.),
        blue,
    ));
    // Ceiling (orange)
    s.instances.push(Instance::quad(
        Point3::new(-2., 3., 1.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 0., 4.),
        orange,
    ));
    // Floor (teal)
    s.instances.push(Instance::quad(
        Point3::new(-2., -3., 5.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 0., -4.),
        teal,
    ));

    s
}

fn metal_test() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let ground = s.lambertian(Color::new(0.5, 0.5, 0.5));
    let red = s.lambertian(Color::new(0.8, 0.3, 0.3));
    let metal = s.metal(Color::new(0.8, 0.8, 0.8), 0.1);

    s.instances
        .push(Instance::sphere(Point3::new(0., -100.5, 0.), 100., ground));
    s.instances
        .push(Instance::sphere(Point3::new(-1., 0., 0.), 0.5, red));
    s.instances
        .push(Instance::sphere(Point3::new(1., 0., 0.), 0.5, metal));

    s
}

fn dielectric_test() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let ground = s.lambertian(Color::new(0.5, 0.5, 0.5));
    let red = s.lambertian(Color::new(0.8, 0.3, 0.3));
    let glass = s.dielectric(1.5);

    s.instances
        .push(Instance::sphere(Point3::new(0., -100.5, 0.), 100., ground));
    s.instances
        .push(Instance::sphere(Point3::new(-1., 0., 0.), 0.5, red));
    s.instances
        .push(Instance::sphere(Point3::new(1., 0., 0.), 0.5, glass));

    s
}

fn two_spheres() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let ground = s.lambertian(Color::new(0.5, 0.5, 0.5));
    let red = s.lambertian(Color::new(0.8, 0.3, 0.3));
    let blue = s.lambertian(Color::new(0.3, 0.3, 0.8));

    s.instances
        .push(Instance::sphere(Point3::new(0., -100.5, 0.), 100., ground));
    s.instances
        .push(Instance::sphere(Point3::new(-1., 0., 0.), 0.5, red));
    s.instances
        .push(Instance::sphere(Point3::new(1., 0., 0.), 0.5, blue));

    s
}

fn glass_debug() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let blue = s.lambertian(Color::new(0.3, 0.3, 0.8));
    let glass = s.dielectric(1.5);

    // Large blue floor sphere
    s.instances
        .push(Instance::sphere(Point3::new(0., -100., 0.), 100., blue));
    // Glass sphere on top
    s.instances
        .push(Instance::sphere(Point3::new(0., 1., 0.), 1.0, glass));

    s
}

fn three_spheres() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let ground = s.lambertian(Color::new(0.5, 0.5, 0.5));
    let brown = s.lambertian(Color::new(0.4, 0.2, 0.1));
    let metal = s.metal(Color::new(0.7, 0.6, 0.5), 0.0);
    let glass = s.dielectric(1.5);

    s.instances
        .push(Instance::sphere(Point3::new(0., -1000., 0.), 1000., ground));
    s.instances
        .push(Instance::sphere(Point3::new(0., 1., 0.), 1.0, glass));
    s.instances
        .push(Instance::sphere(Point3::new(-4., 1., 0.), 1.0, brown));
    s.instances
        .push(Instance::sphere(Point3::new(4., 1., 0.), 1.0, metal));

    s
}

fn many_spheres() -> SceneData {
    let mut s = SceneData {
        background: SKY,
        ..Default::default()
    };

    let ground = s.lambertian(Color::new(0.5, 0.5, 0.5));

    // Small lambertian spheres
    let small_colors = [Color::new(0.73, 0.24, 0.14), Color::new(0.22, 0.55, 0.34)];
    let lamb1 = s.lambertian(small_colors[0]);
    let lamb2 = s.lambertian(small_colors[1]);

    let brown = s.lambertian(Color::new(0.4, 0.2, 0.1));

    let metal0 = s.metal(Color::new(0.85, 0.75, 0.65), 0.1);
    let metal1 = s.metal(Color::new(0.72, 0.72, 0.78), 0.0);
    let metal_main = s.metal(Color::new(0.7, 0.6, 0.5), 0.0);

    let glass = s.dielectric(1.5);
    let glass1 = s.dielectric(1.45);

    // Ground
    s.instances
        .push(Instance::sphere(Point3::new(0., -1000., 0.), 1000., ground));

    // Three main spheres
    s.instances
        .push(Instance::sphere(Point3::new(0., 1., 0.), 1.0, glass));
    s.instances
        .push(Instance::sphere(Point3::new(-4., 1., 0.), 1.0, brown));
    s.instances
        .push(Instance::sphere(Point3::new(4., 1., 0.), 1.0, metal_main));

    // Small spheres
    s.instances
        .push(Instance::sphere(Point3::new(-3.5, 0.2, 1.5), 0.2, lamb1));
    s.instances
        .push(Instance::sphere(Point3::new(-2.2, 0.2, 2.8), 0.2, lamb2));
    s.instances
        .push(Instance::sphere(Point3::new(0.8, 0.2, -1.8), 0.2, metal0));
    s.instances
        .push(Instance::sphere(Point3::new(2.5, 0.2, -0.5), 0.2, metal1));
    s.instances
        .push(Instance::sphere(Point3::new(-0.2, 0.2, 0.5), 0.2, glass1));

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenes;

    /// Every scene the rest of the host offers must actually be buildable.
    #[test]
    fn every_named_scene_builds() {
        for name in scenes::names() {
            let scene = build(name).unwrap_or_else(|| panic!("{name} has no scene data"));
            assert!(!scene.instances.is_empty(), "{name} built no instances");
        }
    }

    /// Instances index into the material arrays, so a mismatch is a GPU read out
    /// of bounds rather than a compile error.
    #[test]
    fn material_references_are_in_range() {
        use rtx_mat::material_kind;

        for name in scenes::names() {
            let scene = build(name).unwrap();

            for instance in &scene.instances {
                let index = instance.material.index as usize;
                let len = match instance.material.kind {
                    material_kind::LAMBERTIAN => scene.lambertians.len(),
                    material_kind::METAL => scene.metals.len(),
                    material_kind::DIELECTRIC => scene.dielectrics.len(),
                    material_kind::DIFFUSE_LIGHT => scene.diffuse_lights.len(),
                    other => panic!("{name} uses unknown material kind {other}"),
                };

                assert!(index < len, "{name} references material {index} of {len}");
            }
        }
    }
}
