use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(name = "rtx")]
#[command(about = "A GPU ray tracer built with rust-gpu")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Open a window and render the scene live
    Live {
        /// Which scene to render (fragment shader entry point)
        #[arg(short, long, default_value = "cornell_box_fs")]
        scene: String,
    },
    /// Render all test scenes to a grid image
    Test,
    /// Render a high-quality still image from a render definition
    Render {
        /// Render definition name (loads from renders/configs/<name>.toml)
        name: String,
    },
    /// Run benchmark with animated camera path
    Bench {
        /// Benchmark definition name (loads from bench/configs/<name>.toml).
        /// If not specified, runs all benchmarks in the bench/configs/ directory.
        name: Option<String>,
    },
    /// Generate SVG charts from benchmark results
    Chart,
    /// Summarise benchmark results as a table
    ///
    /// One row per recorded run, ordered by git history with the newest commit
    /// last. The first columns identify the run: the commit it was built from, the
    /// benchmark name, and the settings that decide how much work a frame is
    /// (resolution, samples per pixel, maximum bounces, frames recorded). Two rows
    /// are only comparable when those settings match.
    ///
    /// The remaining columns are frame times in microseconds: the fastest frame,
    /// the 1st, 25th, 50th, 75th and 99th percentile, and the slowest. Each is an
    /// observed frame rather than an interpolated value. Startup cost is recorded
    /// like any other frame, so the slowest frames of a run are usually shader
    /// compilation and a GPU still at idle clocks rather than the renderer.
    ///
    /// Runs built from a commit that is not in this repository's history cannot be
    /// placed in the ordering; they are listed first and dimmed.
    Stats,
}
