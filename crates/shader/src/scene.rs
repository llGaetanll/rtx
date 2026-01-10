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

pub fn cornell_box(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<1, 6>) {
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

    let cam = Camera::new(CameraParams {
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
    });

    // Cornell box walls
    let left_wall = Quad::new(
        Point3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        green_mat,
    );
    let right_wall = Quad::new(
        Point3::new(0., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        red_mat,
    );
    let floor = Quad::new(
        Point3::new(0., 0., 0.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 0., 555.),
        white_mat,
    );
    let ceiling = Quad::new(
        Point3::new(555., 555., 555.),
        Vec3::new(-555., 0., 0.),
        Vec3::new(0., 0., -555.),
        white_mat,
    );
    let back_wall = Quad::new(
        Point3::new(0., 0., 555.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        white_mat,
    );
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

    (cam, mat_table, tex_table, world)
}
