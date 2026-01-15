# Scene Definitions

Define scenes in TOML files instead of hardcoded shaders.

## Relevant Files

- `crates/shader/src/scene.rs` - Current hardcoded scene functions
- `crates/rtx-obj/src/lib.rs` - `Sphere`, `Quad`, `List` primitives
- `crates/rtx-mat/src/lib.rs` - `Lambertian`, `Metal`, `Dielectric`, `DiffuseLight` materials
- `crates/rtx-tex/src/lib.rs` - `SolidTexture` and texture table

## Overview

Currently scenes are hardcoded as shader entry points (`cornell_box_fs`, `two_spheres_fs`, etc.). Each scene function builds primitives, materials, and textures at shader runtime. This works but:
- Adding a scene requires recompiling the shader
- Scene parameters can't be tweaked without code changes
- Benchmarks and renders must reference shader entry point names

Scene TOML files would allow defining scenes declaratively. The host parses the TOML and uploads scene data to the GPU via buffers, replacing the hardcoded scene functions with a single generic renderer.

## Status

- [ ] Design TOML schema for scenes
- [ ] GPU buffer layout for scene data
- [ ] Host-side TOML parsing and buffer upload
- [ ] Generic shader entry point that reads from buffers
- [ ] Migrate existing scenes to TOML format

### Future Enhancements

- [ ] Live reload: watch TOML files and re-upload on change
- [ ] Scene includes: reference other TOML files for reusable object groups
- [ ] Procedural generation: loops/randomization in scene definition

## Scene Definition Format

Scenes live in `scenes/<name>.toml`:

```toml
[camera]
# Default camera for this scene (can be overridden by render/benchmark)
position = [278.0, 278.0, -800.0]
look_at = [278.0, 278.0, 0.0]
vfov = 40.0
focus_dist = 10.0
defocus_angle = 0.0

[[materials]]
name = "white"
type = "lambertian"
color = [0.73, 0.73, 0.73]

[[materials]]
name = "red"
type = "lambertian"
color = [0.65, 0.05, 0.05]

[[materials]]
name = "light"
type = "diffuse_light"
color = [15.0, 15.0, 15.0]

[[materials]]
name = "glass"
type = "dielectric"
refraction_index = 1.5

[[materials]]
name = "mirror"
type = "metal"
color = [0.8, 0.8, 0.8]
fuzz = 0.0

[[objects]]
type = "quad"
corner = [555.0, 0.0, 0.0]
u = [0.0, 555.0, 0.0]
v = [0.0, 0.0, 555.0]
material = "white"

[[objects]]
type = "sphere"
center = [190.0, 90.0, 190.0]
radius = 90.0
material = "glass"
```

### Material Types

| Type | Fields |
|------|--------|
| `lambertian` | `color` |
| `metal` | `color`, `fuzz` (0.0-1.0) |
| `dielectric` | `refraction_index` |
| `diffuse_light` | `color` (can exceed 1.0 for brightness) |

### Object Types

| Type | Fields |
|------|--------|
| `sphere` | `center`, `radius`, `material` |
| `quad` | `corner`, `u`, `v`, `material` |

## Integration with Renders and Benchmarks

Render and benchmark TOMLs would reference scenes by name or define inline:

**Reference by name:**
```toml
scene = "cornell_box"  # loads scenes/cornell_box.toml

[camera]
position = [278.0, 278.0, -800.0]
look_at = [278.0, 278.0, 0.0]

[quality]
samples = 100
bounces = 50
```

**Inline scene definition:**
```toml
[camera]
position = [13.0, 2.0, 3.0]
look_at = [0.0, 0.0, 0.0]

[quality]
samples = 50
bounces = 10

[scene]
[[scene.materials]]
name = "ground"
type = "lambertian"
color = [0.5, 0.5, 0.5]

[[scene.objects]]
type = "sphere"
center = [0.0, -1000.0, 0.0]
radius = 1000.0
material = "ground"
```

## GPU Buffer Design

Scene data must be uploaded to the GPU since TOML is parsed on the host. Rough layout:

- **Material buffer**: array of material structs (kind + parameters)
- **Object buffer**: array of object structs (kind + geometry + material index)
- **Texture buffer**: (future) image data for texture mapping

The shader would have a single entry point that reads these buffers instead of calling scene-specific functions. This requires:
1. Defining fixed-size buffer layouts (max objects, max materials)
2. Or using storage buffers with dynamic sizing (if rust-gpu supports)

## Open Questions

- Max object/material counts? Fixed compile-time limits vs dynamic?
- How to handle the `List<NS, NQ>` const generics with dynamic scene sizes?
- Should hardcoded scenes remain as fallbacks or be fully replaced?
- Texture support: inline color only, or path to image files?
