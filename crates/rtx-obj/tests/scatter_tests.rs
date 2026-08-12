use std::f32::consts::FRAC_1_SQRT_2;

use rtx_mat::Dielectric;
use rtx_mat::HitRecord;
use rtx_mat::Lambertian;
use rtx_mat::Material;
use rtx_mat::Metal;
use rtx_obj::Instance;
use rtx_obj::hit_unit_sphere;
use rtx_obj::transform_hit_to_world;
use rtx_obj::transform_ray_to_object;
use rtx_prim::Color;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::rand;
use rtx_tex::SolidTexture;
use rtx_tex::TextureInfo;
use rtx_tex::TextureTable;

/// Helper to perform a hit test on an instance and return the hit record.
fn hit_instance(instance: &Instance, ray: &Ray) -> Option<HitRecord> {
    let obj_ray = transform_ray_to_object(ray, &instance.inv_transform);

    let mut rec = HitRecord::default();
    let mut t_range = Range::new(0.001, f32::MAX);

    if hit_unit_sphere(&obj_ray, &mut t_range, &mut rec) {
        transform_hit_to_world(&mut rec, &instance.inv_transform, ray);
        Some(rec)
    } else {
        None
    }
}

/// Helper to check if two vectors are approximately equal.
fn approx_eq(a: Vec3, b: Vec3, epsilon: f32) -> bool {
    (a - b).length() < epsilon
}

// =============================================================================
// Metal Tests
// =============================================================================

#[test]
fn test_metal_reflection_45_degrees() {
    // Ray coming in at 45 degrees, hitting sphere at front center
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    // Origin at (-1, 0, -2), aiming to hit front of sphere at (0, 0, -1)
    let ray_origin = Vec3::new(-1.0, 0.0, -2.0);
    let hit_point = Vec3::new(0.0, 0.0, -1.0);
    let ray_dir = (hit_point - ray_origin).normalize();
    let ray = Ray::new(ray_origin, ray_dir, 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(0.0, 0.0, -1.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let metal = Metal::new(Color::new(1.0, 1.0, 1.0), 0.0);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = rand::init_state();
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    let did_scatter = metal.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    assert!(did_scatter, "Metal should scatter");

    // Perfect reflection: incoming (0.707, 0, 0.707) off normal (0, 0, -1) -> (0.707, 0, -0.707)
    let reflected = scattered.dir().normalize();
    let expected = Vec3::new(FRAC_1_SQRT_2, 0.0, -FRAC_1_SQRT_2);
    assert!(
        approx_eq(reflected, expected, 0.0001),
        "Reflected: expected {expected:?}, got {reflected:?}"
    );
}

#[test]
fn test_metal_reflection_grazing_angle() {
    // Ray hitting sphere from the side
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    let ray_origin = Vec3::new(-2.0, 0.0, 0.0);
    let ray_dir = Vec3::new(1.0, 0.0, 0.0);
    let ray = Ray::new(ray_origin, ray_dir, 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(-1.0, 0.0, 0.0);
    let expected_norm = Vec3::new(-1.0, 0.0, 0.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let metal = Metal::new(Color::new(1.0, 1.0, 1.0), 0.0);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = rand::init_state();
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    let did_scatter = metal.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    assert!(did_scatter, "Metal should scatter");

    // Perfect reflection: incoming (1, 0, 0) off normal (-1, 0, 0) -> (-1, 0, 0)
    let reflected = scattered.dir().normalize();
    let expected = Vec3::new(-1.0, 0.0, 0.0);
    assert!(
        approx_eq(reflected, expected, 0.0001),
        "Reflected: expected {expected:?}, got {reflected:?}"
    );
}

#[test]
fn test_metal_fuzz_affects_direction() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(0.0, 0.0, -1.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    // Scatter with fuzz = 0
    let metal_smooth = Metal::new(Color::new(1.0, 1.0, 1.0), 0.0);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state1 = rand::init_state();
    let mut scattered1 = Ray::default();
    let mut attenuation1 = Color::default();

    metal_smooth.scatter(
        &mut state1,
        &tex_table,
        &ray,
        &rec,
        &mut scattered1,
        &mut attenuation1,
    );

    // Scatter with fuzz = 0.5
    let metal_fuzzy = Metal::new(Color::new(1.0, 1.0, 1.0), 0.5);
    let mut state2 = rand::init_state();
    let mut scattered2 = Ray::default();
    let mut attenuation2 = Color::default();

    metal_fuzzy.scatter(
        &mut state2,
        &tex_table,
        &ray,
        &rec,
        &mut scattered2,
        &mut attenuation2,
    );

    let dir1 = scattered1.dir().normalize();
    let dir2 = scattered2.dir().normalize();

    // Smooth reflection should be exactly (0, 0, -1)
    let expected_smooth = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(dir1, expected_smooth, 0.0001),
        "Smooth reflection: expected {expected_smooth:?}, got {dir1:?}"
    );

    // Fuzzy reflection (deterministic with seed 42)
    let expected_fuzzy = Vec3::new(0.4912008, 0.008160583, -0.8710082);
    assert!(
        approx_eq(dir2, expected_fuzzy, 0.0001),
        "Fuzzy reflection: expected {expected_fuzzy:?}, got {dir2:?}"
    );
}

// =============================================================================
// Dielectric Tests
// =============================================================================

#[test]
fn test_dielectric_refraction_entering_glass() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    // Ray coming at 45 degrees
    let ray_origin = Vec3::new(-1.0, 1.0, -2.0);
    let hit_point = Vec3::new(0.0, 0.0, -1.0);
    let ray_dir = (hit_point - ray_origin).normalize();
    let ray = Ray::new(ray_origin, ray_dir, 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(0.0, 0.0, -1.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        rec.front_face,
        "Should be entering the material (front_face = true)"
    );
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let glass = Dielectric::new(1.5);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = 12345u32;
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    let did_scatter = glass.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    assert!(did_scatter, "Dielectric should always scatter");

    let refracted = scattered.dir().normalize();
    let expected = Vec3::new(0.38490024, -0.38490024, 0.83887047);
    assert!(
        approx_eq(refracted, expected, 0.0001),
        "Refracted: expected {expected:?}, got {refracted:?}"
    );
}

#[test]
fn test_dielectric_refraction_exiting_glass() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    // Ray starting inside the sphere, going outward
    let ray_origin = Vec3::new(0.0, 0.0, 0.0);
    let ray_dir = Vec3::new(0.3, 0.0, 1.0).normalize();
    let ray = Ray::new(ray_origin, ray_dir, 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere from inside");

    assert!(
        !rec.front_face,
        "Should be exiting the material (front_face = false)"
    );

    let expected_p = Vec3::new(0.2873479, 0.0, 0.9578263);
    let expected_norm = Vec3::new(-0.2873479, 0.0, -0.9578263);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let glass = Dielectric::new(1.5);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = 12345u32;
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    let did_scatter = glass.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    assert!(did_scatter, "Dielectric should always scatter");

    let refracted = scattered.dir().normalize();
    let expected = Vec3::new(0.28734788, 0.0, 0.95782626);
    assert!(
        approx_eq(refracted, expected, 0.0001),
        "Refracted: expected {expected:?}, got {refracted:?}"
    );
}

#[test]
fn test_dielectric_total_internal_reflection() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    // Start off-center inside the sphere, shoot sideways for steep angle
    let ray_origin = Vec3::new(0.0, 0.0, 0.5);
    let ray_dir = Vec3::new(1.0, 0.0, 0.0);
    let ray = Ray::new(ray_origin, ray_dir, 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere from inside");

    assert!(!rec.front_face, "Should be exiting (front_face = false)");

    let expected_p = Vec3::new(0.8660254, 0.0, 0.5);
    let expected_norm = Vec3::new(-0.8660254, 0.0, -0.5);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let glass = Dielectric::new(1.5);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = 12345u32;
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    glass.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    // At this angle, TIR should occur
    let reflected = scattered.dir().normalize();
    let expected = Vec3::new(0.947822, 0.0, -0.31880012);
    assert!(
        approx_eq(reflected, expected, 0.0001),
        "TIR reflected: expected {expected:?}, got {reflected:?}"
    );
}

#[test]
fn test_dielectric_normal_incidence() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(0.0, 0.0, -1.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let glass = Dielectric::new(1.5);
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = 12345u32;
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    glass.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    // At normal incidence, ray goes straight through
    let dir = scattered.dir().normalize();
    let expected = Vec3::new(0.0, 0.0, 1.0);
    assert!(
        approx_eq(dir, expected, 0.0001),
        "Normal incidence: expected {expected:?}, got {dir:?}"
    );
}

// =============================================================================
// Lambertian Tests
// =============================================================================

#[test]
fn test_lambertian_scatters_in_hemisphere() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(0.0, 0.0, -1.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let lambertian = Lambertian::from_texture(TextureInfo::solid(0));
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = rand::init_state();
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    lambertian.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    let dir = scattered.dir().normalize();
    let expected = Vec3::new(0.81248367, 0.013498228, -0.5828276);
    assert!(
        approx_eq(dir, expected, 0.0001),
        "Lambertian scatter: expected {expected:?}, got {dir:?}"
    );
}

#[test]
fn test_lambertian_deterministic_with_seed() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let lambertian = Lambertian::from_texture(TextureInfo::solid(0));
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };

    // First scatter with seed 42
    let mut state1 = 42u32;
    let mut scattered1 = Ray::default();
    let mut attenuation1 = Color::default();

    lambertian.scatter(
        &mut state1,
        &tex_table,
        &ray,
        &rec,
        &mut scattered1,
        &mut attenuation1,
    );

    // Second scatter with same seed
    let mut state2 = 42u32;
    let mut scattered2 = Ray::default();
    let mut attenuation2 = Color::default();

    lambertian.scatter(
        &mut state2,
        &tex_table,
        &ray,
        &rec,
        &mut scattered2,
        &mut attenuation2,
    );

    let dir1 = scattered1.dir().normalize();
    let dir2 = scattered2.dir().normalize();

    // Both should be identical
    assert!(
        approx_eq(dir1, dir2, 0.0001),
        "Same seed should produce same result: {dir1:?} vs {dir2:?}"
    );

    // And should match expected value
    let expected = Vec3::new(0.81248367, 0.013498228, -0.5828276);
    assert!(
        approx_eq(dir1, expected, 0.0001),
        "Lambertian with seed 42: expected {expected:?}, got {dir1:?}"
    );
}

#[test]
fn test_lambertian_grazing_angle() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    // Ray hitting at grazing angle
    let ray_origin = Vec3::new(0.0, 0.0, -2.0);
    let hit_point = Vec3::new(0.99, 0.0, -0.141);
    let ray_dir = (hit_point - ray_origin).normalize();
    let ray = Ray::new(ray_origin, ray_dir, 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(0.66951627, 0.0, -0.74279743);
    let expected_norm = Vec3::new(0.66951627, 0.0, -0.74279743);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );

    let lambertian = Lambertian::from_texture(TextureInfo::solid(0));
    let solids = [SolidTexture::from_color(Color::new(0.5, 0.5, 0.5))];
    let tex_table = TextureTable { solids: &solids };
    let mut state = rand::init_state();
    let mut scattered = Ray::default();
    let mut attenuation = Color::default();

    lambertian.scatter(
        &mut state,
        &tex_table,
        &ray,
        &rec,
        &mut scattered,
        &mut attenuation,
    );

    let dir = scattered.dir().normalize();
    let expected = Vec3::new(0.96750796, 0.009416749, -0.252665);
    assert!(
        approx_eq(dir, expected, 0.0001),
        "Lambertian grazing: expected {expected:?}, got {dir:?}"
    );
}

// =============================================================================
// Edge Cases / Regression Tests
// =============================================================================

#[test]
fn test_transformed_sphere_normals() {
    // Sphere translated away from origin
    let instance = Instance::sphere(Vec3::new(5.0, 3.0, -10.0), 1.0, Default::default());

    let ray = Ray::new(Vec3::new(5.0, 3.0, -15.0), Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere");

    let expected_p = Vec3::new(5.0, 3.0, -11.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );
}

#[test]
fn test_scaled_sphere_hit_and_normals() {
    // Sphere with radius 2
    let instance = Instance::sphere(Vec3::ZERO, 2.0, Default::default());

    let ray = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit scaled sphere");

    let expected_p = Vec3::new(0.0, 0.0, -2.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0);
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );
}

#[test]
fn test_ray_from_inside_sphere() {
    let instance = Instance::sphere(Vec3::ZERO, 1.0, Default::default());

    let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 0.0);

    let rec = hit_instance(&instance, &ray).expect("Ray should hit sphere from inside");

    assert!(
        !rec.front_face,
        "front_face should be false when hitting from inside"
    );

    let expected_p = Vec3::new(0.0, 0.0, 1.0);
    let expected_norm = Vec3::new(0.0, 0.0, -1.0); // Flipped to face ray
    assert!(
        approx_eq(rec.p, expected_p, 0.0001),
        "Hit point: expected {:?}, got {:?}",
        expected_p,
        rec.p
    );
    assert!(
        approx_eq(rec.norm, expected_norm, 0.0001),
        "Normal: expected {:?}, got {:?}",
        expected_norm,
        rec.norm
    );
}
