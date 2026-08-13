//! The hierarchy, checked against the scan it replaces.
//!
//! An acceleration structure is only ever allowed to be faster. Every test here
//! is some form of the same question: does walking the tree answer exactly what
//! testing everything would? A tree that quietly misses an instance still renders
//! a picture, just one with a hole in it, so the answers are compared rather than
//! eyeballed.

use rtx_mat::Hit;
use rtx_mat::HitRecord;
use rtx_mat::MaterialInfo;
use rtx_obj::BvhNode;
use rtx_obj::Instance;
use rtx_obj::Scene;
use rtx_obj::bvh;
use rtx_obj::hit_unit_quad;
use rtx_obj::hit_unit_sphere;
use rtx_obj::primitive_kind;
use rtx_obj::transform_hit_to_world;
use rtx_obj::transform_ray_to_object;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::Range;
use rtx_prim::Ray;
use rtx_prim::Vec3;
use rtx_prim::rand;

/// What `Scene::hit` did before there was a tree: test everything, keep the
/// nearest. The tree has to agree with this on every ray.
fn scan(instances: &[Instance], ray: &Ray, t_int: &Range<F>) -> Option<HitRecord> {
    let mut best: Option<HitRecord> = None;
    let mut closest = t_int.end;

    for inst in instances {
        let obj_ray = transform_ray_to_object(ray, &inst.inv_transform);
        let mut obj_rec = HitRecord::default();
        let mut range = Range::new(t_int.start, closest);

        let hit = match inst.kind {
            primitive_kind::SPHERE => hit_unit_sphere(&obj_ray, &mut range, &mut obj_rec),
            _ => hit_unit_quad(&obj_ray, &mut range, &mut obj_rec),
        };

        if hit {
            transform_hit_to_world(&mut obj_rec, &inst.inv_transform, ray);
            obj_rec.mat = inst.material;
            obj_rec.light_index = inst.light_index;
            closest = obj_rec.t;
            best = Some(obj_rec);
        }
    }

    best
}

/// A scattering of spheres and quads over a few units, built from a seed so a
/// failure can be reproduced.
fn scene(seed: u32, count: usize) -> Vec<Instance> {
    let mut state = seed.max(1);
    let coord = |state: &mut u32| rand::rand_f_range(state, Range::new(-5., 5.));

    (0..count)
        .map(|i| {
            let centre = Point3::new(coord(&mut state), coord(&mut state), coord(&mut state));
            let material = MaterialInfo::lambertian(0);

            if i % 3 == 0 {
                let u = Vec3::new(coord(&mut state), coord(&mut state), coord(&mut state));
                let v = Vec3::new(coord(&mut state), coord(&mut state), coord(&mut state));

                Instance::quad(centre, u, v, material)
            } else {
                let radius = rand::rand_f_range(&mut state, Range::new(0.2, 1.5));

                Instance::sphere(centre, radius, material)
            }
        })
        .collect()
}

/// Rays from all over, aimed all over, so the traversal is asked about boxes it
/// enters, boxes it misses and boxes it starts inside.
fn rays(seed: u32, count: usize) -> Vec<Ray> {
    let mut state = seed.max(1);
    let coord = |state: &mut u32| rand::rand_f_range(state, Range::new(-12., 12.));

    (0..count)
        .map(|_| {
            let orig = Point3::new(coord(&mut state), coord(&mut state), coord(&mut state));
            let target = Point3::new(coord(&mut state), coord(&mut state), coord(&mut state));

            Ray::new(orig, target - orig, 0.)
        })
        .collect()
}

/// The whole point, stated directly. Whatever the tree answers, the scan answers
/// the same, for scenes small enough to be one leaf and large enough to be deep.
#[test]
fn the_tree_finds_what_the_scan_finds() {
    for count in [1, 2, 3, 7, 18, 64, 257] {
        let mut instances = scene(count as u32 + 1, count);
        let nodes = bvh::build(&mut instances);
        let world = Scene::new(&instances, &nodes);

        for (n, ray) in rays(99, 200).iter().enumerate() {
            let t_int = Range::new(0.001, F::MAX);

            let mut rec = HitRecord::default();
            let mut range = t_int;
            let found = world.hit(ray, &mut range, &mut rec);

            match scan(&instances, ray, &t_int) {
                Some(expected) => {
                    assert!(found, "{count} instances, ray {n}: tree missed a hit");
                    assert_eq!(
                        rec.t, expected.t,
                        "{count} instances, ray {n}: tree found a different surface"
                    );
                }
                None => assert!(
                    !found,
                    "{count} instances, ray {n}: tree hit something that is not there"
                ),
            }
        }
    }
}

/// A shadow ray asks a cheaper question, and has to get the same answer to it.
#[test]
fn occlusion_agrees_with_the_scan() {
    let mut instances = scene(7, 40);
    let nodes = bvh::build(&mut instances);
    let world = Scene::new(&instances, &nodes);

    for (n, ray) in rays(1234, 300).iter().enumerate() {
        // Short of the full length, so some rays stop before what blocks them
        let range = Range::new(0.001, 0.6);

        assert_eq!(
            world.occluded(ray, &range),
            scan(&instances, ray, &range).is_some(),
            "ray {n}: occlusion and the scan disagree"
        );
    }
}

/// Every instance sits in exactly one leaf, and the leaves tile the buffer. An
/// instance in no leaf is invisible; one in two leaves is tested twice.
#[test]
fn the_leaves_cover_every_instance_once() {
    for count in [1, 5, 33, 128] {
        let mut instances = scene(count as u32, count);
        let nodes = bvh::build(&mut instances);

        let mut covered = vec![0u32; instances.len()];
        for node in nodes.iter().filter(|n| n.is_leaf()) {
            for i in 0..node.count {
                covered[(node.first + i) as usize] += 1;
            }
        }

        assert!(
            covered.iter().all(|&c| c == 1),
            "{count} instances: covered {covered:?}"
        );
    }
}

/// The walk relies on a subtree being one unbroken run of nodes that `exit`
/// points past. Where that is not so, a ray skipping a box either lands inside
/// the subtree it meant to skip or runs off the end.
#[test]
fn every_subtree_is_one_run_the_exit_points_past() {
    let mut instances = scene(11, 100);
    let nodes = bvh::build(&mut instances);

    for (i, node) in nodes.iter().enumerate() {
        assert!(
            node.exit as usize > i,
            "node {i} exits backwards to {}",
            node.exit
        );
        assert!(
            node.exit as usize <= nodes.len(),
            "node {i} exits past the end at {}",
            node.exit
        );

        // A leaf is its own whole subtree, so the walk carrying on to the next
        // node and the walk skipping this one have to arrive at the same place
        if node.is_leaf() {
            assert_eq!(node.exit as usize, i + 1, "leaf {i} claims a subtree");
        }
    }

    assert_eq!(
        nodes[0].exit as usize,
        nodes.len(),
        "the root does not cover the whole tree"
    );
}

/// A node's box has to contain everything under it, or the ray that misses the
/// box skips an instance it would have hit.
#[test]
fn every_box_contains_what_is_under_it() {
    let mut instances = scene(3, 80);
    let nodes = bvh::build(&mut instances);

    for (i, node) in nodes.iter().enumerate() {
        // Which instances lie under this node: the leaves within its run
        let subtree = &nodes[i..node.exit as usize];

        for leaf in subtree.iter().filter(|n| n.is_leaf()) {
            for k in 0..leaf.count {
                let bbox = instances[(leaf.first + k) as usize].world_bbox();

                for axis in 0..3 {
                    assert!(
                        node.min[axis] <= bbox[axis].start && bbox[axis].end <= node.max[axis],
                        "node {i} does not contain instance {} on axis {axis}",
                        leaf.first + k
                    );
                }
            }
        }
    }
}

/// A tree over nothing is nothing, rather than a root pointing at instances that
/// are not there.
#[test]
fn an_empty_scene_builds_an_empty_tree() {
    let mut instances: Vec<Instance> = Vec::new();
    let nodes: Vec<BvhNode> = bvh::build(&mut instances);

    assert!(nodes.is_empty());
}
