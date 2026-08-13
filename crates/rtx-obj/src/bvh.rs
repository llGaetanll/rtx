//! A bounding volume hierarchy over the instances of a scene.
//!
//! Without one, every ray tests every instance, and the cost of a frame is the
//! instance count times the ray count. The hierarchy replaces that scan with a
//! walk down a tree of boxes, so a ray only pays for the instances it could
//! plausibly have hit.
//!
//! The tree is built on the host and uploaded as a flat array of nodes in depth
//! first order. Two things follow from that order, and they are what let the
//! shader walk the tree without a stack:
//!
//! - the node after an interior node is its first child, so a ray that enters a
//!   box carries on to the next index;
//! - a whole subtree occupies one contiguous run, so a ray that misses a box can
//!   skip to the far end of that run in one step. That index is `exit`.
//!
//! A stack would let the walk visit the nearer child first and stop sooner, but
//! it would also need per ray scratch space, and the tracer is already short of
//! registers. This walk needs one loop counter.

use bytemuck::Pod;
use bytemuck::Zeroable;
use rtx_prim::F;
use rtx_prim::Point3;
use rtx_prim::Vec3;

/// One box of the hierarchy, as the GPU reads it.
///
/// Plain float arrays rather than `Vec3`, and each one followed by a word, for
/// the reason `Light` gives: glam's vectors are sixteen bytes when compiled for
/// SPIR-V and twelve on the host, so a vector in an uploaded type would leave the
/// two disagreeing about the layout.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct BvhNode {
    /// Low corner of the box this node stands for.
    pub min: [F; 3],

    /// Where to carry on when a ray misses this box: one past the last node of
    /// this node's subtree. On the root that is the end of the array, which is
    /// what stops the walk.
    pub exit: u32,

    /// High corner of the box this node stands for.
    pub max: [F; 3],

    /// The first instance this leaf holds. Unread on an interior node, which
    /// holds none of its own.
    pub first: u32,

    /// How many instances this leaf holds, or zero on an interior node. A leaf
    /// covers the run `[first, first + count)` of the instance buffer.
    pub count: u32,

    /// The three words that round the type up to 48 bytes. Rust adds them either
    /// way, since the type is sixteen aligned and its 40 bytes of fields round
    /// up; spelling them out is what keeps them initialized, and reading
    /// uninitialized bytes is what would make handing the type to the GPU
    /// unsound.
    pub _pad: [u32; 3],
}

// SAFETY: `repr(C)` over six floats and five `u32`s with every slot written
// down, so all bit patterns are valid and there is no implicit padding. Not
// derived because the shader builds glam without its bytemuck feature.
unsafe impl Zeroable for BvhNode {}
unsafe impl Pod for BvhNode {}

const _: () = assert!(core::mem::size_of::<BvhNode>() == 48);

impl BvhNode {
    pub fn is_leaf(&self) -> bool {
        self.count != 0
    }
}

/// Whether `ray` crosses this node's box anywhere within `range`.
///
/// `inv_dir` is the reciprocal of the ray direction. It is passed in rather than
/// computed here because it is the same for every node a ray visits, and three
/// divides per node is most of the cost of the test.
///
/// An axis the ray runs parallel to divides by zero and gives infinities, which
/// compare the right way round and need no branch of their own. The one case
/// they do not survive is a ray that runs along the plane of a face exactly,
/// where the infinities meet a zero and give a NaN; `min` and `max` return their
/// other operand there, which lets the axis go unconstrained rather than
/// rejecting a box the ray may well cross.
pub fn node_hit(node: &BvhNode, orig: Point3, inv_dir: Vec3, t_min: F, t_max: F) -> bool {
    let min = Vec3::new(node.min[0], node.min[1], node.min[2]);
    let max = Vec3::new(node.max[0], node.max[1], node.max[2]);

    let t0 = (min - orig) * inv_dir;
    let t1 = (max - orig) * inv_dir;

    let near = t0.min(t1).max_element().max(t_min);
    let far = t0.max(t1).min_element().min(t_max);

    near <= far
}

/// The reciprocal of a ray direction, for `node_hit`.
pub fn inv_dir(dir: Vec3) -> Vec3 {
    Vec3::ONE / dir
}

#[cfg(not(target_arch = "spirv"))]
pub use build::build;

#[cfg(not(target_arch = "spirv"))]
mod build {
    use rtx_prim::Aabb;
    use rtx_prim::F;

    use super::BvhNode;
    use crate::Instance;

    /// What one instance test costs, against a node test taken as one.
    ///
    /// An instance test puts the ray through an inverse transform before it can
    /// look at a sphere or a quad at all, which is two matrix products; a node
    /// test is a subtract, a multiply and a compare per axis. The ratio decides
    /// how eagerly the build splits, and a split that does not pay for itself is
    /// how the Cornell box ends up with no tree to walk.
    const COST_INSTANCE: F = 4.;

    /// What walking one node costs, on the same scale.
    ///
    /// Four rather than the one the arithmetic suggests, because the tracer is
    /// bound by how many times it goes round a loop rather than by what it does
    /// inside one: a node a ray visits costs it a dependent load and a branch
    /// that the rest of its wavefront waits on. Measured against the benchmarks,
    /// one left the Cornell box splitting into a tree that cost more than the
    /// scan it replaced, and sixteen left the cube field with leaves too big.
    const COST_NODE: F = 4.;

    /// How many buckets the candidate splits along an axis are gathered into.
    /// Every boundary between two of them is a split worth costing.
    ///
    /// Twelve because past it the split stops moving: thirty two and sixty four
    /// measure the same to within the noise of a run, and only four is clearly
    /// worse. The exact sweep this approximates, which would cost every boundary
    /// between two neighbouring instances, has nothing left to find.
    const BINS: usize = 12;

    // No floor is put under a leaf, on purpose. The usual advice is to stop
    // splitting a few primitives short of the bottom, which holds where testing a
    // primitive is cheap next to visiting a node. Here it is the other way round,
    // an instance test being two matrix products, so the cost model keeps
    // splitting and forcing it to stop early only costs: leaves of eight put the
    // cube field up from 18.5ms to 23.3 and sixteen to 31.3, while a floor of two
    // or four changed nothing either way. The model already makes leaves bigger
    // where they pay, which is why the Cornell box has leaves of five without
    // being told to.

    /// Build a hierarchy over `instances`, reordering them to match.
    ///
    /// The reordering is the point: a leaf names a run of instances by where it
    /// starts and how long it is, and those runs only stay contiguous if the
    /// instances are laid out in the order the splitting settled on. Nothing
    /// outside this buffer refers to an instance by its position - an emitter
    /// carries its own `light_index` - so the order is this function's to choose.
    pub fn build(instances: &mut Vec<Instance>) -> Vec<BvhNode> {
        if instances.is_empty() {
            return Vec::new();
        }

        let boxes: Vec<Aabb> = instances.iter().map(Instance::world_bbox).collect();
        let mut order: Vec<usize> = (0..instances.len()).collect();

        let mut nodes = Vec::new();
        split(&mut order, &boxes, 0, &mut nodes);

        *instances = order.iter().map(|&i| instances[i]).collect();

        nodes
    }

    /// Emit the subtree covering `order`, whose first instance lands at `first`
    /// once the instances are reordered.
    fn split(order: &mut [usize], boxes: &[Aabb], first: usize, nodes: &mut Vec<BvhNode>) {
        let bounds = bounds_of(order, boxes);

        // Reserved before the children are emitted, so the subtree occupies one
        // run starting here. Its `exit` is only known once that run is complete
        let here = nodes.len();
        nodes.push(BvhNode {
            min: [bounds.x().start, bounds.y().start, bounds.z().start],
            exit: 0,
            max: [bounds.x().end, bounds.y().end, bounds.z().end],
            first: first as u32,
            count: 0,
            _pad: [0; 3],
        });

        match best_split(order, boxes, &bounds) {
            // Splitting is only worth it when the two halves between them cost a
            // ray less than testing everything here would. Where the instances
            // all overlap, as six walls of one room do, nothing separates them
            // and the scan this replaces is what a leaf goes back to
            Some(split_at) if split_at.cost < order.len() as F * COST_INSTANCE => {
                let mid = partition(order, boxes, split_at.axis, split_at.bin);
                let (left, right) = order.split_at_mut(mid);

                split(left, boxes, first, nodes);
                split(right, boxes, first + mid, nodes);
            }
            _ => nodes[here].count = order.len() as u32,
        }

        nodes[here].exit = nodes.len() as u32;
    }

    /// A candidate split: everything below `bin` on `axis` against everything
    /// above it, and what a ray would pay for the arrangement.
    struct Split {
        axis: usize,
        bin: usize,
        cost: F,
    }

    /// The cheapest split of `order`, or `None` when the centroids all coincide
    /// and there is nothing to separate.
    ///
    /// The cost of a split is what a ray reaching this node can expect to pay:
    /// one node test to get here, plus each side's instance tests weighted by how
    /// often a ray that reached this box goes on to enter that side, which for a
    /// convex box a ray crosses is the ratio of the two surface areas.
    fn best_split(order: &[usize], boxes: &[Aabb], bounds: &Aabb) -> Option<Split> {
        let mut best: Option<Split> = None;

        for axis in 0..3 {
            let (lo, hi) = centroid_span(order, boxes, axis);
            if hi <= lo {
                continue;
            }

            // Gather the instances into buckets along the axis, so the cost of
            // every boundary between two buckets can be worked out from running
            // totals rather than from another pass over the instances
            let mut counts = [0usize; BINS];
            let mut bin_bounds: [Option<Aabb>; BINS] = Default::default();

            for &i in order {
                let b = bin_of(centroid(&boxes[i], axis), lo, hi);
                counts[b] += 1;
                bin_bounds[b] = Some(match &bin_bounds[b] {
                    Some(acc) => acc.union(&boxes[i]),
                    None => boxes[i].clone(),
                });
            }

            // Sweep from the left, then from the right, so each boundary knows
            // what lies on both sides of it
            let mut left_count = [0usize; BINS];
            let mut left_area = [0.; BINS];
            let mut acc: Option<Aabb> = None;
            let mut running = 0;

            for b in 0..BINS {
                left_count[b] = running;
                left_area[b] = acc.as_ref().map_or(0., area);
                running += counts[b];
                acc = union_opt(acc, bin_bounds[b].as_ref());
            }

            let mut acc: Option<Aabb> = None;
            let mut right = 0;

            for b in (1..BINS).rev() {
                acc = union_opt(acc, bin_bounds[b].as_ref());
                right += counts[b];

                let left = left_count[b];
                if left == 0 || right == 0 {
                    continue;
                }

                let whole = area(bounds);
                let cost = COST_NODE
                    + (left_area[b] * left as F + area(acc.as_ref().unwrap()) * right as F)
                        / whole
                        * COST_INSTANCE;

                if best.as_ref().is_none_or(|s| cost < s.cost) {
                    best = Some(Split { axis, bin: b, cost });
                }
            }
        }

        best
    }

    /// Move everything below `bin` to the front of `order` and return how many
    /// that is. The order within either side is not meaningful, so this swaps
    /// rather than preserving it.
    fn partition(order: &mut [usize], boxes: &[Aabb], axis: usize, bin: usize) -> usize {
        let (lo, hi) = centroid_span(order, boxes, axis);

        let mut mid = 0;
        for i in 0..order.len() {
            if bin_of(centroid(&boxes[order[i]], axis), lo, hi) < bin {
                order.swap(i, mid);
                mid += 1;
            }
        }

        // The sweep only ever picks a boundary with instances on both sides, so
        // this cannot come out empty. Asserted rather than left implied: a split
        // into nothing would recurse forever
        debug_assert!(mid > 0 && mid < order.len(), "split into nothing");

        mid
    }

    fn bin_of(centroid: F, lo: F, hi: F) -> usize {
        let t = (centroid - lo) / (hi - lo);

        ((t * BINS as F) as usize).min(BINS - 1)
    }

    fn centroid_span(order: &[usize], boxes: &[Aabb], axis: usize) -> (F, F) {
        let mut lo = F::INFINITY;
        let mut hi = F::NEG_INFINITY;

        for &i in order {
            let c = centroid(&boxes[i], axis);
            lo = lo.min(c);
            hi = hi.max(c);
        }

        (lo, hi)
    }

    fn union_opt(acc: Option<Aabb>, next: Option<&Aabb>) -> Option<Aabb> {
        match (acc, next) {
            (Some(acc), Some(next)) => Some(acc.union(next)),
            (Some(acc), None) => Some(acc),
            (None, next) => next.cloned(),
        }
    }

    /// The surface area of a box, which is what a ray's odds of entering it are
    /// proportional to.
    fn area(bbox: &Aabb) -> F {
        let dx = bbox.x().end - bbox.x().start;
        let dy = bbox.y().end - bbox.y().start;
        let dz = bbox.z().end - bbox.z().start;

        2. * (dx * dy + dy * dz + dz * dx)
    }

    fn bounds_of(order: &[usize], boxes: &[Aabb]) -> Aabb {
        // Folded from the first box rather than from an empty one: `Aabb::empty`
        // is the zero sized box at the origin, and a union with it would drag
        // every bound out to include a point nothing is near
        let mut bounds = boxes[order[0]].clone();

        for &i in &order[1..] {
            bounds.union_mut(&boxes[i]);
        }

        bounds
    }

    fn centroid(bbox: &Aabb, axis: usize) -> F {
        (bbox[axis].start + bbox[axis].end) / 2.
    }
}
