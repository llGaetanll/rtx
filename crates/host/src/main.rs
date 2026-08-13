use std::error::Error;
use std::fs;
use std::path::Path;

use clap::Parser;

mod bench_app;
mod cli;
mod config;
mod gpu;
mod live_app;
mod preview_app;
mod render_app;
mod scene_data;
mod stats;
mod window_surface;

use cli::Cli;
use cli::Commands;

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

    log::info!("Saved {output_path}");
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Live { files } => live_app::run_live(&files.scene, &files.config),
        Commands::Render { files, preview } => {
            render_app::run_render(&files.scene, &files.config, preview)
        }
        Commands::Bench {
            scene: Some(scene),
            config: Some(config),
        } => bench_app::run_bench(&scene, &config),
        // Clap rejects one without the other, so nothing else is left
        Commands::Bench { .. } => bench_app::run_all_benchmarks(),
        Commands::Chart => run_chart(),
        Commands::Stats => stats::run_stats(),
    }
}
