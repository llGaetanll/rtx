use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use chrono::Utc;
use futures::executor::block_on;

use crate::config;
use crate::config::ImageConfig;
use crate::gpu::AccumulatedRender;
use crate::gpu::GpuContext;
use crate::scene_data;

/// Samples per accumulation pass. Small enough that a single draw call stays well
/// under the GPU watchdog timeout even at high resolutions.
const SAMPLES_PER_PASS: u32 = 8;

const OUTPUT_DIR: &str = "renders";

/// How a requested sample count is split into passes.
#[derive(Copy, Clone)]
pub struct RenderPlan {
    pub passes: u32,
    pub samples_per_pass: u32,
    /// What the passes add up to, which is the requested count rounded up.
    pub samples: u32,
}

impl RenderPlan {
    /// Passes are equal in size, so the requested sample count rounds up to the
    /// nearest multiple that divides evenly.
    fn new(name: &str, requested: u32) -> Self {
        let passes = requested.div_ceil(SAMPLES_PER_PASS);
        let samples_per_pass = requested.div_ceil(passes);
        let samples = passes * samples_per_pass;

        if samples != requested {
            log::debug!(
                "{name}: rounded {requested} samples up to {samples} ({passes} passes of {samples_per_pass})"
            );
        }

        Self {
            passes,
            samples_per_pass,
            samples,
        }
    }
}

/// Convert the accumulated linear image to an 8-bit sRGB image. The live path
/// gets this transfer for free from its sRGB render target.
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

pub fn format_duration(duration: Duration) -> String {
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

/// Timing for a render in progress, and the lines it reports as it goes.
pub struct Progress {
    plan: RenderPlan,
    pixels_per_pass: u64,
    start: Instant,
}

impl Progress {
    pub fn new(plan: RenderPlan, width: u32, height: u32) -> Self {
        Self {
            plan,
            pixels_per_pass: (width as u64) * (height as u64) * (plan.samples_per_pass as u64),
            start: Instant::now(),
        }
    }

    fn msamples_per_second(&self, done: u32) -> f64 {
        (self.pixels_per_pass * done as u64) as f64 / self.start.elapsed().as_secs_f64() / 1e6
    }

    /// A line per finished pass, with an estimate of the time left.
    pub fn log_pass(&self, done: u32) {
        let eta = self.start.elapsed() / done * (self.plan.passes - done);

        log::info!(
            "pass {}/{}  {}/{} spp  {}%  {:.1} Msamples/s  eta {}",
            done,
            self.plan.passes,
            done * self.plan.samples_per_pass,
            self.plan.samples,
            done * 100 / self.plan.passes,
            self.msamples_per_second(done),
            format_duration(eta)
        );
    }

    /// The closing line. `done` is what was actually drawn, which is fewer passes
    /// than planned when a preview is closed early.
    pub fn log_saved(&self, path: &Path, done: u32) {
        log::info!(
            "saved {} ({} spp in {}, {:.1} Msamples/s avg)",
            path.display(),
            done * self.plan.samples_per_pass,
            format_duration(self.start.elapsed()),
            self.msamples_per_second(done)
        );
    }
}

/// Save an accumulated image under a timestamped name and return where it went.
pub fn save(
    name: &str,
    accumulated: &[f32],
    width: u32,
    height: u32,
) -> Result<PathBuf, Box<dyn Error>> {
    let image = encode_srgb(accumulated, width, height);
    fs::create_dir_all(OUTPUT_DIR)?;
    let timestamp = Utc::now().format("%Y-%m-%d-%H-%M-%S");
    let path = PathBuf::from(OUTPUT_DIR).join(format!("{name}-{timestamp}.png"));
    image.save(&path)?;

    Ok(path)
}

/// Announce what is about to be rendered.
pub fn log_start(name: &str, scene_path: &Path, def: &ImageConfig, plan: RenderPlan) {
    log::info!(
        "rendering {} ({}) {}x{} {} spp {} bounces",
        name,
        scene_path.display(),
        def.output.width,
        def.output.height,
        plan.samples,
        def.quality.bounces
    );
}

/// Render `scene_path` through `config_path` and save the resulting image.
pub fn run_render(
    scene_path: &Path,
    config_path: &Path,
    preview: bool,
) -> Result<(), Box<dyn Error>> {
    let def = ImageConfig::load(config_path)?;
    let name = config::name_of(config_path);
    let plan = RenderPlan::new(&name, def.quality.samples);

    if preview {
        return crate::preview_app::run_preview(name, scene_path.to_path_buf(), def, plan);
    }

    let instance = GpuContext::create_instance();
    let gpu = block_on(GpuContext::new(instance, None))?;

    let width = def.output.width;
    let height = def.output.height;
    let RenderPlan {
        passes,
        samples_per_pass,
        ..
    } = plan;

    log_start(&name, scene_path, &def, plan);

    let scene = scene_data::load(scene_path)?;
    let buffers = gpu.upload_scene(&scene);

    let request = AccumulatedRender {
        scene: &buffers,
        width,
        height,
        passes,
        samples_per_pass,
        constants: def
            .camera
            .constants(width, height, def.quality, scene.info()),
    };

    let progress = Progress::new(plan, width, height);

    let accumulated =
        block_on(gpu.render_to_image_accumulated(&request, |done| progress.log_pass(done)))?;

    let path = save(&name, &accumulated, width, height)?;
    progress.log_saved(&path, passes);

    Ok(())
}
