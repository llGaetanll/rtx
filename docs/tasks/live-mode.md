# Interactive Camera Controls (Live Mode)

Keyboard and mouse controls to move the camera in real-time during `live` mode.

## Overview

The camera state lives on the CPU and is passed to the shader each frame via `ShaderConstants`. The host tracks orientation using quaternions to avoid gimbal lock.

## Relevant Files

- `crates/host/src/live_app.rs` - `LiveApp` with camera state, input handling, `update_camera()`
- `crates/shared/src/lib.rs` - `ShaderConstants` with `cam_pos`, `cam_dir`, `cam_vup`

## Controls

- WASD: horizontal movement (forward/back/strafe)
- Space/C: up/down
- Mouse: look direction
- Q/Escape: quit

## Status

### Phase 1: CPU-side input and logging (no shader changes)

- [x] Add camera state to app struct (position, yaw, pitch)
- [x] Track held keys (simple bools for W/A/S/D/Space/C)
- [x] Track mouse delta for look direction
- [x] Each frame: update position based on held keys and orientation, update orientation from mouse
- [x] Log computed camera params to stdout
- [x] Q key exits the loop

### Phase 2: Wire up to shader

- [x] Add camera fields to `ShaderConstants` (cam_pos, cam_dir, cam_fov, etc.)
- [x] Modify shader to construct `Camera` from `ShaderConstants` instead of hardcoded values
- [x] Remove or bypass per-scene camera construction

### Phase 3: Fix gimbal lock with quaternions

The camera previously crashed when looking straight up or down because `vup` became parallel to the view direction. This was fixed with quaternion-based orientation.

**Stage 1: Add `vup` to data path**
- [x] Add `vup: [f32; 3]` to `ShaderConstants`
- [x] Thread `vup` through `two_spheres()` and `CameraParams`
- [x] Host computes `vup` dynamically
- [x] Update tests to pass with dynamic `vup`

**Stage 2: Switch host to quaternions**
- [x] Add `glam` crate dependency to host (for `Quat`)
- [x] Replace `cam_yaw: f32` / `cam_pitch: f32` with `cam_orientation: Quat`
- [x] Rewrite `update_camera()`:
  - Apply yaw rotation in world space (around world Y axis)
  - Apply pitch rotation in local space (around camera's right axis)
  - Multiply rotations into orientation quaternion
- [x] Extract `cam_dir` and `vup` from quaternion for shader
- [x] Remove pitch clamping (quaternions handle full rotation)
- [x] Tune mouse sensitivity for natural feel

## Future Enhancements

- [ ] **Dynamic ray sampling**: Lower samples per pixel when camera/world is moving for faster feedback, then accumulate rays over time when stationary for higher quality. Requires tracking frame-to-frame camera changes and maintaining an accumulation buffer.
- [ ] Screenshot with current camera position
