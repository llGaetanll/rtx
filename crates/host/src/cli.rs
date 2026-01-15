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
    /// Run benchmark with animated camera path
    Bench {
        /// Benchmark definition name (loads from benchmarks/<name>.toml).
        /// If not specified, runs all benchmarks in the benchmarks/ directory.
        name: Option<String>,
    },
}
