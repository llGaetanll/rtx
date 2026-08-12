/// A scene and the camera it is meant to be viewed from.
///
/// The shader has no camera defaults of its own: every camera and quality
/// setting reaches it through push constants, so these are the values `live`,
/// `test` and `bench` start from.
pub struct Scene {
    /// Fragment shader entry point
    pub name: &'static str,
    pub position: [f32; 3],
    pub look_at: [f32; 3],
    pub vup: [f32; 3],
    pub fov: f32,
    pub defocus_angle: f32,
    pub focus_dist: f32,
}

/// Rays per pixel for the interactive and grid renderers. `render` sets its own.
pub const SAMPLES: u32 = 40;

/// Maximum ray bounce depth for the interactive and grid renderers.
pub const BOUNCES: u32 = 10;

const UP: [f32; 3] = [0.0, 1.0, 0.0];
const ORIGIN: [f32; 3] = [0.0, 0.0, 0.0];

pub const SCENES: [Scene; 8] = [
    Scene {
        name: "cornell_box_fs",
        position: [278.0, 278.0, -800.0],
        look_at: [278.0, 278.0, 0.0],
        vup: UP,
        fov: 40.0,
        defocus_angle: 0.0,
        focus_dist: 10.0,
    },
    Scene {
        name: "quads_fs",
        position: [0.0, 0.0, 9.0],
        look_at: ORIGIN,
        vup: UP,
        fov: 80.0,
        defocus_angle: 0.0,
        focus_dist: 10.0,
    },
    Scene {
        name: "metal_test_fs",
        position: [0.0, 1.0, 5.0],
        look_at: ORIGIN,
        vup: UP,
        fov: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
    },
    Scene {
        name: "dielectric_test_fs",
        position: [0.0, 1.0, 5.0],
        look_at: ORIGIN,
        vup: UP,
        fov: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
    },
    Scene {
        name: "two_spheres_fs",
        position: [0.0, 1.0, 5.0],
        look_at: ORIGIN,
        vup: UP,
        fov: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
    },
    Scene {
        name: "glass_debug_fs",
        position: [0.0, 2.0, 5.0],
        look_at: [0.0, 0.5, 0.0],
        vup: UP,
        fov: 40.0,
        defocus_angle: 0.0,
        focus_dist: 5.0,
    },
    Scene {
        name: "three_spheres_fs",
        position: [13.0, 2.0, 3.0],
        look_at: ORIGIN,
        vup: UP,
        fov: 20.0,
        defocus_angle: 0.6,
        focus_dist: 10.0,
    },
    Scene {
        name: "many_spheres_fs",
        position: [13.0, 2.0, 3.0],
        look_at: ORIGIN,
        vup: UP,
        fov: 20.0,
        defocus_angle: 0.6,
        focus_dist: 10.0,
    },
];

impl Scene {
    /// Look up a scene by entry point name.
    pub fn find(name: &str) -> Option<&'static Scene> {
        SCENES.iter().find(|scene| scene.name == name)
    }

    /// Direction from the camera position to its target.
    pub fn direction(&self) -> [f32; 3] {
        [
            self.look_at[0] - self.position[0],
            self.look_at[1] - self.position[1],
            self.look_at[2] - self.position[2],
        ]
    }

    /// Push constants viewing this scene from its own camera at the given size.
    /// The background belongs to the scene, so it is passed in alongside.
    pub fn constants(
        &self,
        width: u32,
        height: u32,
        background: [f32; 3],
    ) -> shared::ShaderConstants {
        shared::ShaderConstants {
            width,
            height,
            time: 0.0,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cam_pos: self.position,
            cam_dir: self.direction(),
            cam_vup: self.vup,
            fov_v: self.fov,
            defocus_angle: self.defocus_angle,
            focus_dist: self.focus_dist,
            px_samples: SAMPLES,
            max_ray_bounce: BOUNCES,
            seed: 0,
            background,
        }
    }
}

/// Every available scene name.
pub fn names() -> impl Iterator<Item = &'static str> {
    SCENES.iter().map(|scene| scene.name)
}
