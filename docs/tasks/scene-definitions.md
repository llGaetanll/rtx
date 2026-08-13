# Scene Definitions

Scenes, images and videos are all TOML files.

## Relevant Files

- `scenes/*.toml` - The scenes
- `configs/image/*.toml`, `configs/video/*.toml` - The cameras looking at them
- `crates/host/src/scene_data.rs` - Scene parsing, and `SceneData` as the GPU reads it
- `crates/host/src/config.rs` - The image and video config formats
- `crates/host/src/render_app.rs`, `live_app.rs`, `bench_app.rs`, `main.rs` - The commands that read them
- `crates/rtx-bench/src/spline.rs` - `CatmullRomSpline`, which every animated camera field rides

## Overview

Scenes used to be built inside the shader, once per pixel per frame. They moved
to the host and were uploaded as buffers, which meant nothing about a scene had
to be known at shader compile time any more; this task was the remaining step of
not writing them in Rust either.

Two ideas drive the layout:

**A scene describes what exists, never where it is viewed from.** Cameras belong
to whatever is doing the looking, so the same scene can back a still image, a
benchmark fly-through and a video without being copied. A scene file therefore
has materials, objects and a background, and nothing else.

**Neither file names the other.** A config holds a camera and nothing that says
what to point it at, and the two files are given to a command as paths. Nothing
resolves a name against a directory the program knows about, so a scene and a
config are ordinary files that can live anywhere and be paired freely.

**An image and a video differ only in whether the camera moves.** They share one
config shape, distinguished by a `type` at the top and by whether the camera's
fields hold a single value or a list of them.

## Layout

The directories are a convention, not a lookup path:

```
scenes/       what exists
configs/image a still camera looking at something
configs/video a moving camera looking at something
bench.toml    which scene each benchmark camera flies through
```

Every command takes both files as paths:

```sh
cargo run --release -- render -s scenes/cornell_box.toml -c configs/image/cornell_box.toml
```

| Command | Uses of the config |
|---------|--------------------|
| `live -s <scene> -c <config>` | camera as the starting view; its own resolution and sample count |
| `render -s <scene> -c <config>` | everything |
| `bench -s <scene> -c <config>` | camera path and quality; records frame times instead of writing frames |
| `bench` | the same, for every pair in `bench.toml` |

`bench` reading video configs is the point of the split rather than an accident
of it: a benchmark is a video whose frames are timed and thrown away. Running
every benchmark still needs the pairs written down somewhere, which is what
`bench.toml` is:

```toml
[[benchmark]]
scene = "scenes/cornell_box.toml"
config = "configs/video/cornell_box.toml"
```

The name a render or a benchmark result is filed under is the config's file name
without its extension.

## Scene Format

```toml
# scenes/cornell_box.toml
background = [0.0, 0.0, 0.0]

[materials.white]
type = "lambertian"
color = [0.73, 0.73, 0.73]

[[objects]]
name = "floor"
type = "quad"
material = "white"
corner = [0.0, 0.0, 0.0]
u = [555.0, 0.0, 0.0]
v = [0.0, 0.0, 555.0]
```

Materials are a table keyed by name, so an object refers to one without
repeating it. Objects are an array, each naming the material it uses. The `name`
on an object is documentation; nothing looks it up.

### Materials

| Type | Fields |
|------|--------|
| `lambertian` | `color` |
| `metal` | `color`, `fuzz` (0.0-1.0) |
| `dielectric` | `ior` |
| `diffuse_light` | `color`, values above 1.0 make it a light rather than a bright surface |

### Objects

| Type | Fields |
|------|--------|
| `sphere` | `center`, `radius` |
| `quad` | `corner`, `u`, `v` |
| `box` | `min`, `max`, optional `rotate_y` (degrees), optional `translate` |

A quad is a corner and the two edge vectors leading away from it, so it is any
parallelogram rather than only an axis aligned rectangle; the order of `u` and
`v` decides which way it faces. A box expands to six quads, with `rotate_y`
applied first about the origin and `translate` afterwards.

## Config Format

Both kinds share a shape. `type` says which one it is, so a config can be read
before it is known what to do with it.

```toml
# configs/image/cornell_box.toml
type = "image"

[camera]
position = [278.0, 278.0, -800.0]
look_at = [278.0, 278.0, 0.0]
vup = [0.0, 1.0, 0.0]
fov = 40.0
defocus_angle = 0.0
focus_dist = 10.0

[quality]
samples = 500
bounces = 50

[output]
width = 1920
height = 1080
```

A video is the same file with a camera that moves:

```toml
# configs/video/cornell_box.toml
type = "video"

[camera]
# A list is keyframes on a spline; a plain value is held for the whole path.
position = [
    [278.0, 278.0, -800.0],
    [278.0, 278.0, -600.0],
    [278.0, 300.0, -400.0],
    [200.0, 278.0, -200.0],
]
look_at = [278.0, 278.0, 0.0]
vup = [0.0, 1.0, 0.0]
fov = 40.0
defocus_angle = 0.0
focus_dist = 10.0

[quality]
samples = 8
bounces = 10

[output]
width = 400
height = 300
frames = 60
```

There is one `[camera]` section, not a static one and an animated one. Any field
in it accepts either a single value or a list of values; a list is interpolated
across the frames, a single value is constant. An image config rejects lists,
since it has no frames to spread them over. The spline needs at least four
keyframes, so a list shorter than that is an error.

`frames` belongs to `[output]` because it is a property of the thing being
produced, alongside its width and height. It is required for a video and
rejected for an image.

## Phases

### Phase 1: Load scenes from TOML

- [x] Scene file types and TOML parsing in `scene_data.rs`
- [x] Build `SceneData` from a parsed file: materials by name, objects to instances, boxes to quads
- [x] Clear errors for unknown material names, unknown types and bad values
- [x] Delete the hardcoded scene builder functions
- [x] Tests: every scene in `scenes/` loads, and material references stay in range
- [x] Check the rendered output still matches, scene by scene

### Phase 2: Image configs

- [x] `configs/image/<name>.toml` for all eight scenes, cameras taken from the `SCENES` table
- [x] Config module shared by the commands, with `type` dispatch
- [x] `render` reads the new location, `renders/configs/` goes away
- [x] `live` and `test` take their cameras from image configs
- [x] Delete the `SCENES` table; scene and config names come from the directories

### Phase 3: Video configs

- [x] Camera fields that accept a value or a list of values
- [x] `configs/video/<name>.toml` for the three existing benchmarks
- [x] `bench` reads video configs, `bench/configs/` goes away
- [x] Benchmark cameras stop borrowing fov and focus from the scene table

## Future Work

- Render a video config to frames, and encode them. The format is in place and
  `bench` already flies its cameras; nothing writes the frames out yet.
- Live reload: watch the TOML files and re-upload on change
- Scene includes: reference other TOML files for reusable object groups
- Procedural generation: loops or randomization in a scene definition
- Textures beyond solid colors, which will need a path to an image file
