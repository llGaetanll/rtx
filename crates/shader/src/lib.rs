#![no_std]

use rtx_mat::Dielectric;
use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialTable;
use rtx_mat::Metal;
use rtx_obj::Instance;
use rtx_obj::Scene;
use rtx_prim::Color;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureTable;
use rtx_util::CameraParams;
use shared::ShaderConstants;
use spirv_std::glam::Vec4;
use spirv_std::glam::vec2;
use spirv_std::glam::vec4;
use spirv_std::spirv;

/// Build camera params from ShaderConstants. The host supplies every camera and
/// quality setting, along with the scene's background.
fn cam_params_from_constants(constants: &ShaderConstants) -> CameraParams {
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
        background: Color::new(
            constants.background[0],
            constants.background[1],
            constants.background[2],
        ),
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

/// The one entry point for every scene.
///
/// Scenes used to be Rust code with an entry point each, which meant every pixel
/// rebuilt the whole scene before tracing a single ray. They are now data the host
/// builds once and uploads, so this shader only reads them.
#[spirv(fragment)]
// Each argument is a binding the shader needs, not a parameter list to shorten
#[allow(clippy::too_many_arguments)]
pub fn trace_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    #[spirv(descriptor_set = 0, binding = 0, storage_buffer)] instances: &[Instance],
    #[spirv(descriptor_set = 0, binding = 1, storage_buffer)] lambertians: &[Lambertian],
    #[spirv(descriptor_set = 0, binding = 2, storage_buffer)] metals: &[Metal],
    #[spirv(descriptor_set = 0, binding = 3, storage_buffer)] dielectrics: &[Dielectric],
    #[spirv(descriptor_set = 0, binding = 4, storage_buffer)] diffuse_lights: &[DiffuseLight],
    #[spirv(descriptor_set = 0, binding = 5, storage_buffer)] solids: &[SolidTexture],
    output: &mut Vec4,
) {
    let cam = rtx_util::Camera::new(cam_params_from_constants(constants));

    let world = Scene::new(instances);
    let mat_table = MaterialTable {
        lambertians,
        metals,
        dielectrics,
        diffuse_lights,
    };
    let tex_table = TextureTable { solids };

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}
