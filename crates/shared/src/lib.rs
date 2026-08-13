#![cfg_attr(target_arch = "spirv", no_std)]

use bytemuck::Pod;
use bytemuck::Zeroable;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ShaderConstants {
    pub width: u32,
    pub height: u32,
    pub time: f32,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cam_pos: [f32; 3],
    pub cam_dir: [f32; 3],
    pub cam_vup: [f32; 3],

    /// Vertical field of view in degrees
    pub fov_v: f32,

    /// Defocus (aperture) angle in degrees
    pub defocus_angle: f32,

    /// Distance to the plane of perfect focus
    pub focus_dist: f32,

    /// Rays per pixel
    pub px_samples: u32,

    /// Maximum ray bounce depth
    pub max_ray_bounce: u32,

    /// Mixed into the per-pixel RNG seed so repeated passes over the same pixel
    /// produce different noise. Used to accumulate samples across draw calls.
    pub seed: u32,

    /// Color of a ray that escapes the scene. Travels with the camera settings
    /// rather than the scene buffers because it is a single value the shader
    /// needs on every miss.
    pub background: [f32; 3],

    /// How many entries of the light buffer are real.
    ///
    /// The buffer cannot answer this itself: a zero sized binding is not allowed,
    /// so a scene with no emitters still uploads one zeroed light. Sampling that
    /// one would aim every shadow ray at a lightless point at the origin.
    pub light_count: u32,
}

/// Settings for drawing an accumulated image into a window.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct BlitConstants {
    /// Size of the accumulated image, which is the render's own resolution and
    /// has nothing to do with the size of the window showing it.
    pub image_width: u32,
    pub image_height: u32,
    pub surface_width: u32,
    pub surface_height: u32,

    /// Reciprocal of the number of passes drawn so far, which turns the summed
    /// image into the average an unfinished render should look like.
    pub scale: f32,
}
