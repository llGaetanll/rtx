//! A ray leaving a surface must not find that surface again.
//!
//! The hit point handed to the next ray is `orig + t * dir` in `f32`, so it lands
//! a little off the surface it is supposed to be on, either side. If the offset
//! the next ray starts with is smaller than that error, some directions find the
//! surface again at a hit a hair away, the path stops there, and the surface comes
//! out darker than it is.
//!
//! What made this worth a test of its own is how it failed. The Cornell box is
//! measured in hundreds of units, and an absolute offset that is ample near the
//! origin is nothing at all a thousand units out. Nothing about the picture said
//! "epsilon": it showed up as vertical bands down the tall box, which look far more
//! like a bad random number generator than like arithmetic, and swapping the
//! generator changed nothing about them. So these check the offset against the
//! rounding it exists to clear, at the distances a scene actually uses, rather
//! than checking a rendered image for bands.

use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_mat::MaterialInfo;
use rtx_obj::Instance;
use rtx_obj::Scene;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_util::ray_epsilon;
use rtx_util::ray_epsilon_at;

/// How far the wall is turned out of the axes, in radians. The same fifteen
/// degrees the Cornell box turns its tall box by, and the reason the bands were on
/// that box rather than on the walls.
const TILT: F = 0.261_799_4;

/// Directions spread over the hemisphere about `norm`, as a scattered ray would
/// be. A fixed spiral rather than random ones, so a failure names the same
/// direction every time it is looked at.
fn hemisphere(norm: Vec3, count: usize) -> Vec<Vec3> {
    let (u, v) = norm.any_orthonormal_pair();

    (0..count)
        .map(|i| {
            let t = (i as F + 0.5) / count as F;
            let cos_theta = t;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            // An irrational turn per step, so the directions do not line up into
            // a few planes the way a rational one would
            let phi = i as F * 2.399_963_2;

            (u * (sin_theta * phi.cos()) + v * (sin_theta * phi.sin()) + norm * cos_theta)
                .normalize()
        })
        .collect()
}

/// A quad the size of a Cornell box wall, placed `dist` from the origin along z.
///
/// Two things about it matter. The distance, because the same quad near the origin
/// passes with any epsilon at all and it is only once the coordinates are large
/// enough for `f32` to round them that an absolute offset stops being one. And the
/// tilt, because an axis aligned face is the case the arithmetic happens to get
/// exactly right: the plane it solves against is one coordinate, so the hit point
/// lands on it to the bit. The tall box in the Cornell box is turned fifteen
/// degrees, which is why the bands were on it and not on the walls behind it.
fn wall_at(dist: F) -> Vec<Instance> {
    let (c, s) = (TILT.cos(), TILT.sin());

    vec![Instance::quad(
        Point3::new(0.0, 0.0, dist),
        Vec3::new(555.0 * c, 0.0, 555.0 * s),
        Vec3::new(0.0, 555.0, 0.0),
        MaterialInfo::default(),
    )]
}

/// Cast a ray off every point of a grid on the quad, in every direction of the
/// hemisphere, and count the ones that find the quad they started on.
fn self_hits(dist: F, grid: usize, dirs: usize) -> (usize, usize) {
    let mut instances = wall_at(dist);
    let bvh = rtx_obj::bvh::build(&mut instances);
    let world = Scene::new(&instances, &bvh);

    let (c, s) = (TILT.cos(), TILT.sin());
    let edge_u = Vec3::new(555.0 * c, 0.0, 555.0 * s);
    let edge_v = Vec3::new(0.0, 555.0, 0.0);
    let corner = Point3::new(0.0, 0.0, dist);

    let mut hits = 0;
    let mut total = 0;

    for gy in 0..grid {
        for gx in 0..grid {
            // Somewhere inside the quad, away from its edges, and at coordinates
            // that are not round numbers: a point that happens to be exactly
            // representable is the one case this cannot go wrong for
            let u = (gx as F + 0.317) / grid as F;
            let v = (gy as F + 0.673) / grid as F;
            let aim = corner + edge_u * u + edge_v * v;

            // Not the aimed at point but the one a camera ray lands on, which is
            // where the rounding this exists to clear comes from. Placing the
            // point by hand would test an epsilon against a point that is already
            // exactly on the surface, which is the one case that never fails
            let eye = Point3::new(277.5, 392.9, dist - 1079.46);
            let camera = Ray::new(eye, aim - eye, 0.0);
            let mut first = HitRecord::default();
            let mut reach = Range::new(ray_epsilon(&camera), F::MAX);
            assert!(
                world.hit(&camera, &mut reach, &mut first),
                "the camera ray missed the wall it was aimed at"
            );
            let (p, norm) = (first.p, first.norm);
            let travelled = first.t * camera.dir().length();

            for &dir in &hemisphere(norm, dirs) {
                let ray = Ray::new(p, dir, 0.0);
                let mut range = Range::new(ray_epsilon_at(p, 1.0, travelled), F::MAX);
                let mut rec = HitRecord::default();

                total += 1;
                if world.hit(&ray, &mut range, &mut rec) {
                    hits += 1;
                }
            }
        }
    }

    (hits, total)
}

/// The offset has to clear the rounding of the point the ray starts at, at every
/// distance a scene might put that point.
///
/// A wall a thousand units out is the Cornell box; the rest bracket it so that a
/// change which happens to suit one scale and not the others is caught here rather
/// than in a render.
#[test]
fn a_ray_leaving_a_quad_never_finds_it_again() {
    for dist in [1.0, 10.0, 100.0, 555.0, 1400.0, 10_000.0] {
        let (hits, total) = self_hits(dist, 8, 64);

        assert_eq!(
            hits, 0,
            "a quad {dist} from the origin caught {hits} of the {total} rays leaving it"
        );
    }
}

/// The offset is a distance in the scene, but the range it is written into counts
/// in units of the ray's direction, and the two are only the same for a unit one.
///
/// This is what the absolute constant got wrong: a camera ray's direction is as
/// long as the focus distance, so the one number meant a whole scene unit for the
/// camera ray and a thousandth of one for the scattered rays that need it. Scaling
/// a direction must not change where the ray is allowed to start.
#[test]
fn the_offset_is_a_distance_and_not_a_count_of_direction_lengths() {
    let orig = Point3::new(300.0, 400.0, 500.0);
    let dir = Vec3::new(0.0, 0.0, 1.0);

    for scale in [1.0, 0.001, 7.5, 1079.46] {
        let ray = Ray::new(orig, dir * scale, 0.0);

        // `t` times the direction is how far along the ray the cutoff sits
        let reach = ray_epsilon(&ray) * ray.dir().length();
        let unit = ray_epsilon(&Ray::new(orig, dir, 0.0));

        assert!(
            (reach - unit).abs() <= unit * 1e-3,
            "a direction {scale} long moved the cutoff from {unit} to {reach}"
        );
    }
}

/// Large enough to clear the rounding, small enough to be invisible.
///
/// A test that only demanded no self hits would pass for an offset of ten units,
/// which would eat the contact shadow under every object in the room. Both ends
/// are the requirement, so both are checked.
#[test]
fn the_offset_stays_between_the_rounding_and_anything_visible() {
    for dist in [1.0, 555.0, 1400.0, 10_000.0] {
        let orig = Point3::new(dist, dist * 0.5, dist * 0.25);
        let ray = Ray::new(orig, Vec3::new(0.0, 0.0, 1.0), 0.0);
        let offset = ray_epsilon(&ray) * ray.dir().length();

        // How badly `f32` rounds a coordinate of this size
        let rounding = dist * F::EPSILON;

        assert!(
            offset >= rounding * 16.0,
            "at {dist} out the offset is {offset}, too close to the {rounding} the \
             coordinate is rounded by"
        );

        // A thousandth of the way across a Cornell box is well under a pixel of
        // any render of one, so a contact shadow keeps its edge
        assert!(
            offset <= (dist * 1e-3).max(1e-3),
            "at {dist} out the offset is {offset}, large enough to show"
        );
    }
}
