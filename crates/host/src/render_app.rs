use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use chrono::Utc;
use futures::executor::block_on;
use serde::Deserialize;

use crate::gpu::AccumulatedRender;
use crate::gpu::GpuContext;
use crate::scene_data;
use crate::scenes;

/// Samples per accumulation pass. Small enough that a single draw call stays well
/// under the GPU watchdog timeout even at high resolutions.
const SAMPLES_PER_PASS: u32 = 8;

const CONFIG_DIR: &str = "renders/configs";
const OUTPUT_DIR: &str = "renders";

/// Render definition loaded from a TOML file. Every setting is explicit: the
/// shader has no defaults of its own to fall back on.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderFile {
    pub scene: String,
    pub camera: CameraDef,
    pub quality: QualityDef,
    pub output: OutputDef,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraDef {
    pub position: [f32; 3],
    pub look_at: [f32; 3],
    pub vup: [f32; 3],
    pub fov: f32,
    pub defocus_angle: f32,
    pub focus_dist: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityDef {
    pub samples: u32,
    pub bounces: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputDef {
    pub width: u32,
    pub height: u32,
}

impl RenderFile {
    /// Load a render definition from `renders/configs/<name>.toml`.
    pub fn load(name: &str) -> Result<Self, Box<dyn Error>> {
        let path = PathBuf::from(CONFIG_DIR).join(format!("{name}.toml"));
        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let def: RenderFile = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        def.validate(name)?;

        Ok(def)
    }

    fn validate(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if scenes::Scene::find(&self.scene).is_none() {
            return Err(format!(
                "Render {} uses unknown scene {}. Available scenes: {}",
                name,
                self.scene,
                scenes::names().collect::<Vec<_>>().join(", ")
            )
            .into());
        }
        if self.quality.samples < 1 {
            return Err(format!("Render {name} needs at least 1 sample").into());
        }
        if self.quality.bounces < 1 {
            return Err(format!("Render {name} needs at least 1 bounce").into());
        }
        if self.output.width < 1 || self.output.height < 1 {
            return Err(format!("Render {name} needs a non-zero output size").into());
        }
        Ok(())
    }

    /// Build the push constants for this render. Per-pass fields (resolution,
    /// samples, seed) are filled in by the GPU layer.
    fn constants(&self, background: [f32; 3]) -> shared::ShaderConstants {
        let position = self.camera.position;
        let look_at = self.camera.look_at;

        shared::ShaderConstants {
            width: 0,
            height: 0,
            time: 0.0,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cam_pos: position,
            cam_dir: [
                look_at[0] - position[0],
                look_at[1] - position[1],
                look_at[2] - position[2],
            ],
            cam_vup: self.camera.vup,
            fov_v: self.camera.fov,
            defocus_angle: self.camera.defocus_angle,
            focus_dist: self.camera.focus_dist,
            px_samples: 0,
            max_ray_bounce: self.quality.bounces,
            seed: 0,
            background,
        }
    }
}

/// Convert the accumulated linear image to an 8-bit sRGB image. The live and test
/// paths get this transfer for free from their sRGB render targets.
fn encode_srgb(accumulated: &[f32], width: u32, height: u32) -> image::RgbaImage {
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);

    for pixel in accumulated.chunks_exact(4) {
        for channel in &pixel[..3] {
            bytes.push((linear_to_srgb(*channel) * 255.0 + 0.5) as u8);
        }
        bytes.push(255);
    }

    image::RgbaImage::from_raw(width, height, bytes).expect("Failed to build image from pixel data")
}

fn linear_to_srgb(linear: f32) -> f32 {
    let linear = linear.clamp(0.0, 1.0);

    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);

    if hours > 0 {
        format!("{hours}h{minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m{seconds:02}s")
    } else if total > 0 {
        format!("{seconds}s")
    } else {
        format!("{:.1}s", duration.as_secs_f64())
    }
}

/// Render a definition by name and save the resulting image.
pub fn run_render(name: String) -> Result<(), Box<dyn Error>> {
    let def = RenderFile::load(&name)?;

    let instance = GpuContext::create_instance();
    let gpu = block_on(GpuContext::new(instance, None))?;

    let width = def.output.width;
    let height = def.output.height;

    // Passes are equal in size, so the requested sample count rounds up to the
    // nearest multiple that divides evenly
    let passes = def.quality.samples.div_ceil(SAMPLES_PER_PASS);
    let samples_per_pass = def.quality.samples.div_ceil(passes);
    let samples = passes * samples_per_pass;

    if samples != def.quality.samples {
        log::debug!(
            "{}: rounded {} samples up to {} ({} passes of {})",
            name,
            def.quality.samples,
            samples,
            passes,
            samples_per_pass
        );
    }

    log::info!(
        "rendering {} ({}) {}x{} {} spp {} bounces",
        name,
        def.scene,
        width,
        height,
        samples,
        def.quality.bounces
    );

    let scene = scene_data::build(&def.scene)
        .ok_or_else(|| format!("Scene {} has no scene data", def.scene))?;
    let buffers = gpu.upload_scene(&scene);

    let request = AccumulatedRender {
        scene: &buffers,
        width,
        height,
        passes,
        samples_per_pass,
        constants: def.constants(scene.background),
    };

    let pixels_per_pass = (width as u64) * (height as u64) * (samples_per_pass as u64);
    let start = Instant::now();

    let accumulated = block_on(gpu.render_to_image_accumulated(&request, |done| {
        let elapsed = start.elapsed();
        let per_pass = elapsed / done;
        let eta = per_pass * (passes - done);
        let throughput = (pixels_per_pass * done as u64) as f64 / elapsed.as_secs_f64() / 1e6;

        log::info!(
            "pass {}/{}  {}/{} spp  {}%  {:.1} Msamples/s  eta {}",
            done,
            passes,
            done * samples_per_pass,
            samples,
            done * 100 / passes,
            throughput,
            format_duration(eta)
        );
    }))?;

    let elapsed = start.elapsed();

    let image = encode_srgb(&accumulated, width, height);
    fs::create_dir_all(OUTPUT_DIR)?;
    let timestamp = Utc::now().format("%Y-%m-%d-%H-%M-%S");
    let path = PathBuf::from(OUTPUT_DIR).join(format!("{name}-{timestamp}.png"));
    image.save(&path)?;

    let throughput = (pixels_per_pass * passes as u64) as f64 / elapsed.as_secs_f64() / 1e6;
    log::info!(
        "saved {} ({} spp in {}, {:.1} Msamples/s avg)",
        path.display(),
        samples,
        format_duration(elapsed),
        throughput
    );

    Ok(())
}
