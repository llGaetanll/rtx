use core::borrow::Borrow;
use core::ops::Range;

use crate::F;

pub type RandState = u64;

mod generators {
    use core::ops::BitXorAssign;
    use core::ops::Shl;
    use core::ops::Shr;

    /// A simple PRF with no dependencies.
    pub fn prf<T>(state: T) -> T
    where
        T: Shl<i32, Output = T> + Shr<i32, Output = T> + BitXorAssign + Copy,
    {
        xorshift(state)
    }

    /// xorshift
    ///
    /// See: https://en.wikipedia.org/wiki/Xorshift#Example_implementation
    fn xorshift<T>(state: T) -> T
    where
        T: Shl<i32, Output = T> + Shr<i32, Output = T> + BitXorAssign + Copy,
    {
        let mut x = state;

        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;

        x
    }
}

pub fn init_state() -> RandState {
    42
}

/// Generates a pseudo-random `u64` in its entire domain.
pub fn rand_u64(state: &mut RandState) -> u64 {
    *state = generators::prf(*state);

    *state
}

#[cfg(target_pointer_width = "64")]
pub fn rand_usize(state: &mut RandState) -> usize {
    *state = generators::prf(*state);

    *state as usize
}

#[cfg(target_pointer_width = "64")]
pub fn rand_usize_range<R>(state: &mut RandState, range: R) -> usize
where
    R: Borrow<Range<usize>>,
{
    let Range { start, end } = range.borrow();

    let u = rand_usize(state);

    start + u % (end - start)
}

/// Generates a pseudo-random `F` in `[0, 1)`.
pub fn rand_f(state: &mut RandState) -> F {
    let x = rand_u64(state);

    let man = x >> (u32::BITS - F::MANTISSA_DIGITS);

    (man as F) / (1u32 << F::MANTISSA_DIGITS) as F
}

/// Generates a pseudo-random `F` in the given range.
pub fn rand_f_range<R>(state: &mut RandState, range: R) -> F
where
    R: Borrow<Range<F>>,
{
    let Range { start, end } = range.borrow();

    let f = rand_f(state);

    start + f * (end - start)
}
