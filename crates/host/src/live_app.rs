use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
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

use crate::config::ImageConfig;
use crate::gpu::GpuContext;
use crate::gpu::SceneBuffers;
use crate::scene_data;
use crate::window_surface::WindowSurface;
use crate::window_surface::WindowSurfaceBuilder;

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

pub struct LiveApp {
    /// The config this view started from. Its camera is only a starting point,
    /// but its lens settings keep applying as the camera is flown around.
    image: ImageConfig,
    scene_path: PathBuf,
    gpu: Option<GpuContext>,
    config: Option<wgpu::SurfaceConfiguration>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    scene_buffers: Option<SceneBuffers>,
    background: [f32; 3],
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
            render_pipeline: None,
            scene_buffers: None,
            background: [0.; 3],
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
        }
    }

    fn update_camera(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
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
            .with_title("Rust Shader Sandbox")
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

        let swapchain_format = surface.get_capabilities(&gpu.adapter).formats[0];

        let render_pipeline = gpu.create_pipeline(swapchain_format);

        let scene = scene_data::load(&self.scene_path)?;
        self.background = scene.background;
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
        self.render_pipeline = Some(render_pipeline);
        self.scene_buffers = Some(scene_buffers);
        self.start = Instant::now();
        Ok(())
    }

    fn render(&mut self) {
        // Update camera state before rendering
        self.update_camera();

        let window_surface = match &self.window_surface {
            Some(ws) => ws,
            None => return,
        };
        let gpu = match &self.gpu {
            Some(gpu) => gpu,
            None => return,
        };

        let window = window_surface.borrow_window();
        let current_size = window.inner_size();
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

        let cam_dir = self.cam_dir();
        let cam_vup = self.cam_vup();
        // The camera has been flown away from where the config put it, so only
        // its lens settings come from there
        let push_constants = shared::ShaderConstants {
            time: self.start.elapsed().as_secs_f32(),
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            cam_pos: self.cam_pos.into(),
            cam_dir: cam_dir.into(),
            cam_vup: cam_vup.into(),
            ..self.image.camera.constants(
                current_size.width,
                current_size.height,
                ImageConfig::preview_quality(),
                self.background,
            )
        };

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(self.render_pipeline.as_ref().unwrap());
            rpass.set_bind_group(0, &self.scene_buffers.as_ref().unwrap().bind_group, &[]);
            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::bytes_of(&push_constants),
            );
            rpass.draw(0..3, 0..1);
        }

        gpu.queue.submit(Some(encoder.finish()));
        frame.present();
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
        match event {
            WindowEvent::CloseRequested => self.close_requested = true,
            WindowEvent::Resized(new_size) => {
                if let Some(config) = self.config.as_mut() {
                    config.width = new_size.width;
                    config.height = new_size.height;
                    if let Some(ws) = &self.window_surface {
                        let surface = ws.borrow_surface();
                        if let Some(gpu) = &self.gpu {
                            surface.configure(&gpu.device, config);
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
        } else if let Some(ws) = &self.window_surface {
            ws.borrow_window().request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.close_requested {
            event_loop.exit();
        } else if let Some(ws) = &self.window_surface {
            ws.borrow_window().request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }
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
