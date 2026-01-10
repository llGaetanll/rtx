use rtx_mat::Dielectric;
use rtx_mat::DiffuseLight;
use rtx_mat::Lambertian;
use rtx_mat::MaterialInfo;
use rtx_mat::MaterialKind;
use rtx_mat::MaterialTable;
use rtx_mat::Metal;
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

const PX_SAMPLES: u32 = 40;
const MAX_RAY_BOUNCE: u32 = 10;

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
        px_samples: PX_SAMPLES,
        max_ray_bounce: MAX_RAY_BOUNCE,
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

/// The classic final scene from Ray Tracing in One Weekend.
/// Many small spheres scattered on a ground plane with three large spheres.
pub fn many_spheres(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<28, 1>) {
    let mut tex_table = TextureTable::new();
    let mut mat_table = MaterialTable::new();

    // Ground texture/material (solid gray instead of checker)
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
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 2,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 3,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 4,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 5,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 6,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 7,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 8,
        }));

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
        .push(Metal::new(Color::new(0.7, 0.6, 0.5), 0.0)); // polished metal for right main sphere

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

    // Ground
    let ground = Sphere::fixed(Point3::new(0., -1000., 0.), 1000., ground_mat);

    // Three main spheres
    let glass_mat = MaterialInfo {
        kind: MaterialKind::Dielectric,
        index: 0,
    };
    let brown_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 9, // After the 8 small lambertians + ground
    };
    let metal_mat = MaterialInfo {
        kind: MaterialKind::Metal,
        index: 8, // After the 8 small metals
    };

    let center_sphere = Sphere::fixed(Point3::new(0., 1., 0.), 1.0, glass_mat);
    let left_sphere = Sphere::fixed(Point3::new(-4., 1., 0.), 1.0, brown_mat);
    let right_sphere = Sphere::fixed(Point3::new(4., 1., 0.), 1.0, metal_mat);

    // Small spheres arranged in a grid pattern
    // Positions chosen to look good and avoid overlaps
    let small_spheres: [(Point3, usize, MaterialKind); 24] = [
        // Lambertian spheres
        (Point3::new(-3.5, 0.2, 1.5), 1, MaterialKind::Lambertian),
        (Point3::new(-2.2, 0.2, 2.8), 2, MaterialKind::Lambertian),
        (Point3::new(-1.0, 0.2, 1.2), 3, MaterialKind::Lambertian),
        (Point3::new(0.5, 0.2, 3.5), 4, MaterialKind::Lambertian),
        (Point3::new(1.8, 0.2, 2.0), 5, MaterialKind::Lambertian),
        (Point3::new(3.2, 0.2, 1.5), 6, MaterialKind::Lambertian),
        (Point3::new(-2.8, 0.2, -1.2), 7, MaterialKind::Lambertian),
        (Point3::new(-1.5, 0.2, -2.5), 8, MaterialKind::Lambertian),
        // Metal spheres
        (Point3::new(0.8, 0.2, -1.8), 0, MaterialKind::Metal),
        (Point3::new(2.5, 0.2, -0.5), 1, MaterialKind::Metal),
        (Point3::new(-0.5, 0.2, -3.2), 2, MaterialKind::Metal),
        (Point3::new(1.2, 0.2, -3.8), 3, MaterialKind::Metal),
        (Point3::new(3.8, 0.2, -2.2), 4, MaterialKind::Metal),
        (Point3::new(-3.2, 0.2, 3.5), 5, MaterialKind::Metal),
        (Point3::new(2.8, 0.2, 3.2), 6, MaterialKind::Metal),
        (Point3::new(-4.5, 0.2, -0.2), 7, MaterialKind::Metal),
        // Glass spheres
        (Point3::new(-0.2, 0.2, 0.5), 1, MaterialKind::Dielectric),
        (Point3::new(1.5, 0.2, 0.2), 2, MaterialKind::Dielectric),
        (Point3::new(-1.8, 0.2, 0.8), 3, MaterialKind::Dielectric),
        (Point3::new(3.5, 0.2, -3.5), 0, MaterialKind::Dielectric),
        // More scattered
        (Point3::new(-4.2, 0.2, 2.2), 1, MaterialKind::Lambertian),
        (Point3::new(4.5, 0.2, 0.8), 2, MaterialKind::Lambertian),
        (Point3::new(-2.5, 0.2, -3.8), 5, MaterialKind::Metal),
        (Point3::new(0.2, 0.2, 4.2), 6, MaterialKind::Metal),
    ];

    let spheres: [Sphere; 28] = [
        ground,
        center_sphere,
        left_sphere,
        right_sphere,
        Sphere::fixed(
            small_spheres[0].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[0].2,
                index: small_spheres[0].1,
            },
        ),
        Sphere::fixed(
            small_spheres[1].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[1].2,
                index: small_spheres[1].1,
            },
        ),
        Sphere::fixed(
            small_spheres[2].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[2].2,
                index: small_spheres[2].1,
            },
        ),
        Sphere::fixed(
            small_spheres[3].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[3].2,
                index: small_spheres[3].1,
            },
        ),
        Sphere::fixed(
            small_spheres[4].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[4].2,
                index: small_spheres[4].1,
            },
        ),
        Sphere::fixed(
            small_spheres[5].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[5].2,
                index: small_spheres[5].1,
            },
        ),
        Sphere::fixed(
            small_spheres[6].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[6].2,
                index: small_spheres[6].1,
            },
        ),
        Sphere::fixed(
            small_spheres[7].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[7].2,
                index: small_spheres[7].1,
            },
        ),
        Sphere::fixed(
            small_spheres[8].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[8].2,
                index: small_spheres[8].1,
            },
        ),
        Sphere::fixed(
            small_spheres[9].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[9].2,
                index: small_spheres[9].1,
            },
        ),
        Sphere::fixed(
            small_spheres[10].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[10].2,
                index: small_spheres[10].1,
            },
        ),
        Sphere::fixed(
            small_spheres[11].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[11].2,
                index: small_spheres[11].1,
            },
        ),
        Sphere::fixed(
            small_spheres[12].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[12].2,
                index: small_spheres[12].1,
            },
        ),
        Sphere::fixed(
            small_spheres[13].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[13].2,
                index: small_spheres[13].1,
            },
        ),
        Sphere::fixed(
            small_spheres[14].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[14].2,
                index: small_spheres[14].1,
            },
        ),
        Sphere::fixed(
            small_spheres[15].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[15].2,
                index: small_spheres[15].1,
            },
        ),
        Sphere::fixed(
            small_spheres[16].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[16].2,
                index: small_spheres[16].1,
            },
        ),
        Sphere::fixed(
            small_spheres[17].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[17].2,
                index: small_spheres[17].1,
            },
        ),
        Sphere::fixed(
            small_spheres[18].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[18].2,
                index: small_spheres[18].1,
            },
        ),
        Sphere::fixed(
            small_spheres[19].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[19].2,
                index: small_spheres[19].1,
            },
        ),
        Sphere::fixed(
            small_spheres[20].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[20].2,
                index: small_spheres[20].1,
            },
        ),
        Sphere::fixed(
            small_spheres[21].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[21].2,
                index: small_spheres[21].1,
            },
        ),
        Sphere::fixed(
            small_spheres[22].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[22].2,
                index: small_spheres[22].1,
            },
        ),
        Sphere::fixed(
            small_spheres[23].0,
            0.2,
            MaterialInfo {
                kind: small_spheres[23].2,
                index: small_spheres[23].1,
            },
        ),
    ];

    // Dummy quad
    let dummy_quad = Quad::new(
        Point3::new(0., -1000., -1000.),
        Vec3::new(0.001, 0., 0.),
        Vec3::new(0., 0.001, 0.),
        ground_mat,
    );

    let world = List::from_objects(spheres, [dummy_quad]);

    (cam, mat_table, tex_table, world)
}

/// Five colored quads arranged in a room-like configuration.
pub fn quads(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<1, 5>) {
    // Textures: red, green, blue, orange, teal
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
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 5: dummy

    // Materials
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
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 2,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 3,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 4,
        }));
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 5,
        }));

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
    let dummy_mat = MaterialInfo {
        kind: MaterialKind::Lambertian,
        index: 5,
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

    // Left wall (red)
    let left = Quad::new(
        Point3::new(-3., -2., 5.),
        Vec3::new(0., 0., -4.),
        Vec3::new(0., 4., 0.),
        red_mat,
    );
    // Back wall (green)
    let back = Quad::new(
        Point3::new(-2., -2., 0.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 4., 0.),
        green_mat,
    );
    // Right wall (blue)
    let right = Quad::new(
        Point3::new(3., -2., 1.),
        Vec3::new(0., 0., 4.),
        Vec3::new(0., 4., 0.),
        blue_mat,
    );
    // Ceiling (orange)
    let ceiling = Quad::new(
        Point3::new(-2., 3., 1.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 0., 4.),
        orange_mat,
    );
    // Floor (teal)
    let floor = Quad::new(
        Point3::new(-2., -3., 5.),
        Vec3::new(4., 0., 0.),
        Vec3::new(0., 0., -4.),
        teal_mat,
    );

    let dummy_sphere = Sphere::fixed(Point3::new(0., 0., -1000.), 0.001, dummy_mat);

    let world = List::from_objects([dummy_sphere], [left, back, right, ceiling, floor]);

    (cam, mat_table, tex_table, world)
}

/// Test scene: one lambertian + one metal sphere.
pub fn metal_test(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<3, 1>) {
    // Textures
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground (gray)
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.8, 0.3, 0.3))); // 1: red sphere

    // Materials
    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        })); // 0: ground
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        })); // 1: red
    mat_table
        .metals
        .push(Metal::new(Color::new(0.8, 0.8, 0.8), 0.1)); // 0: silver metal

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

    // Ground sphere
    let ground = Sphere::fixed(Point3::new(0., -100.5, 0.), 100., ground_mat);

    // Two spheres: lambertian left, metal right
    let red_sphere = Sphere::fixed(Point3::new(-1., 0., 0.), 0.5, red_mat);
    let metal_sphere = Sphere::fixed(Point3::new(1., 0., 0.), 0.5, metal_mat);

    // Dummy quad
    let dummy_quad = Quad::new(
        Point3::new(0., -1000., -1000.),
        Vec3::new(0.001, 0., 0.),
        Vec3::new(0., 0.001, 0.),
        ground_mat,
    );

    let world = List::from_objects([ground, red_sphere, metal_sphere], [dummy_quad]);

    (cam, mat_table, tex_table, world)
}

/// Test scene: one lambertian + one dielectric (glass) sphere.
pub fn dielectric_test(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<3, 1>) {
    // Textures
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground (gray)
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.8, 0.3, 0.3))); // 1: red sphere

    // Materials
    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        })); // 0: ground
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        })); // 1: red
    mat_table.dielectrics.push(Dielectric::new(1.5)); // 0: glass

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

    // Ground sphere
    let ground = Sphere::fixed(Point3::new(0., -100.5, 0.), 100., ground_mat);

    // Two spheres: lambertian left, glass right
    let red_sphere = Sphere::fixed(Point3::new(-1., 0., 0.), 0.5, red_mat);
    let glass_sphere = Sphere::fixed(Point3::new(1., 0., 0.), 0.5, glass_mat);

    // Dummy quad
    let dummy_quad = Quad::new(
        Point3::new(0., -1000., -1000.),
        Vec3::new(0.001, 0., 0.),
        Vec3::new(0., 0.001, 0.),
        ground_mat,
    );

    let world = List::from_objects([ground, red_sphere, glass_sphere], [dummy_quad]);

    (cam, mat_table, tex_table, world)
}

/// Two lambertian spheres - minimal test scene.
pub fn two_spheres(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<3, 1>) {
    // Textures
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground (gray)
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.8, 0.3, 0.3))); // 1: red sphere
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.3, 0.3, 0.8))); // 2: blue sphere

    // Materials - all lambertian
    let mut mat_table = MaterialTable::new();
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        })); // 0: ground
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        })); // 1: red
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 2,
        })); // 2: blue

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

    // Ground sphere
    let ground = Sphere::fixed(Point3::new(0., -100.5, 0.), 100., ground_mat);

    // Two spheres side by side
    let red_sphere = Sphere::fixed(Point3::new(-1., 0., 0.), 0.5, red_mat);
    let blue_sphere = Sphere::fixed(Point3::new(1., 0., 0.), 0.5, blue_mat);

    // Dummy quad (required since rust-gpu can't handle empty arrays)
    let dummy_quad = Quad::new(
        Point3::new(0., -1000., -1000.),
        Vec3::new(0.001, 0., 0.),
        Vec3::new(0., 0.001, 0.),
        ground_mat,
    );

    let world = List::from_objects([ground, red_sphere, blue_sphere], [dummy_quad]);

    (cam, mat_table, tex_table, world)
}

/// Three spheres: glass, lambertian, and metal on a ground plane.
pub fn three_spheres(
    img_width: usize,
    img_height: usize,
) -> (Camera, MaterialTable, TextureTable, List<5, 1>) {
    // Textures
    let mut tex_table = TextureTable::new();
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))); // 0: ground
    tex_table
        .solids
        .push(SolidTexture::from_color(Color::new(0.4, 0.2, 0.1))); // 1: brown

    // Materials
    let mut mat_table = MaterialTable::new();

    // Lambertians
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 0,
        })); // 0: ground
    mat_table
        .lambertians
        .push(Lambertian::from_texture(TextureInfo {
            kind: TextureKind::Solid,
            index: 1,
        })); // 1: brown sphere

    // Metal
    mat_table
        .metals
        .push(Metal::new(Color::new(0.7, 0.6, 0.5), 0.0)); // 0: polished metal

    // Dielectric (glass)
    mat_table.dielectrics.push(Dielectric::new(1.5)); // 0: glass

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

    // Ground sphere
    let ground = Sphere::fixed(Point3::new(0., -1000., 0.), 1000., ground_mat);

    // Three main spheres
    let glass_sphere = Sphere::fixed(Point3::new(0., 1., 0.), 1.0, glass_mat);
    let brown_sphere = Sphere::fixed(Point3::new(-4., 1., 0.), 1.0, brown_mat);
    let metal_sphere = Sphere::fixed(Point3::new(4., 1., 0.), 1.0, metal_mat);

    // Dummy quad
    let dummy_quad = Quad::new(
        Point3::new(0., -1000., -1000.),
        Vec3::new(0.001, 0., 0.),
        Vec3::new(0., 0.001, 0.),
        ground_mat,
    );

    let world = List::from_objects(
        [
            ground,
            glass_sphere,
            brown_sphere,
            metal_sphere,
            Sphere::fixed(Point3::new(0., 0., -1000.), 0.001, ground_mat),
        ],
        [dummy_quad],
    );

    (cam, mat_table, tex_table, world)
}
