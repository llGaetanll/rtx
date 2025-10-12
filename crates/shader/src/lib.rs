#![no_std]

use rtx_mat::{Hit, Lambertian, MaterialType};
use rtx_obj::{List, Object, Sphere};
use rtx_prim::{Color, Point3, RandState, Vec3};
use rtx_tex::{SolidTexture, TextureType};
use rtx_util::{Camera, CameraParams};
use shared::ShaderConstants;
use spirv_std::glam::{vec2, vec4, Vec4, Vec4Swizzles};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

fn gen_params(img_width: usize, img_height: usize) -> CameraParams {
    CameraParams {
        lookfrom: Point3::new(13., 2., 3.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 20.0,
        defocus_angle: 0.8,
        focus_dist: 10.0,
        px_samples: 80,
        max_ray_bounce: 10,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.),
    }
}

const STATE: RandState = 42;

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

    let params = gen_params(constants.width as usize, constants.height as usize);
    let cam = Camera::new(params);

    // let object = Object::Sphere(sphere);

    // let objects: [Object; 1] = [object];
    // let world = List::from_objects(&objects);

    // let i = (frag_coord.x * constants.width as f32).floor() as usize;
    // let j = (frag_coord.y * constants.height as f32).floor() as usize;
    //
    // let mut state = STATE;
    // let mut color = Color::new(0., 0., 0.);
    //
    // let ray = cam.get_ray(&mut state, i, j);
    // color += cam.ray_color(&mut state, &ray, &world, cam.max_ray_bounce);

    *output = vec4(0., 0., 0., 1.0);
}
