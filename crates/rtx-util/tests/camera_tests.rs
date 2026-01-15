use rtx_prim::Color;
use rtx_prim::Vec3;
use rtx_util::Camera;
use rtx_util::CameraParams;

/// Demonstrates gimbal lock: looking straight down with vup=(0,1,0) produces NaN rays
#[test]
#[should_panic(expected = "Ray direction x is NaN")]
fn test_camera_gimbal_lock_looking_down() {
    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let lookat = Vec3::new(0.0, 0.0, 0.0); // Looking straight down

    let cam = Camera::new(CameraParams {
        lookfrom: cam_pos,
        lookat,
        vup: Vec3::new(0., 1., 0.), // Parallel to view direction - causes gimbal lock
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: 1,
        max_ray_bounce: 10,
        img_width: 100,
        img_height: 100,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut state = 42u32;
    let ray = cam.get_ray(&mut state, 50, 50);

    println!("Ray origin: {:?}", ray.orig());
    println!("Ray direction: {:?}", ray.dir());

    assert!(!ray.dir().x.is_nan(), "Ray direction x is NaN");
    assert!(!ray.dir().y.is_nan(), "Ray direction y is NaN");
    assert!(!ray.dir().z.is_nan(), "Ray direction z is NaN");
}

/// Demonstrates gimbal lock: looking straight up with vup=(0,1,0) produces NaN rays
#[test]
#[should_panic(expected = "Ray direction x is NaN")]
fn test_camera_gimbal_lock_looking_up() {
    let cam_pos = Vec3::new(0.0, 0.0, 0.0);
    let lookat = Vec3::new(0.0, 10.0, 0.0); // Looking straight up

    let cam = Camera::new(CameraParams {
        lookfrom: cam_pos,
        lookat,
        vup: Vec3::new(0., 1., 0.), // Parallel to view direction - causes gimbal lock
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: 1,
        max_ray_bounce: 10,
        img_width: 100,
        img_height: 100,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut state = 42u32;
    let ray = cam.get_ray(&mut state, 50, 50);

    println!("Ray origin: {:?}", ray.orig());
    println!("Ray direction: {:?}", ray.dir());

    assert!(!ray.dir().x.is_nan(), "Ray direction x is NaN");
    assert!(!ray.dir().y.is_nan(), "Ray direction y is NaN");
    assert!(!ray.dir().z.is_nan(), "Ray direction z is NaN");
}

/// Fix for gimbal lock: use a different vup when looking down
#[test]
fn test_camera_looking_down_with_correct_vup() {
    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let lookat = Vec3::new(0.0, 0.0, 0.0); // Looking straight down

    let cam = Camera::new(CameraParams {
        lookfrom: cam_pos,
        lookat,
        vup: Vec3::new(0., 0., 1.), // Use Z-axis as up when looking down
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: 1,
        max_ray_bounce: 10,
        img_width: 100,
        img_height: 100,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut state = 42u32;
    let ray = cam.get_ray(&mut state, 50, 50);

    println!("Ray origin: {:?}", ray.orig());
    println!("Ray direction: {:?}", ray.dir());

    assert!(!ray.dir().x.is_nan(), "Ray direction x is NaN");
    assert!(!ray.dir().y.is_nan(), "Ray direction y is NaN");
    assert!(!ray.dir().z.is_nan(), "Ray direction z is NaN");
}

/// Fix for gimbal lock: use a different vup when looking up
#[test]
fn test_camera_looking_up_with_correct_vup() {
    let cam_pos = Vec3::new(0.0, 0.0, 0.0);
    let lookat = Vec3::new(0.0, 10.0, 0.0); // Looking straight up

    let cam = Camera::new(CameraParams {
        lookfrom: cam_pos,
        lookat,
        vup: Vec3::new(0., 0., -1.), // Use -Z-axis as up when looking up
        fov_v: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
        px_samples: 1,
        max_ray_bounce: 10,
        img_width: 100,
        img_height: 100,
        background: Color::new(0.7, 0.8, 1.0),
    });

    let mut state = 42u32;
    let ray = cam.get_ray(&mut state, 50, 50);

    println!("Ray origin: {:?}", ray.orig());
    println!("Ray direction: {:?}", ray.dir());

    assert!(!ray.dir().x.is_nan(), "Ray direction x is NaN");
    assert!(!ray.dir().y.is_nan(), "Ray direction y is NaN");
    assert!(!ray.dir().z.is_nan(), "Ray direction z is NaN");
}
