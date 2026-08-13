//! Scenes, as read from `scenes/<name>.toml` and as the GPU reads them.
//!
//! Scenes used to be built inside the shader and rebuilt by every pixel before it
//! traced a single ray. They are now parsed on the host and uploaded once.
//!
//! A scene says what exists and nothing about where it is viewed from: cameras
//! live in the image and video configs, so one scene can back a still, a
//! benchmark fly-through and a video without being copied.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use rtx_mat::Dielectric;
use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialInfo;
use rtx_mat::Metal;
use rtx_obj::Instance;
use rtx_obj::make_box;
use rtx_prim::Color;
use rtx_prim::Mat4;
use rtx_prim::Point3;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureInfo;
use serde::Deserialize;

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

    fn dielectric(&mut self, ior: f32) -> MaterialInfo {
        self.dielectrics.push(Dielectric::new(ior));

        MaterialInfo::dielectric((self.dielectrics.len() - 1) as u32)
    }

    fn diffuse_light(&mut self, color: Color) -> MaterialInfo {
        let tex = self.solid(color);
        self.diffuse_lights
            .push(DiffuseLight::from_texture(TextureInfo::solid(tex)));

        MaterialInfo::diffuse_light((self.diffuse_lights.len() - 1) as u32)
    }
}

/// A scene file, as written.
///
/// Materials are a table keyed by name so an object can refer to one without
/// repeating it, and so a surface shared by several objects stays a single
/// definition. Objects are an array because their order is their identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    /// What a ray that escapes the scene sees.
    background: [f32; 3],
    materials: BTreeMap<String, MaterialDef>,
    objects: Vec<ObjectDef>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum MaterialDef {
    Lambertian { color: [f32; 3] },
    Metal { color: [f32; 3], fuzz: f32 },
    Dielectric { ior: f32 },
    DiffuseLight { color: [f32; 3] },
}

/// The `name` on an object is documentation. Nothing looks it up, but a wall is
/// easier to find in a file of twelve quads when it says which wall it is.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ObjectDef {
    Sphere {
        #[serde(default)]
        name: Option<String>,
        material: String,
        center: [f32; 3],
        radius: f32,
    },
    Quad {
        #[serde(default)]
        name: Option<String>,
        material: String,
        corner: [f32; 3],
        u: [f32; 3],
        v: [f32; 3],
    },
    Box {
        #[serde(default)]
        name: Option<String>,
        material: String,
        min: [f32; 3],
        max: [f32; 3],
        /// Degrees about Y, applied about the origin before `translate`.
        #[serde(default)]
        rotate_y: f32,
        #[serde(default)]
        translate: [f32; 3],
    },
}

impl ObjectDef {
    fn material(&self) -> &str {
        match self {
            Self::Sphere { material, .. }
            | Self::Quad { material, .. }
            | Self::Box { material, .. } => material,
        }
    }

    /// How the object is described in an error, so a scene with a dozen quads
    /// says which one is wrong.
    fn label(&self, index: usize) -> String {
        let (kind, name) = match self {
            Self::Sphere { name, .. } => ("sphere", name),
            Self::Quad { name, .. } => ("quad", name),
            Self::Box { name, .. } => ("box", name),
        };

        match name {
            Some(name) => format!("{kind} \"{name}\""),
            None => format!("{kind} {index}"),
        }
    }
}

fn point(p: [f32; 3]) -> Point3 {
    Point3::new(p[0], p[1], p[2])
}

fn vector(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

fn color(c: [f32; 3]) -> Color {
    Color::new(c[0], c[1], c[2])
}

/// Read and build the scene written in `path`.
pub fn load(path: &Path) -> Result<SceneData, Box<dyn Error>> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let file: SceneFile = toml::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

    build(&file).map_err(|e| format!("Scene {}: {}", path.display(), e).into())
}

/// The scene's name, which is its file name without the extension. Nothing
/// resolves a scene by it; it is what a log line and a benchmark record call the
/// thing being rendered.
pub fn name_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// The names of the TOML files in a directory, sorted, without their extension.
/// Only the tests walk a directory of them: nothing in the program looks a scene
/// up by name any more.
#[cfg(test)]
pub fn toml_stems(dir: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut stems: Vec<String> = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read {dir}/: {e}"))?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()? != "toml" {
                return None;
            }
            path.file_stem()?.to_str().map(str::to_string)
        })
        .collect();

    stems.sort();

    Ok(stems)
}

/// Turn a parsed scene file into the buffers the GPU reads.
///
/// Materials are built in name order and before any object refers to them, so an
/// unused material still costs an entry. That keeps the material indices a
/// property of the file rather than of the order objects happen to be listed in.
fn build(file: &SceneFile) -> Result<SceneData, Box<dyn Error>> {
    let mut scene = SceneData {
        background: file.background,
        ..Default::default()
    };

    let mut materials: BTreeMap<&str, MaterialInfo> = BTreeMap::new();
    for (name, def) in &file.materials {
        let info = match *def {
            MaterialDef::Lambertian { color: c } => scene.lambertian(color(c)),
            MaterialDef::Metal { color: c, fuzz } => scene.metal(color(c), fuzz),
            MaterialDef::Dielectric { ior } => scene.dielectric(ior),
            MaterialDef::DiffuseLight { color: c } => scene.diffuse_light(color(c)),
        };
        materials.insert(name.as_str(), info);
    }

    for (index, object) in file.objects.iter().enumerate() {
        let material = *materials.get(object.material()).ok_or_else(|| {
            format!(
                "{} uses unknown material \"{}\". Defined materials: {}",
                object.label(index),
                object.material(),
                materials.keys().copied().collect::<Vec<_>>().join(", ")
            )
        })?;

        match *object {
            ObjectDef::Sphere { center, radius, .. } => {
                if radius <= 0.0 {
                    return Err(format!("{} needs a positive radius", object.label(index)).into());
                }
                scene
                    .instances
                    .push(Instance::sphere(point(center), radius, material));
            }
            ObjectDef::Quad { corner, u, v, .. } => {
                let (u, v) = (vector(u), vector(v));
                if u.cross(v).length_squared() == 0.0 {
                    return Err(format!(
                        "{} has parallel edges, so it has no surface",
                        object.label(index)
                    )
                    .into());
                }
                scene
                    .instances
                    .push(Instance::quad(point(corner), u, v, material));
            }
            ObjectDef::Box {
                min,
                max,
                rotate_y,
                translate,
                ..
            } => {
                let (min, max) = (point(min), point(max));
                if (max - min).min_element() <= 0.0 {
                    return Err(format!(
                        "{} needs a max corner greater than its min on every axis",
                        object.label(index)
                    )
                    .into());
                }
                let transform = Mat4::from_translation(vector(translate))
                    * Mat4::from_rotation_y(rotate_y.to_radians());
                scene
                    .instances
                    .extend_from_slice(&make_box(min, max, transform, material));
            }
        }
    }

    if scene.instances.is_empty() {
        return Err("has no objects".into());
    }

    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests run from the crate directory rather than the workspace root, so the
    /// scene directory has to be found rather than assumed.
    const SCENES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenes");

    fn scene_names() -> Vec<String> {
        toml_stems(SCENES).expect("scenes/ is unreadable")
    }

    fn load_named(name: &str) -> Result<SceneData, Box<dyn Error>> {
        load(&PathBuf::from(SCENES).join(format!("{name}.toml")))
    }

    /// The scene files are data, so a typo in one is only found by reading it.
    #[test]
    fn every_scene_file_loads() {
        for name in scene_names() {
            let scene = load_named(&name).unwrap_or_else(|e| panic!("{e}"));
            assert!(!scene.instances.is_empty(), "{name} built no instances");
        }
    }

    /// Instances index into the material arrays, so a mismatch is a GPU read out
    /// of bounds rather than a compile error.
    #[test]
    fn material_references_are_in_range() {
        use rtx_mat::material_kind;

        for name in scene_names() {
            let scene = load_named(&name).unwrap();

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

    fn parse(source: &str) -> Result<SceneData, Box<dyn Error>> {
        build(&toml::from_str(source)?)
    }

    const ONE_SPHERE: &str = r#"
        background = [0.0, 0.0, 0.0]
        [materials.red]
        type = "lambertian"
        color = [1.0, 0.0, 0.0]
        [[objects]]
        type = "sphere"
        material = "red"
        center = [0.0, 0.0, 0.0]
        radius = 1.0
    "#;

    #[test]
    fn a_box_becomes_six_quads() {
        let source = r#"
            background = [0.0, 0.0, 0.0]
            [materials.white]
            type = "lambertian"
            color = [1.0, 1.0, 1.0]
            [[objects]]
            type = "box"
            material = "white"
            min = [0.0, 0.0, 0.0]
            max = [1.0, 1.0, 1.0]
        "#;

        assert_eq!(parse(source).unwrap().instances.len(), 6);
    }

    #[test]
    fn a_missing_material_names_the_object() {
        let source = ONE_SPHERE.replace("material = \"red\"", "material = \"blue\"");
        let error = match parse(&source) {
            Ok(_) => panic!("an object with no material was accepted"),
            Err(e) => e.to_string(),
        };

        assert!(error.contains("blue"), "{error}");
        assert!(error.contains("red"), "{error}");
    }

    #[test]
    fn a_degenerate_quad_is_rejected() {
        let source = r#"
            background = [0.0, 0.0, 0.0]
            [materials.white]
            type = "lambertian"
            color = [1.0, 1.0, 1.0]
            [[objects]]
            type = "quad"
            material = "white"
            corner = [0.0, 0.0, 0.0]
            u = [1.0, 0.0, 0.0]
            v = [2.0, 0.0, 0.0]
        "#;

        assert!(parse(source).is_err());
    }

    #[test]
    fn an_unknown_field_is_rejected() {
        let source = ONE_SPHERE.replace("radius = 1.0", "radius = 1.0\nradius2 = 2.0");

        assert!(parse(&source).is_err());
    }
}
