# Wavefront Path Tracing

How the tracer would be split into stages that pass their state through buffers,
and why that is worth doing here. Nothing below is built. This is the plan to
pick up if and when it is.

## Relevant Files

- `crates/shader/src/lib.rs` - `trace_fs` and `trace_cs`, the two entry points
- `crates/rtx-util/src/cam.rs` - `Camera::render` and `ray_color`, the loop this
  splits apart
- `crates/host/src/gpu.rs` - `Accumulation`, `create_trace_compute_pipeline`, the
  compute plumbing this builds on
- `crates/rtx-obj/src/scene.rs` - `hit` and `occluded`, which become the extend
  and shadow stages

## Why

Not for the reason the literature usually gives. The standard argument is
divergence: threads in a wavefront take different paths through the shading code
and idle waiting for each other. That argument is weak here, because the scenes
are small, there are four materials and they are all cheap, and the Cornell box is
closed so hardly any path terminates early for compaction to exploit.

The argument that does hold is register pressure. Mesa reports what the megakernel
costs:

```
trace_fs: 1474 instructions, SIMD16, 0 spills, GRF registers: 159
trace_cs: 1486 instructions, SIMD16, 0 spills, GRF registers: 169
```

An Intel thread gets 128 registers before the driver switches to a large register
mode that halves how many threads stay resident on an execution unit. Both entry
points are over that line, so both run at half occupancy, with half the threads
available to hide memory latency.

Half occupancy explains what every measurement in this project has run into. The
instance loop cost the same per iteration whether the work inside it was matrix
products or nothing much. Adding the hierarchy walk cost twelve to fifteen percent
on scenes far too small for the walk itself to matter. Both look like a tracer
waiting on memory with too few threads to hide it behind.

Splitting the megakernel is the direct fix. A stage that only walks the hierarchy,
or only evaluates one material, needs a fraction of the live state the whole path
loop does, and each should land well under 128 registers. **Mesa will say so
before any of this is benchmarked**, which makes the first milestone below a cheap
test of the whole premise: build the stages, read the register counts, and stop if
they are not under the line.

## What It Costs

Wavefront trades registers for bandwidth. State that lived in registers between
bounces has to live in memory instead, and be written and read once per bounce.

At 400x300 with one sample per pass and a path state of 80 bytes:

```
120k paths x 80 bytes x 2 (read and write) x 10 bounces = 192 MB per pass
192 MB x 8 passes                                       = 1.5 GB per frame
```

A 20 ms frame at that rate needs 75 GB/s, against system memory shared with the
CPU. That is the number to watch: it is the same order as the bandwidth available,
so path state wants to be as small as it can be made, and any field that does not
change between bounces wants to stay out of it.

## The Stages

`ray_color` in `cam.rs` currently does all of this in one loop per sample. Each
bullet becomes a kernel.

1. **generate** - one thread per pixel, builds the camera ray and the initial path
   state. What `Camera::get_ray` does now.
2. **extend** - one thread per live path, walks the hierarchy for the closest hit.
   `Scene::hit`. This is the stage most likely to come out cheap in registers, and
   the one worth reading the counts on first.
3. **shade** - one thread per live path: emission and its MIS weight, the direct
   lighting sample, the BSDF sample, Russian roulette. Emits a shadow ray and a
   continuation ray, or terminates the path.
4. **shadow** - one thread per shadow ray, `Scene::occluded`, adding the
   contribution of the ones that reach their light.

The host loop becomes:

```
generate
for depth in 0..max_bounces:
    extend
    shade
    shadow
    stop when no paths are live
```

## Data

Two things live in buffers that live in registers today.

**Path state**, one entry per path in flight:

- ray origin and direction
- throughput
- RNG state
- pixel index, so `shade` knows where to add its contribution
- the previous BSDF density and whether the previous bounce was specular, which
  `ray_color` carries as `prev_pdf_bsdf` and `prev_specular` for the MIS weight

The accumulated colour does **not** belong here. Contributions can be added
straight into the existing `Accumulation` buffer, which already sums passes and is
indexed by pixel.

**Queues**, so a stage only runs over paths that are still live: an index buffer
per stage and an atomic counter for each. `dispatch_workgroups_indirect` reads the
counter, so the host never has to know how many paths survived a bounce.

## Sizing

One sample per pass rather than all of them at once. At 400x300 that is 120k paths
of about 10 MB, against 77 MB for eight samples together, and the pass structure
already exists: `render_to_image_accumulated` loops passes with a different seed
each, and `Accumulation` sums them.

## Milestones

Each is measurable on its own, and the first is the one that decides whether the
rest is worth building.

### Phase 0: a compute entry point

Built once, measured, and taken back out. It went in at 20% slower than the
fragment path before any of the splitting below had happened, which is not worth
carrying a second code path for until someone picks up phase 1. The numbers it
produced are the measurements section at the end, and are the reason the rest of
this document exists.

What it was, and what to build again:

- [ ] `trace_cs`, the megakernel dispatched over 8x8 tiles rather than rasterized.
      The scene bind group needs `ShaderStages::COMPUTE` adding to its visibility,
      and push constants a `COMPUTE` range
- [ ] An accumulation storage buffer of one `vec4` per pixel for it to sum into,
      needing none of the float blending the texture path checks the adapter for.
      Give it `COPY_DST` as well as `STORAGE`, or it cannot be cleared
- [ ] Lanes mapped so a subgroup covers a 4x4 block rather than the 8x2 strip the
      linear order gives. SPIR-V numbers local invocations linearly, so this does
      not come free from an 8x8 workgroup. Worth 10% on its own
- [ ] A blit that puts the buffer on screen, or better, write the swapchain
      directly and skip it - see future work
- [ ] Verified against the fragment path. `cornell_box` rendered byte identical
      last time, so anything less is a bug

### Phase 1: split the kernel, no queues

The cheapest thing that answers the register question. Every stage dispatches over
every path and skips the dead ones with a flag, so there are no atomics, no
indirect dispatch and no compaction to get wrong.

- [ ] Path state buffer and its layout
- [ ] `generate`, `extend`, `shade` as separate entry points
- [ ] Host loop over bounces
- [ ] **Read the GRF counts from Mesa.** If the stages are not under 128, the
      premise is wrong and this stops here
- [ ] Images still match the megakernel

### Phase 2: compaction

- [ ] Queue buffers and atomic counters
- [ ] `dispatch_workgroups_indirect` off the counters
- [ ] Terminated paths stop costing a thread

### Phase 3: shadow rays as their own stage

- [ ] Shadow ray buffer written by `shade`
- [ ] `shadow` kernel over it, adding what is not occluded

### Phase 4: material sorting

- [ ] Sort or bucket the shade queue by material kind

Worth the least of the four here, and worth revisiting only if a scene ever has
enough materials, or expensive enough ones, for the sort to pay for itself. There
are four kinds today and all of them are cheap.

## Verifying It

The renderer is deterministic given a seed, and it should stay that way. Each path
draws its random numbers in a fixed order - the direct lighting sample, then the
BSDF sample, then Russian roulette - and as long as the stages keep that order and
carry the RNG state in the path buffer, a wavefront render should come out
**identical to the megakernel's**, not merely close.

That makes the test easy and strict: render `cornell_box` both ways and compare
bytes, as the hierarchy work did. Anything else is a bug, not noise. The one
allowance is that a contribution added into `Accumulation` from a different stage
may sum in a different order, which is worth a byte or two.

`crates/rtx-util/tests/lighting_tests.rs` traces on the CPU against a quadrature
integral, and stays the reference for whether the light transport is right at all.

## Risks

- **Bandwidth**, as above. The one that could sink it.
- **Atomics in rust-gpu**. `spirv_std::arch` has them, but nothing in this project
  uses one yet, so the first queue counter is also the first test of that.
- **Indirect dispatch through wgpu**, likewise unused so far.
- **Five entry points instead of one.** The scene bindings are already listed one
  by one on every entry point, and each new stage repeats them. Whatever the
  stages share should go in a function they all call, the way `traced_color` is
  shared by the two tracers today.

## Future Work

- **Front to back traversal becomes affordable.** The hierarchy walk is stackless
  because a stack costs per ray scratch space and the megakernel had no registers
  to spare - see the BVH entry in `TODO.md`. If splitting the kernel frees the
  registers this expects it to, an `extend` stage could afford the stack, visit the
  nearer child first and stop at the first hit rather than walking every box it
  enters. The two items on the roadmap are cheaper together than apart, and that is
  an argument for doing this one first.
- **Write the swapchain from the compute stage.** Worth doing whether or not any
  of the above happens: it removes the blit, which is 1.5 ms of the compute path's
  frame today, and it is only there because a storage buffer cannot be presented.
- **Remeasure on a discrete GPU.** The 20% deficit is the single number this whole
  plan is weighed against, and it comes from one integrated part.

## Measurements

The baseline to beat, as the mean frame of each benchmark, on an Intel Arc
integrated GPU (Panther Lake, Mesa):

| benchmark | instances | fragment | compute megakernel |
| --- | --- | --- | --- |
| cornell_box | 18 | 23.0 ms | 29.5 ms |
| many_spheres | 9 | 6.5 ms | 8.0 ms |
| two_spheres | 3 | 2.0 ms | 2.5 ms |
| vertex_cubes | 499 | 18.5 ms | 23.3 ms |

Where the compute path's 6.6 ms on `cornell_box` goes: 1.5 ms is the blit that
puts the buffer on screen, 0.2 ms the clear, and 4.9 ms the dispatch itself. The
blit is scaffolding rather than a real cost - a compute-only renderer would write
the swapchain directly - so the honest figure for the dispatch is about 20%.

What that 20% is not: the driver stats above show the two kernels compile to
almost the same program, so it is not register pressure, spilling or SIMD width.
Workgroup shapes of 8x8, 16x16, 32x1, 8x4 and 4x4 all land within noise of each
other, so it is not the tile either. What is left is how the hardware dispatches
compute against how it dispatches fragments, and an integrated GPU is built around
the second.

This matters for reading any later result: **the stages have to win back 20%
before they show a gain at all.** On another machine that deficit may not exist.
Every number here is from one GPU, and a discrete card would be worth remeasuring
on before drawing conclusions from any of it.
