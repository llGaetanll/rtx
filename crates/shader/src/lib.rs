#![no_std]

use rtx_mat::Dielectric;
use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialTable;
use rtx_mat::Metal;
use rtx_obj::BvhNode;
use rtx_obj::Instance;
use rtx_obj::Light;
use rtx_obj::Lights;
use rtx_obj::Scene;
use rtx_prim::Color;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureTable;
use rtx_util::CameraParams;
use shared::BlitConstants;
use shared::ShaderConstants;
use shared::TileBlitConstants;
use spirv_std::glam::Vec4;
use spirv_std::glam::uvec2;
use spirv_std::glam::vec2;
use spirv_std::glam::vec4;
use spirv_std::image::Image2d;
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
fn pcg(state: u32) -> u32 {
    let state = state.wrapping_mul(747796405).wrapping_add(2891336453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}

/// Seed one pixel's chain for one pass.
///
/// Each input gets its own mixing round. Summing them into one word instead left
/// vertical neighbours one apart going into a single round, which is not enough
/// avalanche: their first draws came out correlated at r = -0.13 eight rows apart,
/// and no amount of passes averaged that away.
///
/// `x` and `y` are the pixel's place in the whole image, not in the tile being
/// drawn. Seeding from the tile local coordinate instead would give the first
/// pixel of every tile the same chain, and the tiling would print itself into the
/// noise.
fn gen_state(x: u32, y: u32, seed: u32) -> u32 {
    // Zero is a fixed point of the xorshift chain, so never hand it one
    pcg(x ^ pcg(y ^ pcg(seed))).max(1)
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
    #[spirv(descriptor_set = 0, binding = 6, storage_buffer)] lights: &[Light],
    #[spirv(descriptor_set = 0, binding = 7, storage_buffer)] bvh: &[BvhNode],
    output: &mut Vec4,
) {
    let cam = rtx_util::Camera::new(cam_params_from_constants(constants));

    let world = Scene::new(instances, bvh);

    // The buffer is padded to one element when the scene has no emitters, since a
    // zero sized binding is not allowed, so the count decides how much of it is
    // real rather than its length
    let lights = Lights::bounded(lights, constants.light_count as usize);
    let mat_table = MaterialTable {
        lambertians,
        metals,
        dielectrics,
        diffuse_lights,
    };
    let tex_table = TextureTable { solids };

    // Where this fragment sits in the whole image. The draw covers one tile, so
    // the fragment coordinate is tile local and the camera, which knows only the
    // full image, has to be asked about the pixel this really is
    let x = frag_coord.x as u32 + constants.tile_x;
    let y = frag_coord.y as u32 + constants.tile_y;

    let i = y as usize;
    let j = x as usize;

    let mut state = gen_state(x, y, constants.seed);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world, &lights);

    *output = vec4(color.x, color.y, color.z, 1.0);
}

/// Shrink one finished tile into the preview image.
///
/// Drawn over the whole preview target, so most fragments are outside the
/// rectangle this tile occupies and are discarded rather than written. Keeping
/// the other tiles is the point: the preview is built up over the course of the
/// render and each tile only touches its own part of it.
#[spirv(fragment)]
pub fn tile_blit_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &TileBlitConstants,
    #[spirv(descriptor_set = 0, binding = 0)] tile: &Image2d,
    output: &mut Vec4,
) {
    let x = frag_coord.x - constants.dst_x;
    let y = frag_coord.y - constants.dst_y;

    if x < 0.0 || y < 0.0 || x >= constants.dst_width || y >= constants.dst_height {
        // Another tile's part of the preview, or the margin past the last tile
        // in a row. Discarding leaves whatever was drawn there before
        spirv_std::arch::kill();
    }

    // Nearest texel of the tile, which is all a non filterable float image
    // allows. The preview is a look at the framing rather than the render, so
    // the aliasing that comes with shrinking this way does not matter
    let u = (x / constants.dst_width * constants.tile_width as f32) as u32;
    let v = (y / constants.dst_height * constants.tile_height as f32) as u32;

    let sum: Vec4 = tile.fetch(uvec2(
        u.min(constants.tile_width - 1),
        v.min(constants.tile_height - 1),
    ));
    let color = sum.truncate() * constants.scale;

    *output = vec4(color.x, color.y, color.z, 1.0);
}

/// Show an in-progress accumulated image in a window.
///
/// The image keeps its own resolution and aspect ratio, so it is scaled to fit
/// the window and the leftover space on either side is left black. Texels are
/// picked rather than filtered, which the float target the render accumulates
/// into cannot do anyway.
#[spirv(fragment)]
pub fn blit_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &BlitConstants,
    #[spirv(descriptor_set = 0, binding = 0)] accumulated: &Image2d,
    output: &mut Vec4,
) {
    let image = vec2(constants.image_width as f32, constants.image_height as f32);
    let surface = vec2(
        constants.surface_width as f32,
        constants.surface_height as f32,
    );

    let zoom = (surface.x / image.x).min(surface.y / image.y);
    let origin = (surface - image * zoom) * 0.5;
    let texel = (vec2(frag_coord.x, frag_coord.y) - origin) / zoom;

    if texel.x < 0.0 || texel.y < 0.0 || texel.x >= image.x || texel.y >= image.y {
        *output = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    let sum: Vec4 = accumulated.fetch(uvec2(texel.x as u32, texel.y as u32));
    let color = sum.truncate() * constants.scale;

    *output = vec4(color.x, color.y, color.z, 1.0);
}
