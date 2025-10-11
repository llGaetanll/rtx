#![no_std]

use shared::ShaderConstants;
use spirv_std::glam::{vec2, vec4, Vec4, Vec4Swizzles};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

#[spirv(vertex)]
pub fn main_vs(#[spirv(vertex_index)] vert_id: i32, #[spirv(position)] out_pos: &mut Vec4) {
    let uv = vec2(((vert_id << 1) & 2) as f32, (vert_id & 2) as f32);
    let pos = uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
    *out_pos = vec4(pos.x, pos.y, 0.0, 1.0);
}

#[spirv(fragment)]
pub fn main_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let resolution = vec2(constants.width as f32, constants.height as f32);
    let uv = frag_coord.xy() / resolution;

    // Simple gradient based on UV coordinates and time
    let time = constants.time;
    let r = 0.5 + 0.5 * (time + uv.x * 3.0).sin();
    let g = 0.5 + 0.5 * (time * 1.3 + uv.y * 3.0).sin();
    let b = 0.5 + 0.5 * (time * 1.7 + (uv.x + uv.y) * 2.0).sin();

    *output = vec4(r, g, b, 1.0);
}
