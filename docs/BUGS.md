# Known Bugs

No known bugs at this time.

## Resolved

### Live mode hang (fixed)

**Symptoms:** The program would hang during `live` mode, particularly when looking downward. The GPU appeared to be doing infinite work rather than crashing.

**Root cause:** The `rand_unit()` function in `rtx-prim/src/traits.rs` used rejection sampling with an unbounded loop. The xorshift RNG has a property where if the state becomes 0, it stays 0 forever. This caused `rand_f()` to always return 0, which meant `rand_range(-1, 1)` always produced `(-1, -1, -1)` with `length_squared = 3.0`, failing the rejection test and looping forever.

**Fix:** Replaced rejection sampling with direct spherical coordinate sampling that generates uniformly distributed points on the unit sphere in exactly 2 random calls, with no loop.
