use std::path::PathBuf;

use clap::Args;
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

/// The two files every render needs: what exists, and how it is looked at. Both
/// are paths, so a scene and a config are ordinary files that can live anywhere
/// rather than names resolved against a directory the program knows about.
#[derive(Args)]
pub struct Files {
    /// Path to a scene TOML file
    #[arg(short, long)]
    pub scene: PathBuf,

    /// Path to a config TOML file, holding the camera, quality and output
    #[arg(short, long)]
    pub config: PathBuf,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Open a window and fly around a scene
    Live {
        /// The image config's camera is the starting view; the sample count and
        /// window size are live mode's own.
        #[command(flatten)]
        files: Files,
    },
    /// Render a high-quality still image
    Render {
        #[command(flatten)]
        files: Files,

        /// Watch the image accumulate in a window. Closing it early saves what
        /// has been rendered so far.
        #[arg(short, long)]
        preview: bool,
    },
    /// Time the frames of a video's camera path
    ///
    /// Given a scene and a video config, times that pair. Given neither, times
    /// every benchmark listed in bench.toml. The two go together, so one without
    /// the other is an error rather than half a benchmark.
    Bench {
        /// Path to a scene TOML file
        #[arg(short, long, requires = "config")]
        scene: Option<PathBuf>,

        /// Path to a video config TOML file
        #[arg(short, long, requires = "scene")]
        config: Option<PathBuf>,
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
