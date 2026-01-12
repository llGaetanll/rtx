use rtx_mat::Dielectric;
use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialInfo;
use rtx_mat::MaterialKind;
use rtx_mat::MaterialTable;
use rtx_mat::Metal;
use rtx_obj::Instance;
use rtx_obj::Scene;
use rtx_prim::Color;
use rtx_prim::Point3;
use rtx_prim::Vec3;
use rtx_tex::SolidTexture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureKind;
use rtx_tex::TextureTable;
use rtx_util::Camera;
use rtx_util::CameraParams;

const PX_SAMPLES: u32 = 40;
const MAX_RAY_BOUNCE: u32 = 10;

pub fn cornell_box(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, Scene) {
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

    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        })); // 0: red
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        })); // 1: green
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 2,
        })); // 2: white
    mat_table
        .diffuse_lights
        .push(DiffuseLight::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 3,
        })); // 0: light

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
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0., 0., 0.),
    });

    let mut world = Scene::new();
    // Left wall (green)
    world.push(Instance::quad(
        Point3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        green_mat,
    ));
    // Right wall (red)
    world.push(Instance::quad(
        Point3::new(0., 0., 0.),
        Vec3::new(0., 555., 0.),
        Vec3::new(0., 0., 555.),
        red_mat,
    ));
    // Floor
    world.push(Instance::quad(
        Point3::new(0., 0., 0.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 0., 555.),
        white_mat,
    ));
    // Ceiling
    world.push(Instance::quad(
        Point3::new(555., 555., 555.),
        Vec3::new(-555., 0., 0.),
        Vec3::new(0., 0., -555.),
        white_mat,
    ));
    // Back wall
    world.push(Instance::quad(
        Point3::new(0., 0., 555.),
        Vec3::new(555., 0., 0.),
        Vec3::new(0., 555., 0.),
        white_mat,
    ));
    // Light
    world.push(Instance::quad(
        Point3::new(343., 554., 332.),
        Vec3::new(-130., 0., 0.),
        Vec3::new(0., 0., -105.),
        light_mat,
    ));

    (cam, mat_table, tex_table, world)
}

pub fn many_spheres(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    let mut mat_table = MaterialTable::new();

    // Ground texture/material
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5)));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        }));
    let ground_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };

    // Small lambertian sphere textures (indices 1-8)
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.73, 0.24, 0.14)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.22, 0.55, 0.34)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.85, 0.62, 0.18)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.35, 0.28, 0.65)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.68, 0.15, 0.42)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.18, 0.45, 0.72)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.82, 0.75, 0.21)));
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.42, 0.65, 0.38)));

    // Small lambertian materials (indices 1-8)
    for i in 1..=8 {
        mat_table
            .lambertians
            .push(Lambertian::from_texture(TextureInfo {
                kind: TextureKind::Solid,
                index: i,
            }));
    }

    // Brown sphere texture for left main sphere (index 9)
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.4, 0.2, 0.1)));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 9,
        }));

    // Metal spheres (indices 0-8)
    mat_table
        .metals
        .push(Metal::new(Color::new(0.85, 0.75, 0.65), 0.1));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.72, 0.72, 0.78), 0.0));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.95, 0.85, 0.55), 0.2));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.65, 0.55, 0.45), 0.3));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.88, 0.68, 0.58), 0.05));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.75, 0.82, 0.72), 0.15));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.92, 0.78, 0.62), 0.0));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.68, 0.78, 0.88), 0.1));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.7, 0.6, 0.5), 0.0));

    // Glass spheres (indices 0-3)
    mat_table.dielectrics.push(Dielectric::new(1.5));
    mat_table.dielectrics.push(Dielectric::new(1.45));
    mat_table.dielectrics.push(Dielectric::new(1.55));
    mat_table.dielectrics.push(Dielectric::new(1.5));

    let cam = Camera::new(CameraParams {
        lookfrom: Point3::new(13., 2., 3.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 20.0,
        defocus_angle: 0.6,
        focus_dist: 10.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let glass_mat = MaterialInfo {
        kind: MaterialKind::Dielectric,
        index: 0,
    };
    let brown_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 9,
    };
    let metal_mat = MaterialInfo {
        kind: MaterialKind::Metal,
        index: 8,
    };

    let mut world = Scene::new();

    // Ground
    world.push(Instance::sphere(
        Point3::new(0., -1000., 0.),
        1000.,
        ground_mat,
    ));

    // Three main spheres
    world.push(Instance::sphere(Point3::new(0., 1., 0.), 1.0, glass_mat));
    world.push(Instance::sphere(Point3::new(-4., 1., 0.), 1.0, brown_mat));
    world.push(Instance::sphere(Point3::new(4., 1., 0.), 1.0, metal_mat));

    // Small spheres - hardcoded
    let lamb1 = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let lamb2 = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 2,
    };
    let metal0 = MaterialInfo {
        kind: MaterialKind::Metal,
        index: 0,
    };
    let metal1 = MaterialInfo {
        kind: MaterialKind::Metal,
        index: 1,
    };
    let glass1 = MaterialInfo {
        kind: MaterialKind::Dielectric,
        index: 1,
    };

    world.push(Instance::sphere(Point3::new(-3.5, 0.2, 1.5), 0.2, lamb1));
    world.push(Instance::sphere(Point3::new(-2.2, 0.2, 2.8), 0.2, lamb2));
    world.push(Instance::sphere(Point3::new(0.8, 0.2, -1.8), 0.2, metal0));
    world.push(Instance::sphere(Point3::new(2.5, 0.2, -0.5), 0.2, metal1));
    world.push(Instance::sphere(Point3::new(-0.2, 0.2, 0.5), 0.2, glass1));

    (cam, mat_table, tex_table, world)
}

pub fn quads(img_width: usize, img_height: usize) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(1.0, 0.2, 0.2))); // 0: red
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.2, 1.0, 0.2))); // 1: green
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.2, 0.2, 1.0))); // 2: blue
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(1.0, 0.5, 0.0))); // 3: orange
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.2, 0.8, 0.8))); // 4: teal

    let mut mat_table = MaterialTable::new();
    for i in 0..5 {
        mat_table
            .lambertians
            .push(Lambertian::from_texture(TextureInfo {
                kind: TextureKind::Solid,
                index: i,
            }));
    }

    let red_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let green_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let blue_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 2,
    };
    let orange_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 3,
    };
    let teal_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 4,
    };

    let cam = Camera::new(CameraParams {
        lookfrom: Point3::new(0., 0., 9.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 80.0,
        defocus_angle: 0.0,
        focus_dist: 10.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut world = Scene::new();
    // Left wall (red)
    world.push(Instance::quad(
        Point3::new(-3., -2., 5.),
        Vec3::new(0., 0., -4.),
        Vec3::new(0., 4., 0.),
        red_mat,
    ));
    // Back wall (green)
    world.push(Instance::quad(
        Point3::new(-2., -2., 0.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 4., 0.),
        green_mat,
    ));
    // Right wall (blue)
    world.push(Instance::quad(
        Point3::new(3., -2., 1.),
        Vec3::new(0., 0., 4.),
        Vec3::new(0., 4., 0.),
        blue_mat,
    ));
    // Ceiling (orange)
    world.push(Instance::quad(
        Point3::new(-2., 3., 1.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 0., 4.),
        orange_mat,
    ));
    // Floor (teal)
    world.push(Instance::quad(
        Point3::new(-2., -3., 5.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 0., -4.),
        teal_mat,
    ));

    (cam, mat_table, tex_table, world)
}

pub fn metal_test(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.8, 0.3, 0.3))); // 1: red

    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        }));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.8, 0.8, 0.8), 0.1));

    let ground_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let red_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let metal_mat = MaterialInfo {
        kind: MaterialKind::Metal,
        index: 0,
    };

    let cam = Camera::new(CameraParams {
        lookfrom: Point3::new(0., 1., 5.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut world = Scene::new();
    world.push(Instance::sphere(
        Point3::new(0., -100.5, 0.),
        100.,
        ground_mat,
    ));
    world.push(Instance::sphere(Point3::new(-1., 0., 0.), 0.5, red_mat));
    world.push(Instance::sphere(Point3::new(1., 0., 0.), 0.5, metal_mat));

    (cam, mat_table, tex_table, world)
}

pub fn dielectric_test(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.8, 0.3, 0.3))); // 1: red

    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        }));
    mat_table.dielectrics.push(Dielectric::new(1.5));

    let ground_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let red_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let glass_mat = MaterialInfo {
        kind: MaterialKind::Dielectric,
        index: 0,
    };

    let cam = Camera::new(CameraParams {
        lookfrom: Point3::new(0., 1., 5.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut world = Scene::new();
    world.push(Instance::sphere(
        Point3::new(0., -100.5, 0.),
        100.,
        ground_mat,
    ));
    world.push(Instance::sphere(Point3::new(-1., 0., 0.), 0.5, red_mat));
    world.push(Instance::sphere(Point3::new(1., 0., 0.), 0.5, glass_mat));

    (cam, mat_table, tex_table, world)
}

pub fn two_spheres(
    img_width: usize,
    img_height: usize,
    lookfrom: Point3,
    lookat: Point3,
) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.8, 0.3, 0.3))); // 1: red
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.3, 0.3, 0.8))); // 2: blue

    let mut mat_table = MaterialTable::new();
    for i in 0..3 {
        mat_table
            .lambertians
            .push(Lambertian::from_texture(TextureInfo {
                kind: TextureKind::Solid,
                index: i,
            }));
    }

    let ground_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let red_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let blue_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 2,
    };

    let cam = Camera::new(CameraParams {
        lookfrom,
        lookat,
        vup: Vec3::new(0., 1., 0.),
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut world = Scene::new();
    world.push(Instance::sphere(
        Point3::new(0., -100.5, 0.),
        100.,
        ground_mat,
    ));
    world.push(Instance::sphere(Point3::new(-1., 0., 0.), 0.5, red_mat));
    world.push(Instance::sphere(Point3::new(1., 0., 0.), 0.5, blue_mat));

    (cam, mat_table, tex_table, world)
}

pub fn glass_debug(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.3, 0.3, 0.8))); // 0: blue

    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        }));
    mat_table.dielectrics.push(Dielectric::new(1.5));

    let blue_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let glass_mat = MaterialInfo {
        kind: MaterialKind::Dielectric,
        index: 0,
    };

    let cam = Camera::new(CameraParams {
        lookfrom: Point3::new(0., 2., 5.),
        lookat: Point3::new(0., 0.5, 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut world = Scene::new();
    // Large blue floor sphere
    world.push(Instance::sphere(Point3::new(0., -100., 0.), 100., blue_mat));
    // Glass sphere on top
    world.push(Instance::sphere(Point3::new(0., 1., 0.), 1.0, glass_mat));

    (cam, mat_table, tex_table, world)
}

pub fn three_spheres(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, Scene) {
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.4, 0.2, 0.1))); // 1: brown

    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        }));
    mat_table
        .metals
        .push(Metal::new(Color::new(0.7, 0.6, 0.5), 0.0));
    mat_table.dielectrics.push(Dielectric::new(1.5));

    let ground_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 0,
    };
    let brown_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 1,
    };
    let metal_mat = MaterialInfo {
        kind: MaterialKind::Metal,
        index: 0,
    };
    let glass_mat = MaterialInfo {
        kind: MaterialKind::Dielectric,
        index: 0,
    };

    let cam = Camera::new(CameraParams {
        lookfrom: Point3::new(13., 2., 3.),
        lookat: Point3::new(0., 0., 0.),
        vup: Vec3::new(0., 1., 0.),
        fov_v: 20.0,
        defocus_angle: 0.6,
        focus_dist: 10.0,
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
        img_width,
        img_height,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut world = Scene::new();
    world.push(Instance::sphere(
        Point3::new(0., -1000., 0.),
        1000.,
        ground_mat,
    ));
    world.push(Instance::sphere(Point3::new(0., 1., 0.), 1.0, glass_mat));
    world.push(Instance::sphere(Point3::new(-4., 1., 0.), 1.0, brown_mat));
    world.push(Instance::sphere(Point3::new(4., 1., 0.), 1.0, metal_mat));

    (cam, mat_table, tex_table, world)
}
