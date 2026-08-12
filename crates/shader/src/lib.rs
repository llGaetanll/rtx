#![no_std]

mod scene;

use rtx_prim::Color;
use rtx_prim::Vec3;
use rtx_util::CameraParams;
use shared::ShaderConstants;
use spirv_std::glam::Vec4;
use spirv_std::glam::vec2;
use spirv_std::glam::vec4;
use spirv_std::spirv;

/// Build camera params from ShaderConstants. The host supplies every camera and
/// quality setting; the scene only contributes its background.
fn cam_params_from_constants(constants: &ShaderConstants, background: Color) -> CameraParams {
    let lookfrom = Vec3::new(
        constants.cam_pos[0],
        constants.cam_pos[1],
        constants.cam_pos[2],
    );
    let cam_dir = Vec3::new(
        constants.cam_dir[0],
        constants.cam_dir[1],
        constants.cam_dir[2],
    );

    CameraParams {
        lookfrom,
        lookat: lookfrom + cam_dir,
        vup: Vec3::new(
            constants.cam_vup[0],
            constants.cam_vup[1],
            constants.cam_vup[2],
        ),
        fov_v: constants.fov_v,
        defocus_angle: constants.defocus_angle,
        focus_dist: constants.focus_dist,
        px_samples: constants.px_samples,
        max_ray_bounce: constants.max_ray_bounce,
        img_width: constants.width as usize,
        img_height: constants.height as usize,
        background,
    }
}

/// Basic PCG
fn gen_state(frag_coord: Vec4, seed: u32) -> u32 {
    let x = frag_coord.x as u32;
    let y = frag_coord.y as u32;

    let state = x
        .wrapping_mul(747796405)
        .wrapping_add(y)
        .wrapping_add(seed.wrapping_mul(2891336453));
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
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_CORNELL_BOX);
    let (cam, mat_table, tex_table, world) = scene::cornell_box(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn quads_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_QUADS);
    let (cam, mat_table, tex_table, world) = scene::quads(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn metal_test_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_METAL_TEST);
    let (cam, mat_table, tex_table, world) = scene::metal_test(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn dielectric_test_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_DIELECTRIC_TEST);
    let (cam, mat_table, tex_table, world) = scene::dielectric_test(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn two_spheres_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_TWO_SPHERES);
    let (cam, mat_table, tex_table, world) = scene::two_spheres(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn glass_debug_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_GLASS_DEBUG);
    let (cam, mat_table, tex_table, world) = scene::glass_debug(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn three_spheres_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_THREE_SPHERES);
    let (cam, mat_table, tex_table, world) = scene::three_spheres(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

#[spirv(fragment)]
pub fn many_spheres_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    let cam_params = cam_params_from_constants(constants, scene::BACKGROUND_MANY_SPHERES);
    let (cam, mat_table, tex_table, world) = scene::many_spheres(cam_params);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}
