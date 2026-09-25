# Denoising in Live Mode

Make a moving view in `live` look clean rather than grainy, by reusing samples
across frames and across neighbouring pixels. Nothing below is built. This is
the plan to pick up when it is time.

## Overview

With [live accumulation](live-accumulation.md), a still view refines quickly,
but a moving one is thrown away every frame. What the window shows while moving
is therefore a single pass of `[live].samples`, around 2 samples per pixel, and
that is the noise.

Real-time path tracers solve this with two filters, which together are SVGF
(Schied et al., 2017):

1. **Temporal reprojection.** Keep the accumulated image while the camera moves.
   Each pixel finds where its surface was in the previous frame and blends into
   that history, so a moving view keeps most of the samples it already had.
2. **Spatial filtering.** Blur what is still noisy, but stop at edges, using the
   surface normal and depth of what each pixel sees. Walls go smooth and the
   edges of the boxes stay sharp.

Both need to know what surface each pixel's first hit landed on, which the
tracer does not currently output. That is the first phase, and both filters
build on it.

## Design Decisions

- **SVGF-style, built from fragment passes.** Every pass in this project is a
  full screen triangle, which suits both filters. No compute or interop is
  needed.
- **No vendor denoisers.** OptiX and OIDN would need interop out of wgpu or a
  readback to the CPU every frame. Neither fits a wgpu and rust-gpu project.
- **Live only.** `render` converges to thousands of samples and needs no
  denoiser, and a filtered image is biased. The denoiser should also fade out
  as a still view accumulates, so a converged live view is the same unbiased
  image it is today.
- **The first hit comes from one ray per pixel.** With several samples per pass,
  and with defocus, each sample's first hit can differ. The pixel centre ray, or
  the first sample's ray, gives one stable surface per pixel. Using the first
  sample's ray costs nothing extra.
- **Sample counts per pixel, not per image.** Once history is reprojected,
  pixels have different amounts of it. A disoccluded pixel starts from zero while
  its neighbour keeps its history. The accumulation target's alpha channel
  already sums to the pass count, because `trace_fs` writes `1.0` there. Carried
  through reprojection, it becomes each pixel's own count, and the blit divides
  by it instead of by a global `1 / passes`.
- **Open question: `moving_scale`.** History at one resolution does not line up
  with frames at another. Either reproject across the change of size, or treat
  denoising and `moving_scale` as alternatives, which is likely simpler since
  denoising makes lower resolution less necessary.

## Relevant Files

- `crates/shader/src/lib.rs` - `trace_fs` (gains the first-hit outputs),
  `blit_fs`, and new filter entry points
- `crates/rtx-util/src/cam.rs` - `Camera::render` and `ray_color`, where the
  first hit is found; `Camera::new`, which reprojection has to invert
- `crates/shared/src/lib.rs` - `ShaderConstants`, `BlitConstants`, and new
  constants for the filter passes (the previous frame's camera)
- `crates/host/src/gpu.rs` - `Accumulator`, which gains the first-hit targets
  and a history to ping-pong with
- `crates/host/src/live_app.rs` - the frame loop, which today resets on any
  camera change
- `crates/host/src/blit.rs` - the blit, which moves to per-pixel counts
- `crates/host/src/config.rs` - `Live`, for the setting that turns this on

## Phases

### Phase 1: First-hit outputs

- [ ] `trace_fs` writes a second and third output: first-hit depth along the
      ray, and the surface normal. Albedo too, if phase 3 demodulates
- [ ] Extra render targets in `Accumulator` for them, with blending replaced
      rather than added, since they describe this frame, not a sum
- [ ] A debug view in `live` that shows depth or normals instead of colour, to
      check them by eye
- [ ] Check that frame time has not moved. Extra outputs add live state to a
      kernel that is already short of registers (see `wavefront.md`)

### Phase 2: Temporal reprojection

- [ ] Keep the previous frame's camera, and pass it to a reprojection pass
- [ ] For each pixel, rebuild its first-hit world position from depth and the
      current camera, and project it into the previous camera to find where it
      was
- [ ] Two history textures, ping-ponged, since a pass cannot read and write the
      same one
- [ ] Reject history where the surface was hidden last frame: the reprojected
      depth or normal disagrees with what the previous frame's first-hit targets
      hold there, or the point was off screen
- [ ] Bilinear history fetch done by hand (the float target cannot be filtered),
      weighting only the taps that pass the rejection test
- [ ] Carry each pixel's sample count in alpha, and cap it while moving (say 32
      passes), so reprojection errors fade instead of lingering
- [ ] The blit divides by each pixel's count
- [ ] Moving no longer resets the accumulator. A still view keeps summing
      exactly as now
- [ ] A `[live]` setting to turn it on, opt in like the others

### Phase 3: Spatial filter

- [ ] À-trous wavelet filter: a 5x5 kernel applied several times at
      increasing step sizes (1, 2, 4, 8), each iteration its own pass
- [ ] Edge-stopping weights from normal, depth and luminance, so the blur stays
      on one surface
- [ ] Divide the albedo out before filtering and multiply it back after, so
      texture detail is not blurred along with the noise. There are no image
      textures yet, so this is optional until there are
- [ ] Filter strength falls off with each pixel's sample count, reaching zero
      well before a still view converges

### Phase 4: Variance guidance

- [ ] Track per-pixel luminance moments alongside the history, and estimate
      variance from them
- [ ] Scale the luminance edge-stopping weight by that variance, so noisy
      regions blur more and converged ones hardly at all. This is what turns
      phases 2 and 3 into SVGF proper

## Cheaper Alternatives

Smaller things that reduce visible noise without any of the above. None of them
replace it, but any could be done on its own first:

- [ ] **`moving_samples`**: more samples per pixel while moving, paired with
      `moving_scale`. Half resolution affords four times the samples for the
      same frame time: softer, but less noisy
- [ ] **Firefly clamp**: cap the brightness of a single sample, removing the
      isolated bright specks from indirect paths that find the light by chance.
      Biased, so live only, perhaps only while moving
- [ ] **Blue-noise or low-discrepancy sampling**: replace the white-noise
      random numbers with a sequence that spreads error evenly. The same sample
      count, but the noise reads as fine grain rather than clumps

## References

- Schied et al., "Spatiotemporal Variance-Guided Filtering: Real-Time
  Reconstruction for Path-Traced Global Illumination", HPG 2017
- Dammertz et al., "Edge-Avoiding À-Trous Wavelet Transform for Fast Global
  Illumination Filtering", HPG 2010

## Future Work

- (items added here as we discover them during implementation)
