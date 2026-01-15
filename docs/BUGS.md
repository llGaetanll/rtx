# Known Bugs

## Live mode crash (unresolved)

### Symptoms
- The program crashes during `live` mode with the `two_spheres_fs` scene
- No error messages - the GPU just dies ("Parent device is lost")
- Originally thought to be related to looking straight up/down, but investigation revealed it's more complex

### What we ruled out

**Gimbal lock (fixed separately):** We initially suspected gimbal lock because the crash seemed to happen when looking down. However:
- We added tests in `crates/rtx-util/tests/camera_tests.rs` that demonstrate gimbal lock produces NaN ray directions
- We fixed gimbal lock by passing `vup` from host to shader and switching to quaternion-based camera orientation
- The crash still occurs after the fix

**Key observation:** The crash happened at `dir=(0.81, -0.15, 0.57)` - a nearly horizontal view direction, NOT at a steep angle. This suggests the crash is unrelated to gimbal lock.

### What we know
1. The crash only happens on GPU, not CPU - the ray tracing code runs fine in CPU tests
2. Looking up works fine; looking down (toward ground) triggers it more often
3. Camera height affects when crash occurs - higher up means you can look further down
4. The crash also happens when camera enters inside an object
5. The crash may not be deterministic - it might depend on accumulated state or specific ray/geometry combinations

### Hypotheses to investigate
1. **Ray-sphere intersection edge cases** - When rays hit the large ground sphere at steep angles or from certain positions, there may be numerical issues
2. **Transform precision** - The ground sphere has a large scale (radius=100), which means the inverse transform scales by 1/100. Combined with large world-space coordinates, this could cause precision issues
3. **GPU-specific behavior** - Something that works on CPU might fail on GPU due to different floating-point handling, shader compilation, or driver issues
4. **State accumulation** - Something building up over frames (memory, numerical drift, etc.)

### Reproduction
1. Run `cargo run -- live --scene two_spheres_fs`
2. Move the camera around, particularly looking downward toward the ground
3. The crash typically happens after some movement, not immediately

### Test infrastructure
- CPU tests exist in `crates/rtx-obj/tests/scatter_tests.rs` and `crates/rtx-util/tests/camera_tests.rs`
- These can be used to test ray tracing logic without GPU involvement
- The `test_two_spheres_look_down_crash` test in scatter_tests.rs attempts to reproduce the crash scenario but passes on CPU

### Suggested next step: Write-ahead logging

Add logging of shader inputs *before* each frame is submitted to the GPU. This way, when the crash occurs, we'll have the exact parameters that caused it.

**Implementation:**
1. In `render()`, log `ShaderConstants` (cam_pos, cam_dir, cam_vup, etc.) right before `queue.submit()`
2. Ensure the log is flushed before submission so it survives a crash
3. When crash occurs, capture the last logged parameters
4. Create a CPU test with those exact parameters to reproduce and debug

This avoids the problem of the crash killing the process before we can inspect state - the write-ahead log will already be on disk.
