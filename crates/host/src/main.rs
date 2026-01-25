use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Instant;

use clap::Parser;
use futures::executor::block_on;

mod bench_app;
mod camera_path;
mod cli;
mod gpu;
mod live_app;
mod spline;
mod window_surface;

use cli::Cli;
use cli::Commands;
use gpu::GpuContext;

fn run_test() -> Result<(), Box<dyn Error>> {
    log::debug!("Test mode: rendering all scenes to grid image...");

    let instance = GpuContext::create_instance();
    let gpu = block_on(GpuContext::new(instance, None))?;

    // All available scenes (fragment shader entry points)
    let scenes = [
        "cornell_box_fs",
        "quads_fs",
        "metal_test_fs",
        "dielectric_test_fs",
        "two_spheres_fs",
        "glass_debug_fs",
        "three_spheres_fs",
        "many_spheres_fs",
    ];

    // 720p per scene
    let scene_width = 1280u32;
    let scene_height = 720u32;

    // 4x4 grid
    let grid_cols = 4u32;
    let grid_rows = 4u32;
    let grid_width = scene_width * grid_cols;
    let grid_height = scene_height * grid_rows;

    // Create the final grid image with checkerboard background
    let mut grid_img = image::RgbaImage::new(grid_width, grid_height);

    // Fill with checkerboard pattern for empty slots
    let color_a = image::Rgba([0x17, 0x1d, 0x1c, 0xff]);
    let color_b = image::Rgba([0x3f, 0x50, 0x4d, 0xff]);
    let checker_size = 128u32;

    for y in 0..grid_height {
        for x in 0..grid_width {
            let checker_x = x / checker_size;
            let checker_y = y / checker_size;
            let color = if (checker_x + checker_y) % 2 == 0 {
                color_a
            } else {
                color_b
            };
            grid_img.put_pixel(x, y, color);
        }
    }

    // Render each scene and place in grid (top to bottom, left to right)
    for (i, scene) in scenes.iter().enumerate() {
        let col = (i as u32) % grid_cols;
        let row = (i as u32) / grid_cols;

        let start = Instant::now();
        let pixels = block_on(gpu.render_to_image(scene_width, scene_height, scene));
        let elapsed = start.elapsed();

        log::debug!(
            "Rendered {} ({}/{}) in {:.2?}",
            scene,
            i + 1,
            scenes.len(),
            elapsed
        );
        let scene_img = image::RgbaImage::from_raw(scene_width, scene_height, pixels)
            .expect("Failed to create image from pixel data");

        // Copy scene image into grid
        let x_offset = col * scene_width;
        let y_offset = row * scene_height;
        for y in 0..scene_height {
            for x in 0..scene_width {
                let pixel = scene_img.get_pixel(x, y);
                grid_img.put_pixel(x + x_offset, y + y_offset, *pixel);
            }
        }
    }

    std::fs::create_dir_all("renders")?;
    let path = "renders/render.png";

    log::debug!("Saving {}x{} grid to {}...", grid_width, grid_height, path);
    grid_img.save(path)?;

    log::info!("Saved {}", path);
    Ok(())
}

fn run_chart() -> Result<(), Box<dyn Error>> {
    let bench_results_dir = Path::new("bench/results");
    if !bench_results_dir.exists() {
        return Err("bench/results/ directory not found. Run some benchmarks first.".into());
    }

    let data = rtx_bench::load_all_benchmarks(bench_results_dir)?;
    if data.is_empty() {
        return Err("No benchmark data found in bench/results/".into());
    }

    log::debug!(
        "Loaded {} benchmark(s): {}",
        data.len(),
        data.iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let svg = rtx_bench::generate_svg(&data);

    fs::create_dir_all("bench/charts")?;
    let output_path = "bench/charts/chart.svg";
    fs::write(output_path, &svg)?;

    log::info!("Saved {}", output_path);
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Live { scene }) => live_app::run_live(&scene),
        Some(Commands::Test) => run_test(),
        Some(Commands::Bench { name: Some(name) }) => bench_app::run_bench(name),
        Some(Commands::Bench { name: None }) => bench_app::run_all_benchmarks(),
        Some(Commands::Chart) => run_chart(),
        None => live_app::run_live("cornell_box_fs"),
    }
}
