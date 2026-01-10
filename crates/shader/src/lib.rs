#![no_std]

mod scene;

use shared::ShaderConstants;
use spirv_std::glam::Vec4;
use spirv_std::glam::vec2;
use spirv_std::glam::vec4;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

/// Basic PCG
fn gen_state(frag_coord: Vec4) -> u32 {
    let x = frag_coord.x as u32;
    let y = frag_coord.y as u32;

    let state = x.wrapping_mul(747796405).wrapping_add(y);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}

#[spirv(vertex)]
pub fn main_vs(#[spirv(vertex_index)] vert_id: i32, #[spirv(position)] out_pos: &mut Vec4) {
    let uv = vec2(((vert_id << 1) & 2) as f32, (vert_id & 2) as f32);
    let pos = uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
    *out_pos = vec4(pos.x, pos.y, 0.0, 1.0);
}

#[spirv(fragment)]
pub fn cornell_box_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let (cam, mat_table, tex_table, world) =
        scene::cornell_box(constants.width as usize, constants.height as usize);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}
