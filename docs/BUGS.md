# Known Bugs

No known bugs at this time.

## Resolved

### Vertical bands down the Cornell box's tall box (fixed)

**Symptoms:** A high resolution Cornell box showed vertical streaks down the face
of the tall box, a few percent darker than their surroundings and with hard edges.
They were plateaus rather than noise: at 2000 samples per pixel the noise floor was
half a level in 65535 and the bands were a hundred and fifty, and they landed in
exactly the same columns every render.

**What it was not:** it looked like a random number generator, and it was not.
Replacing xorshift32 with PCG32, and separately rehashing the per pixel seed, each
left the bands in the same columns with the same magnitudes to within a level. Nor
was it the tiling, the first hit geometry (a normal buffer and an analytic N·L
shading of the hit point were both clean), the light sampling and MIS (the bands
survive with direct lighting switched off entirely), or the rasteriser (the
fragment coordinate is exactly the column, once each, for every column).

**Root cause:** self intersection. `RAY_EPSILON` was an absolute `0.001` applied to
`t`, and both halves of that were wrong for this scene.

A hit point is `orig + t * dir` evaluated in `f32`, so it lands off its surface by
roughly its own magnitude times the machine epsilon. In a room measured in hundreds
of units, seen from a thousand away, that is about `1e-4` — the same order as the
`0.001` that was supposed to clear it. Worse, `t` counts in units of the ray's
direction, which is not normalized: a camera ray's direction is as long as the
focus distance, so the one constant meant a whole scene unit for the camera ray and
a thousandth of one for the scattered rays that actually needed it.

Rays leaving the surface therefore found it again and stopped there, and the
fraction that did depended on how the face was tilted and how far away it was, which
is why the tall box (turned fifteen degrees) banded and the axis aligned walls did
not — an axis aligned plane solves for one coordinate and lands on it to the bit.
A direct reproduction of the geometry has 8% of the rays leaving a tilted wall
re-hitting it, matching the 4 to 8% darkening measured in the render.

**Fix:** `Camera::ray_epsilon_at` in `rtx-util/src/cam.rs`. The offset is a distance
in the scene, scaled to the larger of where the ray starts and how far the ray that
found that point travelled, and then divided by the length of the direction so that
it means the same distance whatever the direction is scaled to.

**Tests:** `rtx-util/tests/self_intersection_tests.rs`, which check the offset
against the rounding it exists to clear rather than checking a render for bands. All
three fail against the old constant.

### Live mode hang (fixed)

**Symptoms:** The program would hang during `live` mode, particularly when looking downward. The GPU appeared to be doing infinite work rather than crashing.

**Root cause:** The `rand_unit()` function in `rtx-prim/src/traits.rs` used rejection sampling with an unbounded loop. The xorshift RNG has a property where if the state becomes 0, it stays 0 forever. This caused `rand_f()` to always return 0, which meant `rand_range(-1, 1)` always produced `(-1, -1, -1)` with `length_squared = 3.0`, failing the rejection test and looping forever.

**Fix:** Replaced rejection sampling with direct spherical coordinate sampling that generates uniformly distributed points on the unit sphere in exactly 2 random calls, with no loop.
