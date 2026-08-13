//! The configs that say what to do with a scene.
//!
//! A scene says what exists; a config says where it is viewed from and what is
//! produced. An image is a still from a fixed camera, a video is the same thing
//! with a camera that moves, so the two share a shape and are told apart by the
//! `type` at the top of the file.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use glam::Vec3;
use rtx_bench::CameraPath;
use rtx_bench::CatmullRomSpline;
use serde::Deserialize;

use crate::scene_data;

pub const IMAGE_DIR: &str = "configs/image";
pub const VIDEO_DIR: &str = "configs/video";

/// Control points a Catmull-Rom spline needs before it describes a curve. The
/// first and last only set the tangents at the ends, so four points is the
/// smallest path that goes anywhere.
const MIN_KEYFRAMES: usize = 4;

/// Rays per pixel for the interactive and grid renderers, which set their own
/// rather than taking the sample count an image config asks of `render`. That
/// count is chosen to make a good picture, not to keep a window responsive.
pub const PREVIEW_SAMPLES: u32 = 40;

/// Maximum ray bounce depth for the interactive and grid renderers.
pub const PREVIEW_BOUNCES: u32 = 10;

/// The `type` at the top of a config. It is written down rather than inferred
/// from the directory so a file says what it is on its own.
#[derive(Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ConfigType {
    Image,
    Video,
}

/// A still image: one scene, one camera, one file out.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageConfig {
    #[serde(rename = "type")]
    pub kind: ConfigType,
    pub scene: String,
    pub camera: Camera,
    pub quality: Quality,
    pub output: Output,
}

/// Every setting is explicit. The shader has no camera defaults of its own to
/// fall back on: the host supplies all of them on every draw.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub position: [f32; 3],
    pub look_at: [f32; 3],
    pub vup: [f32; 3],
    pub fov: f32,
    pub defocus_angle: f32,
    pub focus_dist: f32,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Quality {
    pub samples: u32,
    pub bounces: u32,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub width: u32,
    pub height: u32,
}

/// A moving camera looking at a scene. `bench` records how long its frames take
/// rather than keeping them; rendering them out is not implemented yet.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoConfig {
    #[serde(rename = "type")]
    pub kind: ConfigType,
    pub scene: String,
    pub camera: VideoCamera,
    pub quality: Quality,
    pub output: VideoOutput,
}

/// The same camera as an image's, except that any of its fields may be a list of
/// keyframes instead of a single value. There is one camera section either way:
/// a still and a fly-through differ in what the fields hold, not in where they
/// are written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoCamera {
    pub position: Animated<[f32; 3]>,
    pub look_at: Animated<[f32; 3]>,
    pub vup: Animated<[f32; 3]>,
    pub fov: Animated<f32>,
    pub defocus_angle: Animated<f32>,
    pub focus_dist: Animated<f32>,
}

/// `frames` sits with the width and height because it is a property of what is
/// produced, not of the camera that produces it.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct VideoOutput {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
}

/// A camera setting over the length of a video: either held for the whole path
/// or interpolated through a list of keyframes.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Animated<T> {
    Fixed(T),
    Keyframes(Vec<T>),
}

impl Animated<[f32; 3]> {
    fn points(&self) -> Vec<Vec3> {
        match self {
            Self::Fixed(v) => vec![Vec3::from(*v); MIN_KEYFRAMES],
            Self::Keyframes(vs) => vs.iter().map(|v| Vec3::from(*v)).collect(),
        }
    }
}

impl Animated<f32> {
    /// A scalar rides the same spline as a position, in the x component. One
    /// interpolation is easier to trust than two that have to agree.
    fn points(&self) -> Vec<Vec3> {
        match self {
            Self::Fixed(v) => vec![Vec3::new(*v, 0.0, 0.0); MIN_KEYFRAMES],
            Self::Keyframes(vs) => vs.iter().map(|v| Vec3::new(*v, 0.0, 0.0)).collect(),
        }
    }
}

impl<T> Animated<T> {
    fn keyframes(&self) -> usize {
        match self {
            Self::Fixed(_) => MIN_KEYFRAMES,
            Self::Keyframes(vs) => vs.len(),
        }
    }
}

/// Every camera setting of a video as a curve, ready to be sampled per frame.
pub struct CameraTracks {
    position: CatmullRomSpline,
    look_at: CatmullRomSpline,
    vup: CatmullRomSpline,
    fov: CatmullRomSpline,
    defocus_angle: CatmullRomSpline,
    focus_dist: CatmullRomSpline,
}

impl CameraTracks {
    /// The camera at a point in the path, with `t` in 0..=1.
    pub fn at(&self, t: f32) -> Camera {
        Camera {
            position: self.position.evaluate(t).into(),
            look_at: self.look_at.evaluate(t).into(),
            vup: self.vup.evaluate(t).into(),
            fov: self.fov.evaluate(t).x,
            defocus_angle: self.defocus_angle.evaluate(t).x,
            focus_dist: self.focus_dist.evaluate(t).x,
        }
    }
}

impl VideoCamera {
    pub fn tracks(&self) -> CameraTracks {
        CameraTracks {
            position: CatmullRomSpline::new(self.position.points()),
            look_at: CatmullRomSpline::new(self.look_at.points()),
            vup: CatmullRomSpline::new(self.vup.points()),
            fov: CatmullRomSpline::new(self.fov.points()),
            defocus_angle: CatmullRomSpline::new(self.defocus_angle.points()),
            focus_dist: CatmullRomSpline::new(self.focus_dist.points()),
        }
    }

    /// The position and look-at path, which is what a benchmark result records
    /// of the camera.
    pub fn path(&self, frames: u32) -> CameraPath {
        CameraPath::new(self.position.points(), self.look_at.points(), frames)
    }

    /// Every field with the number of keyframes it was given, for validation.
    fn tracks_with_names(&self) -> [(&'static str, usize); 6] {
        [
            ("position", self.position.keyframes()),
            ("look_at", self.look_at.keyframes()),
            ("vup", self.vup.keyframes()),
            ("fov", self.fov.keyframes()),
            ("defocus_angle", self.defocus_angle.keyframes()),
            ("focus_dist", self.focus_dist.keyframes()),
        ]
    }
}

impl VideoConfig {
    /// Load a video config from `configs/video/<name>.toml`.
    pub fn load(name: &str) -> Result<Self, Box<dyn Error>> {
        let path = PathBuf::from(VIDEO_DIR).join(format!("{name}.toml"));
        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let config: Self = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        config.validate(name)?;

        Ok(config)
    }

    /// Every video config, in name order.
    pub fn load_all() -> Result<Vec<(String, Self)>, Box<dyn Error>> {
        let names = scene_data::toml_stems(VIDEO_DIR)?;
        if names.is_empty() {
            return Err(format!("No configs found in {VIDEO_DIR}/").into());
        }

        names
            .into_iter()
            .map(|name| Self::load(&name).map(|config| (name, config)))
            .collect()
    }

    fn validate(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if self.kind != ConfigType::Video {
            return Err(format!("{name} is an image config, not a video").into());
        }
        if !scene_data::names()?.iter().any(|s| s == &self.scene) {
            return Err(format!(
                "Video {} uses unknown scene {}. Available scenes: {}",
                name,
                self.scene,
                scene_data::names_or_empty()
            )
            .into());
        }
        if self.output.frames < 1 {
            return Err(format!("Video {name} needs at least 1 frame").into());
        }
        if self.quality.samples < 1 {
            return Err(format!("Video {name} needs at least 1 sample").into());
        }
        if self.quality.bounces < 1 {
            return Err(format!("Video {name} needs at least 1 bounce").into());
        }
        if self.output.width < 1 || self.output.height < 1 {
            return Err(format!("Video {name} needs a non-zero output size").into());
        }

        self.check_camera(name)
    }

    /// Every animated field has to describe a curve. A single value does so by
    /// standing still, but a list has to be long enough for the spline.
    fn check_camera(&self, name: &str) -> Result<(), Box<dyn Error>> {
        for (field, keyframes) in self.camera.tracks_with_names() {
            if keyframes < MIN_KEYFRAMES {
                return Err(format!(
                    "Video {name} gives camera.{field} {keyframes} keyframes, \
                     but a path needs at least {MIN_KEYFRAMES}"
                )
                .into());
            }
        }

        Ok(())
    }
}

impl Camera {
    /// Direction from the camera position to its target.
    pub fn direction(&self) -> [f32; 3] {
        let (p, t) = (self.position, self.look_at);

        [t[0] - p[0], t[1] - p[1], t[2] - p[2]]
    }

    /// Push constants for this camera. The caller owns the resolution and the
    /// quality because those differ between a window, a grid tile and a render
    /// pass looking at the same thing.
    pub fn constants(
        &self,
        width: u32,
        height: u32,
        quality: Quality,
        background: [f32; 3],
    ) -> shared::ShaderConstants {
        shared::ShaderConstants {
            width,
            height,
            time: 0.0,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cam_pos: self.position,
            cam_dir: self.direction(),
            cam_vup: self.vup,
            fov_v: self.fov,
            defocus_angle: self.defocus_angle,
            focus_dist: self.focus_dist,
            px_samples: quality.samples,
            max_ray_bounce: quality.bounces,
            seed: 0,
            background,
        }
    }
}

impl ImageConfig {
    /// Load an image config from `configs/image/<name>.toml`.
    pub fn load(name: &str) -> Result<Self, Box<dyn Error>> {
        let path = PathBuf::from(IMAGE_DIR).join(format!("{name}.toml"));
        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let config: Self = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        config.validate(name)?;

        Ok(config)
    }

    /// Every image config, in name order.
    pub fn load_all() -> Result<Vec<(String, Self)>, Box<dyn Error>> {
        let names = scene_data::toml_stems(IMAGE_DIR)?;
        if names.is_empty() {
            return Err(format!("No configs found in {IMAGE_DIR}/").into());
        }

        names
            .into_iter()
            .map(|name| Self::load(&name).map(|config| (name, config)))
            .collect()
    }

    fn validate(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if self.kind != ConfigType::Image {
            return Err(format!("{name} is a video config, not an image").into());
        }
        if !scene_data::names()?.iter().any(|s| s == &self.scene) {
            return Err(format!(
                "Image {} uses unknown scene {}. Available scenes: {}",
                name,
                self.scene,
                scene_data::names_or_empty()
            )
            .into());
        }
        if self.quality.samples < 1 {
            return Err(format!("Image {name} needs at least 1 sample").into());
        }
        if self.quality.bounces < 1 {
            return Err(format!("Image {name} needs at least 1 bounce").into());
        }
        if self.output.width < 1 || self.output.height < 1 {
            return Err(format!("Image {name} needs a non-zero output size").into());
        }

        Ok(())
    }

    /// The sample count and depth an interactive view uses, rather than the ones
    /// the config asks of `render`.
    pub fn preview_quality() -> Quality {
        Quality {
            samples: PREVIEW_SAMPLES,
            bounces: PREVIEW_BOUNCES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Configs are data, so a typo in one is only found by reading it. The paths
    /// are absolute because tests run from the crate directory.
    const IMAGES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/image");
    const VIDEOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/video");
    const SCENES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenes");

    fn read<T: serde::de::DeserializeOwned>(dir: &str, name: &str) -> T {
        let path = PathBuf::from(dir).join(format!("{name}.toml"));
        toml::from_str(&fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    #[test]
    fn every_image_config_parses_and_names_a_scene() {
        let scenes = scene_data::toml_stems(SCENES).unwrap();
        let names = scene_data::toml_stems(IMAGES).expect("configs/image is unreadable");
        assert!(!names.is_empty(), "no image configs");

        for name in names {
            let config: ImageConfig = read(IMAGES, &name);

            assert_eq!(config.kind, ConfigType::Image, "{name}");
            assert!(
                scenes.contains(&config.scene),
                "{name} uses unknown scene {}",
                config.scene
            );
            assert!(config.quality.samples >= 1, "{name} samples");
            assert!(config.quality.bounces >= 1, "{name} bounces");
            assert!(config.output.width >= 1, "{name} width");
            assert!(config.output.height >= 1, "{name} height");
        }
    }

    /// Pointing `render` at a video should say so rather than fail on a missing
    /// field.
    #[test]
    fn a_video_is_not_an_image() {
        let source = r#"
            type = "video"
            scene = "two_spheres"
            [camera]
            position = [0.0, 1.0, 5.0]
            look_at = [0.0, 0.0, 0.0]
            vup = [0.0, 1.0, 0.0]
            fov = 40.0
            defocus_angle = 0.0
            focus_dist = 5.0
            [quality]
            samples = 8
            bounces = 10
            [output]
            width = 400
            height = 300
        "#;

        let config: ImageConfig = toml::from_str(source).unwrap();
        assert_eq!(config.kind, ConfigType::Video);
    }

    #[test]
    fn every_video_config_parses_and_names_a_scene() {
        let scenes = scene_data::toml_stems(SCENES).unwrap();
        let names = scene_data::toml_stems(VIDEOS).expect("configs/video is unreadable");
        assert!(!names.is_empty(), "no video configs");

        for name in names {
            let config: VideoConfig = read(VIDEOS, &name);

            assert_eq!(config.kind, ConfigType::Video, "{name}");
            assert!(
                scenes.contains(&config.scene),
                "{name} uses unknown scene {}",
                config.scene
            );
            assert!(config.output.frames >= 1, "{name} frames");

            for (field, keyframes) in config.camera.tracks_with_names() {
                assert!(
                    keyframes >= MIN_KEYFRAMES,
                    "{name} camera.{field} has {keyframes} keyframes"
                );
            }
        }
    }

    fn camera(look_at: &str) -> VideoCamera {
        let source = format!(
            r#"
            position = [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
            ]
            look_at = {look_at}
            vup = [0.0, 1.0, 0.0]
            fov = 40.0
            defocus_angle = 0.0
            focus_dist = 5.0
            "#
        );

        toml::from_str(&source).unwrap()
    }

    /// A field given one value is held for the whole path, which is the only
    /// thing that lets a fixed look-at share a section with a moving position.
    #[test]
    fn a_fixed_field_does_not_move() {
        let tracks = camera("[7.0, 8.0, 9.0]").tracks();

        for t in [0.0, 0.25, 0.5, 1.0] {
            assert_eq!(tracks.at(t).look_at, [7.0, 8.0, 9.0], "at t={t}");
            assert_eq!(tracks.at(t).fov, 40.0, "at t={t}");
        }
    }

    /// The path runs through the middle keyframes; the outer two only set the
    /// tangents at the ends.
    #[test]
    fn keyframes_are_interpolated_across_the_path() {
        let tracks =
            camera("[[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0], [0.0, 3.0, 0.0]]").tracks();

        assert_eq!(tracks.at(0.0).look_at[1], 1.0);
        assert_eq!(tracks.at(1.0).look_at[1], 2.0);
        assert!((tracks.at(0.5).look_at[1] - 1.5).abs() < 1e-5);
    }

    #[test]
    fn a_short_keyframe_list_is_rejected() {
        let source = r#"
            type = "video"
            scene = "two_spheres"
            [camera]
            position = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]
            look_at = [0.0, 0.0, 0.0]
            vup = [0.0, 1.0, 0.0]
            fov = 40.0
            defocus_angle = 0.0
            focus_dist = 5.0
            [quality]
            samples = 8
            bounces = 10
            [output]
            width = 400
            height = 300
            frames = 60
        "#;

        let config: VideoConfig = toml::from_str(source).unwrap();
        let error = config.check_camera("short").unwrap_err().to_string();

        assert!(error.contains("position"), "{error}");
        assert!(error.contains("3 keyframes"), "{error}");
    }
}
