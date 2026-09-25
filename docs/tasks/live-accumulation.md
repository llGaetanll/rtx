# Live Accumulation

Make `live` responsive by tracing a few samples per frame instead of 40, and
summing frames while the camera stands still so the picture keeps refining.

## Overview

`live` currently draws every frame from scratch at 40 samples per pixel and 10
bounces, straight into the swapchain. The seed is always 0, so a still camera
traces the same rays every frame and shows the same noise: the whole frame budget
goes into redrawing an identical image.

The change is to draw a small number of samples per frame (around 2) into a float
accumulation target, add each frame to it with a new seed, and show the average.
Moving the camera or resizing the window empties it and starts over. Frames get
roughly an order of magnitude cheaper, and a still view passes today's 40 samples
within a few frames and keeps going.

The judging command:

```
cargo run --release -- live -s scenes/cornell_box.toml -c configs/image/cornell_box.toml
```

## Design Decisions

- **Settings live in a `[live]` section of the image config.** `live` already
  takes an image config, so its settings sit next to the camera it starts from.
  The section is optional, with defaults in code, so the other image configs keep
  working without edits:

  ```toml
  [live]
  samples = 2        # per frame
  bounces = 10
  accumulate = true
  ```

  This replaces `PREVIEW_SAMPLES`, `PREVIEW_BOUNCES` and
  `ImageConfig::preview_quality()`, whose only user is `live`.
- **Video configs are untouched.** A video's camera moves every frame, so an
  accumulator would reset every frame and never sum anything. Their `[quality]`
  sample count is already the per-frame setting that matters.
- **`accumulate = false` keeps today's behaviour** at the configured sample count,
  so the two can be compared side by side.
- **Reuse what `render --preview` already has.** `Accumulator` in `gpu.rs` sums
  passes into an `Rgba32Float` target with additive blending and a per-pass seed,
  and `blit_fs` shows the sum scaled by `1 / passes`, fitted to the window. Two
  differences for live:
  - `Accumulator::pass` blocks on `device.poll(Wait)`. Live wants to record the
    pass into the frame's own encoder and submit it along with the blit.
  - Tiling is not needed: a window always fits in one tile.
- **Reset on any change to what is being looked at**: camera position,
  orientation, or window size. A full reset rather than a moving average, which
  would smear the image while moving.
- **Pick an sRGB swapchain format**, as `preview_app.rs` does. `blit_fs` writes
  linear colour and relies on the surface to encode it; live currently takes
  `formats[0]`, which may not be sRGB.

## Relevant Files

- `crates/host/src/live_app.rs` - `LiveApp`, the render loop and camera state
- `crates/host/src/config.rs` - `ImageConfig` (`deny_unknown_fields`, so the new
  section must be declared), `PREVIEW_SAMPLES`, `PREVIEW_BOUNCES`,
  `preview_quality()`
- `crates/host/src/gpu.rs` - `Accumulator`, `Accumulator::pass`, `Tiling`
- `crates/host/src/preview_app.rs` - `Blit`, and the sRGB format selection, to
  reuse or lift out
- `crates/shader/src/lib.rs` - `trace_fs` (seeded from `constants.seed`),
  `blit_fs`
- `crates/shared/src/lib.rs` - `ShaderConstants`, `BlitConstants`
- `configs/image/cornell_box.toml` - the config used to judge this

## Phases

### Phase 1: `[live]` config section

- [x] Add a `Live` struct (`samples`, `bounces`, `accumulate`) with defaults, as
      an optional `live` field on `ImageConfig`
- [x] Validate it: `samples` and `bounces` above zero
- [x] Replace `PREVIEW_SAMPLES`, `PREVIEW_BOUNCES` and `preview_quality()` with it
      in `live_app.rs`
- [x] Tests: a config without the section gets the defaults, and one with it
      parses
- [x] Add a `[live]` section to `configs/image/cornell_box.toml`

### Phase 2: Accumulate in live

- [ ] Give `Accumulator` a pass that records into a caller's encoder without
      waiting on the GPU, keeping the blocking one for `render`
- [ ] Hold an `Accumulator` and a blit pipeline in `LiveApp`, sized to the window
- [ ] Each frame: one pass into the accumulator, then blit it to the swapchain
- [ ] Pick an sRGB swapchain format
- [ ] `accumulate = false`: reset before every pass, so each frame stands alone
- [ ] Reset when the camera position or orientation changes
- [ ] Recreate the accumulator when the window is resized
- [ ] Show the pass count in the window title

### Phase 3: Check it

- [ ] Run the judging command and compare frame rate and still-camera quality
      against `accumulate = false` with `samples = 40`
- [ ] Update the "Dynamic ray sampling" item in `live-mode.md` and the "Sample
      accumulation in live mode" item in `TODO.md`

## Future Work

- [ ] Movement speed is 5 units a second, which barely moves in a 555 unit
      Cornell box. It should scale with the scene
- [ ] Stop adding passes after a cap, so a converged view leaves the GPU idle
- [ ] Adapt samples per frame to hold a frame time target
- [ ] Lower resolution while moving
