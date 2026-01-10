#![no_std]

use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialInfo;
use rtx_mat::MaterialKind;
use rtx_mat::MaterialTable;
use rtx_obj::List;
use rtx_obj::Quad;
use rtx_obj::Sphere;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureKind;
use rtx_tex::TextureTable;
use rtx_util::Camera;
use rtx_util::CameraParams;
use shared::ShaderConstants;
use spirv_std::glam::Vec4;
use spirv_std::glam::vec2;
use spirv_std::glam::vec4;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

fn gen_params(img_width: usize, img_height: usize) -> CameraParams {
    CameraParams {
        lookfrom: Point3::new(278., 278., -800.),
        lookat: Point3::new(278., 278., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 10.0,
        px_samples: 80,
        max_ray_bounce: 10,
        img_width,
        img_height,
        background: Color::new(0., 0., 0.),
    }
}

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
pub fn main_fs(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(push_constant)] constants: &ShaderConstants,
    output: &mut Vec4,
) {
    /*
    let resolution = vec2(constants.width as f32, constants.height as f32);
    let uv = frag_coord.xy() / resolution;

    // Simple gradient based on UV coordinates and time
    let time = constants.time;
    let r = 0.5 + 0.5 * (time + uv.x * 3.0).sin();
    let g = 0.5 + 0.5 * (time * 1.3 + uv.y * 3.0).sin();
    let b = 0.5 + 0.5 * (time * 1.7 + (uv.x + uv.y) * 2.0).sin();

    *output = vec4(r, g, b, 1.0);
    */

    // Textures: red, green, white, light
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.65, 0.05, 0.05))); // 0: red
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.12, 0.45, 0.15))); // 1: green
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.73, 0.73, 0.73))); // 2: white
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(15., 15., 15.))); // 3: light

    // Materials
    let mut mat_table = MaterialTable::new();
    let red_tex = TextureInfo {
        kind: TextureKind::Solid,
        index: 0,
    };
    let green_tex = TextureInfo {
        kind: TextureKind::Solid,
        index: 1,
    };
    let white_tex = TextureInfo {
        kind: TextureKind::Solid,
        index: 2,
    };
    let light_tex = TextureInfo {
        kind: TextureKind::Solid,
        index: 3,
    };

    mat_table
        .lambertians
        .push(Lambertian::from_texture(red_tex)); // 0: red
    mat_table
        .lambertians
        .push(Lambertian::from_texture(green_tex)); // 1: green
    mat_table
        .lambertians
        .push(Lambertian::from_texture(white_tex)); // 2: white
    mat_table
        .diffuse_lights
        .push(DiffuseLight::from_texture(light_tex)); // 0: light

    let red_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let green_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let white_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 2,
    };
    let light_mat = MaterialInfo {
        kind: MaterialKind::DiffuseLight,
        index: 0,
    };

    let params = gen_params(constants.width as usize, constants.height as usize);
    let cam = Camera::new(params);

    // Cornell box walls
    // Left wall (green)
    let left_wall = Quad::new(
        Point3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        green_mat,
    );
    // Right wall (red)
    let right_wall = Quad::new(
        Point3::new(0., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        red_mat,
    );
    // Floor (white)
    let floor = Quad::new(
        Point3::new(0., 0., 0.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 0., 555.),
        white_mat,
    );
    // Ceiling (white)
    let ceiling = Quad::new(
        Point3::new(555., 555., 555.),
        Vec3::new(-555., 0., 0.),
        Vec3::new(0., 0., -555.),
        white_mat,
    );
    // Back wall (white)
    let back_wall = Quad::new(
        Point3::new(0., 0., 555.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        white_mat,
    );
    // Light (on ceiling)
    let light = Quad::new(
        Point3::new(343., 554., 332.),
        Vec3::new(-130., 0., 0.),
        Vec3::new(0., 0., -105.),
        light_mat,
    );

    // NOTE: rust-gpu cannot handle empty arrays (fails with "cannot offset a pointer
    // to an arbitrary element" when indexing). This dummy sphere is placed far behind
    // the camera so it's never visible, but ensures the sphere array is non-empty.
    let dummy_sphere = Sphere::fixed(Point3::new(0., 0., -1000.), 0.001, white_mat);

    let world = List::from_objects(
        [dummy_sphere],
        [left_wall, right_wall, floor, ceiling, back_wall, light],
    );

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}
