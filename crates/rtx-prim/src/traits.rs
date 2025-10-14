use spirv_std::num_traits::Float;

use crate::rand;
use crate::RandState;
use crate::Range;
use crate::Vec3;
use crate::F;

pub trait Vec3Ext {
    fn rand_range(state: &mut RandState, range: Range<F>) -> Self;

    fn rand_unit(state: &mut RandState) -> Self;

    fn rand_hemisphere(state: &mut RandState, normal: Vec3) -> Self;

    fn rand_unit_disk(state: &mut RandState) -> Self;

    fn near_zero(&self) -> bool;
}

impl Vec3Ext for Vec3 {
    fn rand_range(state: &mut RandState, range: Range<F>) -> Self {
        let x = rand::rand_f_range(state, range);
        let y = rand::rand_f_range(state, range);
        let z = rand::rand_f_range(state, range);

        Self::new(x, y, z)
    }

    fn rand_unit(state: &mut RandState) -> Self {
        loop {
            let p = Vec3::rand_range(state, Range::new(-1.0, 1.0));

            let l = p.length_squared();

            // Avoid floating point error
            if (1e-160..1.).contains(&l) {
                return p / l.sqrt();
            }
        }
    }

    fn rand_hemisphere(state: &mut RandState, normal: Vec3) -> Self {
        let v = Self::rand_unit(state);

        if v.dot(normal) > 0. {
            v
        } else {
            -v
        }
    }

    fn rand_unit_disk(state: &mut RandState) -> Self {
        loop {
            let rng = Range::new(-1.0, 1.0);
            let x = rand::rand_f_range(state, rng);
            let y = rand::rand_f_range(state, rng);

            let p = Vec3::new(x, y, 0.0);

            if p.length_squared() < 1. {
                return p;
            }
        }
    }

    fn near_zero(&self) -> bool {
        const ERR: F = 1e-8;

        (self.x.abs() < ERR) && (self.y.abs() < ERR) && (self.z.abs() < ERR)
    }
}
