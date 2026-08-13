//! Light sampling is the one part of the renderer whose bugs are invisible in an
//! image: a wrong density still produces a plausible picture, just the wrong one,
//! converging confidently to something that is not the answer. So it is checked
//! here numerically rather than by eye.

use rtx_mat::MaterialInfo;
use rtx_obj::Light;
use rtx_obj::Lights;
use rtx_obj::light_kind;
use rtx_prim::Point3;
use rtx_prim::RandState;
use rtx_prim::Vec3;

/// The Cornell box ceiling panel, which is the light every one of these tests is
/// really about. Its edges are ordered so it faces down, into the room.
fn cornell_light() -> Light {
    Light::quad(
        Point3::new(343.0, 784.7828, 332.0),
        Vec3::new(-130.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -105.0),
        MaterialInfo::diffuse_light(0),
    )
}

/// A unit quad in the XY plane at the origin, facing +Z.
fn unit_quad() -> Light {
    Light::quad(
        Point3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        MaterialInfo::diffuse_light(0),
    )
}

#[test]
fn quad_light_records_its_area_and_facing() {
    let light = cornell_light();

    assert_eq!(light.kind, light_kind::QUAD);
    assert_eq!(light.area, 130.0 * 105.0);

    // The panel is on the ceiling, so the face that emits looks down at the room
    assert_eq!(light.norm(), Vec3::new(0.0, -1.0, 0.0));

    let material = light.material();
    assert_eq!(material.kind, MaterialInfo::diffuse_light(0).kind);
    assert_eq!(material.index, 0);
}

/// The solid angle a triangle subtends at the origin, by Van Oosterom and
/// Strackee. Independent of anything the renderer computes, which is the point:
/// it gives the sampler something external to be wrong against.
fn triangle_solid_angle(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (la, lb, lc) = (a.length() as f64, b.length() as f64, c.length() as f64);

    let numerator = a.dot(b.cross(c)) as f64;
    let denominator =
        la * lb * lc + a.dot(b) as f64 * lc + a.dot(c) as f64 * lb + b.dot(c) as f64 * la;

    2.0 * numerator.atan2(denominator)
}

/// The solid angle the whole quad subtends at `from`, as its two triangles.
fn quad_solid_angle(light: &Light, from: Point3) -> f64 {
    let q = Vec3::new(light.q[0], light.q[1], light.q[2]) - from;
    let u = Vec3::new(light.u[0], light.u[1], light.u[2]);
    let v = Vec3::new(light.v[0], light.v[1], light.v[2]);

    (triangle_solid_angle(q, q + u, q + u + v) + triangle_solid_angle(q, q + u + v, q + v)).abs()
}

/// The area to solid angle Jacobian is the easiest thing here to get wrong, and a
/// wrong one is a systematically too bright or too dark image rather than a noisy
/// one.
///
/// `mean(1 / pdf)` over points sampled uniformly on the light is exactly the solid
/// angle the light subtends, so comparing it against the analytic solid angle
/// pins the density to something the renderer had no hand in computing. A missing
/// `dist^2` or `cos` fails this; checking the density against its own formula
/// would not.
#[test]
fn quad_pdf_matches_the_solid_angle_the_light_subtends() {
    let light = cornell_light();
    let lights = [light];
    let lights = Lights::new(&lights);

    // Three vantage points in the room: straight below the panel, off to one
    // side, and near a corner where it is seen at a slant
    let viewpoints = [
        Point3::new(277.5, 100.0, 277.5),
        Point3::new(100.0, 300.0, 200.0),
        Point3::new(500.0, 50.0, 500.0),
    ];

    for (i, from) in viewpoints.iter().enumerate() {
        let mut state: RandState = 0x9e3779b9u32.wrapping_add((i as u32).wrapping_mul(2654435761));

        let samples = 400_000;
        let mut total = 0.0f64;

        for _ in 0..samples {
            let sample = lights.sample(&mut state, *from);
            assert!(sample.pdf > 0.0, "the panel is fully visible from {from:?}");

            total += 1.0 / sample.pdf as f64;
        }

        let estimated = total / samples as f64;
        let analytic = quad_solid_angle(&light, *from);

        assert!(
            (estimated - analytic).abs() < analytic * 2e-3,
            "from {from:?} the sampler implies a solid angle of {estimated}, \
             but the panel subtends {analytic}"
        );
    }
}

/// `sample` and `pdf` are two descriptions of one distribution, and MIS weighs
/// them against each other. If they disagree the weights stop summing to one and
/// the image is wrong in a way no render will point at.
#[test]
fn pdf_agrees_with_the_density_sample_produced() {
    let lights = [cornell_light()];
    let lights = Lights::new(&lights);

    let from = Point3::new(277.5, 200.0, 277.5);
    let mut state: RandState = 12345;

    for _ in 0..10_000 {
        let sample = lights.sample(&mut state, from);

        let recovered = lights.pdf(sample.index, from, sample.dir, sample.dist);

        assert!(
            (recovered - sample.pdf).abs() <= sample.pdf * 1e-5,
            "pdf() said {recovered}, sample said {}",
            sample.pdf
        );
    }
}

/// Picking one light out of several has to be paid for in the density, or every
/// estimate is too bright by exactly the number of lights.
#[test]
fn selection_probability_is_included_once() {
    let one = [unit_quad()];
    let four = [unit_quad(), unit_quad(), unit_quad(), unit_quad()];

    let from = Point3::new(0.5, 0.5, 2.0);

    // The same light four times over, so any direction has the same geometry and
    // only the one-in-four choice can differ
    let single = Lights::new(&one).pdf(0, from, Vec3::new(0.0, 0.0, -1.0), 2.0);
    let quadruple = Lights::new(&four).pdf(2, from, Vec3::new(0.0, 0.0, -1.0), 2.0);

    assert!((single - 4.0 * quadruple).abs() < 1e-6);

    let mut state: RandState = 7;
    let sampled = Lights::new(&four).sample(&mut state, from);
    let direct = Lights::new(&one).pdf(0, from, sampled.dir, sampled.dist);

    assert!((direct - 4.0 * sampled.pdf).abs() < direct * 1e-5);
}

/// A point behind the emitting face sees nothing. The pdf has to say so rather
/// than return a density for light that never arrives.
#[test]
fn a_point_behind_the_light_gets_no_sample() {
    let lights = [unit_quad()];
    let lights = Lights::new(&lights);

    let mut state: RandState = 99;

    // The quad faces +Z, so this is the dark side
    let behind = Point3::new(0.5, 0.5, -2.0);

    for _ in 0..1000 {
        assert_eq!(lights.sample(&mut state, behind).pdf, 0.0);
    }

    // And level with the plane, where the density would otherwise blow up
    let edge_on = Vec3::new(1.0, 0.0, 0.0);
    assert_eq!(lights.pdf(0, behind, edge_on, 2.0), 0.0);
}

/// A scene with no emitters must not need a special case at the call site.
#[test]
fn an_empty_light_list_samples_to_nothing() {
    let lights = Lights::new(&[]);
    let mut state: RandState = 1;

    assert!(lights.is_empty());
    assert_eq!(lights.sample(&mut state, Point3::ZERO).pdf, 0.0);
    assert_eq!(lights.pdf(0, Point3::ZERO, Vec3::Z, 1.0), 0.0);
}
