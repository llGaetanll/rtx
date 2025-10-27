#![no_std]

use rtx_mat::{Dielectric, Hit, Lambertian, MaterialInfo, MaterialKind, MaterialTable, Metal};
use rtx_obj::{List, Sphere};
use rtx_prim::{rand, Color, Point3, RandState, Vec3, F};
use rtx_tex::{SolidTexture, TextureInfo, TextureKind, TextureTable};
use rtx_util::{Camera, CameraParams};
use shared::ShaderConstants;
use spirv_std::glam::{vec2, vec4, Vec2, Vec4, Vec4Swizzles};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

fn gen_params(img_width: usize, img_height: usize) -> CameraParams {
    CameraParams {
        lookfrom: Point3::new(13., 2., 3.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 20.0,
        defocus_angle: 0.0,
        focus_dist: 10.0,
        px_samples: 80,
        max_ray_bounce: 3,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.),
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

    let tex_table = TextureTable {
        solids: [
            SolidTexture::from_color(Color::new(0.65, 0.05, 0.05)),
            SolidTexture::from_color(Color::new(0.12, 0.45, 0.15)),
        ],
    };

    let mat_table = MaterialTable {
        lambertians: [
            Lambertian::from_texture(TextureInfo {
                kind: TextureKind::Solid,
                index: 0,
            }),
            Lambertian::from_texture(TextureInfo {
                kind: TextureKind::Solid,
                index: 1,
            }),
        ],
        metals: [Metal::new(Color::new(0.5, 0.5, 0.5), 0.3)],
        dielectrics: [Dielectric::new(0.4)],
    };

    let params = gen_params(constants.width as usize, constants.height as usize);
    let cam = Camera::new(params);

    let list = [
        Sphere::fixed(
            Point3::new(0., 1., 0.),
            1.,
            MaterialInfo {
                kind: MaterialKind::Lambertian,
                index: 0,
            },
        ),
        Sphere::fixed(
            Point3::new(0., -100., 0.),
            100.,
            MaterialInfo {
                kind: MaterialKind::Lambertian,
                index: 1,
            },
        ),
    ];

    let world = List::from_objects(list);

    let i = frag_coord.y as usize;
    let j = frag_coord.x as usize;

    let mut state = gen_state(frag_coord);

    let color = cam.render(&mut state, i, j, &mat_table, &tex_table, &world);

    *output = vec4(color.x, color.y, color.z, 1.0);
}
