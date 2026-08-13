use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(name = "rtx")]
#[command(about = "A GPU ray tracer built with rust-gpu")]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Open a window and fly around a scene
    Live {
        /// Image config to start from (loads configs/image/<name>.toml). Its
        /// camera is the starting view; the sample count and window size are
        /// live mode's own.
        name: String,
    },
    /// Render every image config to a grid image
    Test,
    /// Render a high-quality still image
    Render {
        /// Image config name (loads configs/image/<name>.toml)
        name: String,
    },
    /// Time the frames of a video's camera path
    Bench {
        /// Video config name (loads configs/video/<name>.toml). If not
        /// specified, runs every config in configs/video/.
        name: Option<String>,
    },
    /// Generate SVG charts from benchmark results
    Chart,
    /// Summarize benchmark results as a table
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
