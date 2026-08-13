use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

use spirv_builder::MetadataPrintout;
use spirv_builder::SpirvBuilder;

const TARGET: &str = "spirv-unknown-vulkan1.2";

fn build_shader(path_to_crate: &str) -> Result<(), Box<dyn Error>> {
    let builder = SpirvBuilder::new(path_to_crate, TARGET).print_metadata(MetadataPrintout::Full);

    let _result = builder.build()?;
    Ok(())
}

/// Whether to leave the shader alone and reuse whatever the last real build made.
///
/// rust-analyzer runs cargo over the whole workspace whenever a file is saved.
/// That runs this build script, which starts a second cargo for the SPIR-V
/// target, and that one waits on the shared package cache lock the first one is
/// holding. Neither can finish, so every later cargo command queues behind them
/// until the processes are killed. It only bites when the dependency graph
/// changes, since that is when the lock is taken, which makes it a confusing way
/// to lose half an hour.
///
/// Set `RTX_SKIP_SHADER_BUILD` to force the same behaviour by hand.
fn skip_shader_build() -> bool {
    if env::var_os("RTX_SKIP_SHADER_BUILD").is_some() {
        return true;
    }

    env::var("RUSTC_WRAPPER").is_ok_and(|wrapper| wrapper.contains("rust-analyzer"))
}

/// Where a previous real build left the compiled shader.
///
/// `include_spirv!` needs the path at compile time whether or not this script
/// built anything, so a skipped build points at the existing artifact. Before the
/// first real build there is nothing to point at, and the compile error names the
/// missing file.
fn previously_built_shader() -> Option<PathBuf> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    let path = manifest
        .join("../../target/spirv-builder")
        .join(TARGET)
        .join("release/deps/shader.spv");

    path.exists().then_some(path)
}

fn set_git_sha() {
    // Re-run if HEAD changes (e.g., branch switch)
    println!("cargo::rerun-if-changed=../../.git/HEAD");

    // Also watch the ref that HEAD points to (e.g., refs/heads/master)
    // This ensures we re-run when new commits are made on the current branch
    if let Ok(head_contents) = std::fs::read_to_string("../../.git/HEAD")
        && let Some(ref_path) = head_contents.trim().strip_prefix("ref: ")
    {
        println!("cargo::rerun-if-changed=../../.git/{ref_path}");
    }

    let sha = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo::rustc-env=GIT_SHA={sha}");
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-env-changed=RTX_SKIP_SHADER_BUILD");

    if skip_shader_build() {
        match previously_built_shader() {
            Some(path) => println!("cargo::rustc-env=shader.spv={}", path.display()),
            None => {
                println!("cargo::warning=skipped the shader build, and none was found to reuse")
            }
        }
    } else {
        build_shader("../shader")?;
    }

    set_git_sha();

    Ok(())
}
