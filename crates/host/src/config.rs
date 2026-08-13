//! The configs that say what to do with a scene.
//!
//! A scene says what exists; a config says where it is viewed from and what is
//! produced. An image is a still from a fixed camera, a video is the same thing
//! with a camera that moves, so the two share a shape and are told apart by the
//! `type` at the top of the file.
//!
//! A config names no scene. Which scene it is pointed at is a command line
//! argument, so any camera can be aimed at any scene without editing a file.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use glam::Vec3;
use rtx_bench::CameraPath;
use rtx_bench::CatmullRomSpline;
use serde::Deserialize;

/// The benchmarks `bench` runs when it is given no files of its own. A
/// benchmark is a scene paired with a video config, and the pairing has to be
/// written down somewhere; this is that place.
pub const BENCH_MANIFEST: &str = "bench.toml";

/// Control points a Catmull-Rom spline needs before it describes a curve. The
/// first and last only set the tangents at the ends, so four points is the
/// smallest path that goes anywhere.
const MIN_KEYFRAMES: usize = 4;

/// Rays per pixel for the interactive renderer, which sets its own rather than
/// taking the sample count an image config asks of `render`. That count is
/// chosen to make a good picture, not to keep a window responsive.
pub const PREVIEW_SAMPLES: u32 = 40;

/// Maximum ray bounce depth for the interactive renderer.
pub const PREVIEW_BOUNCES: u32 = 10;

/// Read and parse a TOML file, saying which file was at fault when it fails.
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    toml::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e).into())
}

/// What a config is called in a log line or an output file name: its file name
/// without the extension.
pub fn name_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// One entry of the bench manifest: the two files a benchmark run needs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchEntry {
    pub scene: PathBuf,
    pub config: PathBuf,
}

/// `bench.toml`, the list of benchmarks that a bare `bench` runs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchManifest {
    #[serde(rename = "benchmark")]
    pub benchmarks: Vec<BenchEntry>,
}

impl BenchManifest {
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let manifest: Self = read(path)?;
        if manifest.benchmarks.is_empty() {
            return Err(format!("{} lists no benchmarks", path.display()).into());
        }

        Ok(manifest)
    }
}

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
    /// Load a video config from `path`.
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let config: Self = read(path)?;
        config.validate(&name_of(path))?;

        Ok(config)
    }

    fn validate(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if self.kind != ConfigType::Video {
            return Err(format!("{name} is an image config, not a video").into());
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
    /// Load an image config from `path`.
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let config: Self = read(path)?;
        config.validate(&name_of(path))?;

        Ok(config)
    }

    fn validate(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if self.kind != ConfigType::Image {
            return Err(format!("{name} is a video config, not an image").into());
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
    use crate::scene_data;

    /// Configs are data, so a typo in one is only found by reading it. The paths
    /// are absolute because tests run from the crate directory.
    const IMAGES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/image");
    const VIDEOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/video");

    fn read_named<T: serde::de::DeserializeOwned>(dir: &str, name: &str) -> T {
        let path = PathBuf::from(dir).join(format!("{name}.toml"));
        toml::from_str(&fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    #[test]
    fn every_image_config_parses() {
        let names = scene_data::toml_stems(IMAGES).expect("configs/image is unreadable");
        assert!(!names.is_empty(), "no image configs");

        for name in names {
            let config: ImageConfig = read_named(IMAGES, &name);

            assert_eq!(config.kind, ConfigType::Image, "{name}");
            assert!(config.quality.samples >= 1, "{name} samples");
            assert!(config.quality.bounces >= 1, "{name} bounces");
            assert!(config.output.width >= 1, "{name} width");
            assert!(config.output.height >= 1, "{name} height");
        }
    }

    /// The manifest is the one place a scene and a config are paired, so a path
    /// that has gone stale in it is only found by looking.
    #[test]
    fn the_bench_manifest_points_at_files_that_exist() {
        let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let manifest = BenchManifest::load(&root.join(BENCH_MANIFEST)).unwrap();

        for entry in &manifest.benchmarks {
            let config = root.join(&entry.config);
            let scene = root.join(&entry.scene);

            assert!(scene.is_file(), "{} is not a file", entry.scene.display());
            VideoConfig::load(&config).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    /// Pointing `render` at a video should say so rather than fail on a missing
    /// field.
    #[test]
    fn a_video_is_not_an_image() {
        let source = r#"
            type = "video"
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
    fn every_video_config_parses() {
        let names = scene_data::toml_stems(VIDEOS).expect("configs/video is unreadable");
        assert!(!names.is_empty(), "no video configs");

        for name in names {
            let config: VideoConfig = read_named(VIDEOS, &name);

            assert_eq!(config.kind, ConfigType::Video, "{name}");
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
