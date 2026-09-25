use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use futures::executor::block_on;
use glam::Mat3;
use glam::Quat;
use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::ElementState;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
use winit::window::WindowAttributes;
use winit::window::WindowId;

use crate::blit::Blit;
use crate::config::ImageConfig;
use crate::gpu::Accumulator;
use crate::gpu::GpuContext;
use crate::gpu::SceneBuffers;
use crate::gpu::Tiling;
use crate::scene_data;
use crate::window_surface::WindowSurface;
use crate::window_surface::WindowSurfaceBuilder;

/// How often the window title's frame rate and sample count are refreshed.
/// Every frame would be unreadable, and on some compositors not free.
const TITLE_INTERVAL: Duration = Duration::from_millis(250);

/// How long the camera has to be still before a view traced at
/// `[live].moving_scale` goes back to full resolution.
///
/// Mouse movement does not arrive every frame, so a drag has frames in it where
/// the camera did not move. Switching on each of those would flicker between
/// the two resolutions for as long as the drag lasts.
const MOVING_HOLD: Duration = Duration::from_millis(100);

/// Tracks which movement keys are currently held
#[derive(Default)]
struct KeysHeld {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    space: bool,
    c: bool,
}

impl KeysHeld {
    fn any(&self) -> bool {
        self.w || self.a || self.s || self.d || self.space || self.c
    }
}

pub struct LiveApp {
    /// The config this view started from. Its camera is only a starting point,
    /// but its lens settings keep applying as the camera is flown around.
    image: ImageConfig,
    scene_path: PathBuf,
    gpu: Option<GpuContext>,
    config: Option<wgpu::SurfaceConfiguration>,
    /// Where frames are summed while the camera is still. Sized to the window.
    accumulator: Option<Accumulator>,
    blit: Option<Blit>,
    scene_buffers: Option<SceneBuffers>,
    scene: crate::scene_data::SceneInfo,
    /// Declared after everything holding a GPU handle. Fields drop in declaration
    /// order, and the surface has to go before the window it borrows.
    window_surface: Option<WindowSurface>,
    close_requested: bool,
    start: Instant,
    cursor_x: f32,
    cursor_y: f32,
    // Camera state
    cam_pos: Vec3,
    cam_orientation: Quat,
    keys_held: KeysHeld,
    last_cursor_x: f32,
    last_cursor_y: f32,
    last_frame: Instant,
    /// The camera the accumulated frames were drawn from. Any other camera
    /// sees a different picture, so they are thrown away when this changes.
    drawn_pos: Vec3,
    drawn_orientation: Quat,
    /// When the title was last refreshed, and frames drawn since.
    title_updated: Instant,
    frames_since_title: u32,
    /// The still view has all the samples `[live].max_samples` asks for, so
    /// nothing is traced until the camera moves again.
    converged: bool,
    /// When the camera last moved, which decides whether the view is traced at
    /// the lower moving resolution. `None` until it first moves.
    last_moved: Option<Instant>,
}

impl LiveApp {
    pub fn new(config: ImageConfig, scene_path: PathBuf) -> Self {
        let camera = config.camera;
        let cam_pos = Vec3::from(camera.position);

        Self {
            image: config,
            scene_path,
            gpu: None,
            window_surface: None,
            config: None,
            accumulator: None,
            blit: None,
            scene_buffers: None,
            scene: Default::default(),
            close_requested: false,
            start: Instant::now(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cam_pos,
            cam_orientation: orientation(camera),
            keys_held: KeysHeld::default(),
            last_cursor_x: 0.0,
            last_cursor_y: 0.0,
            last_frame: Instant::now(),
            drawn_pos: cam_pos,
            drawn_orientation: orientation(camera),
            title_updated: Instant::now(),
            frames_since_title: 0,
            converged: false,
            last_moved: None,
        }
    }

    fn update_camera(&mut self) {
        let now = Instant::now();
        // Capped, because a converged view stops drawing frames. Without it the
        // first key press after a pause would move the camera as though it had
        // been held for the whole of that pause
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        // Mouse look: compute delta from last cursor position
        // Skip if last_cursor is uninitialized (first frame)
        let first_frame = self.last_cursor_x == 0.0 && self.last_cursor_y == 0.0;
        let mouse_dx = if first_frame {
            0.0
        } else {
            self.cursor_x - self.last_cursor_x
        };
        let mouse_dy = if first_frame {
            0.0
        } else {
            self.cursor_y - self.last_cursor_y
        };
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;

        // Update orientation from mouse using quaternion rotations
        let sensitivity = 0.003;

        // Yaw: rotate around world Y axis (allows full 360° horizontal rotation)
        let yaw_delta = Quat::from_rotation_y(-mouse_dx * sensitivity);

        // Pitch: rotate around camera's local X (right) axis
        let pitch_delta = Quat::from_rotation_x(-mouse_dy * sensitivity);

        // Apply yaw in world space (pre-multiply), pitch in local space (post-multiply)
        self.cam_orientation = yaw_delta * self.cam_orientation * pitch_delta;

        // Normalize to prevent drift from floating point errors
        self.cam_orientation = self.cam_orientation.normalize();

        // Extract forward and right vectors from orientation for movement
        // Camera looks down -Z in its local space, so forward = orientation * -Z
        let forward = self.cam_orientation * Vec3::NEG_Z;
        let right = self.cam_orientation * Vec3::X;

        // For movement, use only horizontal components (project onto XZ plane)
        let forward_horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_horizontal = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        // Movement speed (units per second)
        let speed = 5.0 * dt;

        // Apply movement based on held keys
        if self.keys_held.w {
            self.cam_pos += forward_horizontal * speed;
        }
        if self.keys_held.s {
            self.cam_pos -= forward_horizontal * speed;
        }
        if self.keys_held.a {
            self.cam_pos -= right_horizontal * speed;
        }
        if self.keys_held.d {
            self.cam_pos += right_horizontal * speed;
        }
        if self.keys_held.space {
            self.cam_pos.y += speed;
        }
        if self.keys_held.c {
            self.cam_pos.y -= speed;
        }

        // Log camera params
        let cam_dir = self.cam_dir();
        log::debug!(
            "Camera: pos=({:.1}, {:.1}, {:.1}) dir=({:.2}, {:.2}, {:.2})",
            self.cam_pos.x,
            self.cam_pos.y,
            self.cam_pos.z,
            cam_dir.x,
            cam_dir.y,
            cam_dir.z,
        );
    }

    /// Get camera forward direction from quaternion
    fn cam_dir(&self) -> Vec3 {
        self.cam_orientation * Vec3::NEG_Z
    }

    /// Get camera up vector from quaternion
    fn cam_vup(&self) -> Vec3 {
        self.cam_orientation * Vec3::Y
    }

    async fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("rtx live")
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window_box = event_loop.create_window(window_attributes)?;

        let instance = GpuContext::create_instance();

        let window_surface = WindowSurfaceBuilder {
            window: Box::new(window_box),
            surface_builder: |window| {
                instance
                    .create_surface(window)
                    .expect("Failed to create surface")
            },
        }
        .build();

        window_surface.borrow_window().set_cursor_visible(false);
        let window_size = window_surface.borrow_window().inner_size();
        let surface = window_surface.borrow_surface();

        let gpu = GpuContext::new(instance, Some(surface)).await?;

        let swapchain_format = crate::blit::surface_format(surface, &gpu.adapter);

        // Never zero sized: a window can be created minimized, and a texture
        // cannot be
        let width = window_size.width.max(1);
        let height = window_size.height.max(1);
        let accumulator = Accumulator::tiled(&gpu, &Tiling::new(&gpu.device, width, height))?;
        let blit = Blit::new(&gpu, accumulator.view(), swapchain_format);

        let scene = scene_data::load(&self.scene_path)?;
        self.scene = scene.info();
        let scene_buffers = gpu.upload_scene(&scene);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: Default::default(),
        };
        surface.configure(&gpu.device, &config);

        self.gpu = Some(gpu);
        self.window_surface = Some(window_surface);
        self.config = Some(config);
        self.accumulator = Some(accumulator);
        self.blit = Some(blit);
        self.scene_buffers = Some(scene_buffers);
        self.start = Instant::now();
        Ok(())
    }

    fn render(&mut self) {
        // Update camera state before rendering
        self.update_camera();
        let cam_dir = self.cam_dir();
        let cam_vup = self.cam_vup();

        let (Some(window_surface), Some(gpu), Some(config)) =
            (&self.window_surface, &self.gpu, &self.config)
        else {
            return;
        };
        let (Some(accumulator), Some(blit), Some(scene_buffers)) =
            (&mut self.accumulator, &self.blit, &self.scene_buffers)
        else {
            return;
        };

        // A minimized window has nothing to draw into
        if config.width == 0 || config.height == 0 {
            return;
        }

        // What has been summed so far only belongs to the camera it was drawn
        // from. Without accumulation every frame starts over, which is live mode
        // as it was before
        let moved =
            self.cam_pos != self.drawn_pos || self.cam_orientation != self.drawn_orientation;
        if moved || !self.image.live.accumulate {
            accumulator.reset();
            self.drawn_pos = self.cam_pos;
            self.drawn_orientation = self.cam_orientation;
        }
        if moved {
            self.last_moved = Some(Instant::now());
        }

        // Coarser while moving, if the config asks for it. Changing size empties
        // the accumulator, so a view that has just stopped starts refining again
        // at full resolution rather than stretching what it had
        let live = self.image.live;
        let low_res = live.moving_scale < 1.0
            && self
                .last_moved
                .is_some_and(|moved| moved.elapsed() < MOVING_HOLD);
        let (width, height) = if low_res {
            (
                scaled(config.width, live.moving_scale),
                scaled(config.height, live.moving_scale),
            )
        } else {
            (config.width, config.height)
        };
        accumulator.set_image_size(width, height);

        let surface = window_surface.borrow_surface();
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("Error getting next frame: {e:?}");
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // The camera has been flown away from where the config put it, so only
        // its lens settings come from there. The accumulator fills in the size
        // and seed
        let constants = shared::ShaderConstants {
            time: self.start.elapsed().as_secs_f32(),
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            cam_pos: self.cam_pos.into(),
            cam_dir: cam_dir.into(),
            cam_vup: cam_vup.into(),
            ..self
                .image
                .camera
                .constants(config.width, config.height, live.quality(), self.scene)
        };

        // A still view that already has every sample it asked for is only shown
        // again, which is what lets the GPU idle
        let samples_before = accumulator.passes_done() * live.samples;
        let capped = live.max_samples > 0 && samples_before >= live.max_samples;
        if !capped {
            accumulator.record_pass(&mut encoder, scene_buffers, &constants, live.samples);
        }

        let passes = accumulator.passes_done();
        let samples = passes * live.samples;
        // Not while still coarse: the view has yet to go back to full size, and
        // an idle event loop would never draw the frame that does it
        self.converged = !low_res && live.max_samples > 0 && samples >= live.max_samples;
        blit.draw(
            &mut encoder,
            &view,
            &shared::BlitConstants {
                // Smaller than the window while moving, which the blit
                // stretches to fill it
                image_width: width,
                image_height: height,
                surface_width: config.width,
                surface_height: config.height,
                scale: 1.0 / passes as f32,
            },
        );

        gpu.queue.submit(Some(encoder.finish()));
        frame.present();

        self.frames_since_title += 1;
        let since = self.title_updated.elapsed();
        if self.converged {
            // Frames stop here, so this is the last chance to say so, and a
            // frame rate would describe frames that are no longer being drawn
            window_surface
                .borrow_window()
                .set_title(&format!("rtx live: converged, {samples} samples/px"));
        } else if since >= TITLE_INTERVAL {
            let fps = self.frames_since_title as f32 / since.as_secs_f32();
            window_surface
                .borrow_window()
                .set_title(&format!("rtx live: {fps:.0} fps, {samples} samples/px"));
            self.title_updated = Instant::now();
            self.frames_since_title = 0;
        }
    }
}

impl ApplicationHandler for LiveApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = block_on(self.init(event_loop)) {
            eprintln!("Initialization error: {e}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let redraw = matches!(event, WindowEvent::RedrawRequested);

        match event {
            WindowEvent::CloseRequested => self.close_requested = true,
            WindowEvent::Resized(new_size) => {
                if let Some(config) = self.config.as_mut() {
                    config.width = new_size.width;
                    config.height = new_size.height;
                    // Minimized. Neither a surface nor a texture can be zero
                    // sized, and render skips drawing until this changes
                    let visible = new_size.width > 0 && new_size.height > 0;
                    if let (true, Some(ws), Some(gpu)) = (visible, &self.window_surface, &self.gpu)
                    {
                        ws.borrow_surface().configure(&gpu.device, config);

                        // A different size is a different picture, so the
                        // sum starts again at the new resolution
                        if let (Some(accumulator), Some(blit)) =
                            (&mut self.accumulator, &mut self.blit)
                        {
                            accumulator.resize(gpu, new_size.width, new_size.height);
                            blit.rebind(gpu, accumulator.view());
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_x = position.x as f32;
                self.cursor_y = position.y as f32;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        if pressed {
                            self.close_requested = true;
                        }
                    }
                    Key::Named(NamedKey::Space) => self.keys_held.space = pressed,
                    Key::Character(c) => match c.as_str() {
                        "w" | "W" => self.keys_held.w = pressed,
                        "a" | "A" => self.keys_held.a = pressed,
                        "s" | "S" => self.keys_held.s = pressed,
                        "d" | "D" => self.keys_held.d = pressed,
                        "c" | "C" => self.keys_held.c = pressed,
                        "q" | "Q" => {
                            if pressed {
                                self.close_requested = true;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }

        if self.close_requested {
            event_loop.exit();
        } else if !redraw && let Some(ws) = &self.window_surface {
            // Any input may have moved the camera, and a converged view is
            // not drawing frames on its own to notice. Not after a redraw,
            // which would keep an idle view drawing forever
            ws.borrow_window().request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.close_requested {
            event_loop.exit();
            return;
        }

        // A converged view with nothing held has nothing new to draw, so wait
        // for input instead of spinning. Input arrives as a window event, which
        // asks for the frame that notices it
        if self.converged && !self.keys_held.any() {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            if let Some(ws) = &self.window_surface {
                ws.borrow_window().request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }
}

/// A window dimension at a fraction of its size, never less than one pixel.
fn scaled(size: u32, scale: f32) -> u32 {
    ((size as f32 * scale).round() as u32).max(1)
}

/// The orientation looking from a camera's position towards what it looks at.
/// `live` steers with a quaternion rather than a target point, so the config's
/// `look_at` only decides where the view starts.
fn orientation(camera: crate::config::Camera) -> Quat {
    let forward = Vec3::from(camera.direction()).normalize_or(Vec3::NEG_Z);
    let right = forward
        .cross(Vec3::from(camera.vup))
        .try_normalize()
        .unwrap_or(Vec3::X);
    let back = -forward;

    Quat::from_mat3(&Mat3::from_cols(right, back.cross(right), back)).normalize()
}

pub fn run_live(scene_path: &Path, config_path: &Path) -> Result<(), Box<dyn Error>> {
    let config = ImageConfig::load(config_path)?;
    log::debug!("Running live with scene: {}", scene_path.display());

    let event_loop = EventLoop::new()?;
    let mut app = LiveApp::new(config, scene_path.to_path_buf());
    event_loop.run_app(&mut app).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Camera;

    fn camera(position: [f32; 3], look_at: [f32; 3]) -> Camera {
        Camera {
            position,
            look_at,
            vup: [0.0, 1.0, 0.0],
            fov: 40.0,
            defocus_angle: 0.0,
            focus_dist: 10.0,
        }
    }

    /// The starting view has to be the one the config describes, and getting the
    /// handedness wrong would mirror the scene rather than fail.
    #[test]
    fn orientation_faces_the_target() {
        for (position, look_at) in [
            ([278.0, 278.0, -800.0], [278.0, 278.0, 0.0]),
            ([13.0, 2.0, 3.0], [0.0, 0.0, 0.0]),
            ([0.0, 2.0, 5.0], [0.0, 0.5, 0.0]),
        ] {
            let camera = camera(position, look_at);
            let rotation = orientation(camera);

            let wanted = Vec3::from(camera.direction()).normalize();
            let facing = rotation * Vec3::NEG_Z;
            assert!(
                (facing - wanted).length() < 1e-5,
                "looking {facing} instead of {wanted}"
            );

            let up = rotation * Vec3::Y;
            assert!(up.dot(Vec3::Y) > 0.0, "upside down: {up}");
            assert!(up.dot(facing).abs() < 1e-5, "up is not perpendicular: {up}");
        }
    }
}
