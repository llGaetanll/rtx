use std::error::Error;
use std::time::Instant;

use clap::Parser;
use clap::Subcommand;
use futures::executor::block_on;
use glam::Quat;
use glam::Vec3;
use ouroboros::self_referencing;
use wgpu;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::ElementState;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
use winit::window::Window;
use winit::window::WindowAttributes;
use winit::window::WindowId;

mod camera_path;
mod gpu;
mod spline;

use gpu::GpuContext;

#[derive(Parser)]
#[command(name = "rtx")]
#[command(about = "A GPU ray tracer built with rust-gpu")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a window and render the scene live
    Live {
        /// Which scene to render (fragment shader entry point)
        #[arg(short, long, default_value = "cornell_box_fs")]
        scene: String,
    },
    /// Render all test scenes to a grid image
    Test,
}

#[self_referencing]
struct WindowSurface {
    window: Box<Window>,
    #[borrows(window)]
    #[covariant]
    surface: wgpu::Surface<'this>,
}

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

struct RustShaderSandboxApp {
    scene: String,
    gpu: Option<GpuContext>,
    window_surface: Option<WindowSurface>,
    config: Option<wgpu::SurfaceConfiguration>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    close_requested: bool,
    start: Instant,
    cursor_x: f32,
    cursor_y: f32,
    // Camera state
    cam_pos: Vec3,
    cam_orientation: Quat, // Camera orientation as quaternion
    keys_held: KeysHeld,
    last_cursor_x: f32,
    last_cursor_y: f32,
    last_frame: Instant,
}

impl RustShaderSandboxApp {
    fn new(scene: String) -> Self {
        // Default camera: two_spheres position
        // two_spheres camera: lookfrom (0, 1, 5), lookat (0, 0, 0)
        let cam_pos = Vec3::new(0.0, 1.0, 5.0);

        // Initial orientation: looking slightly down toward origin
        // Start with identity (looking down -Z), then pitch down slightly
        let pitch = -0.197f32; // Slightly down to look at origin
        let cam_orientation = Quat::from_rotation_x(pitch);

        Self {
            scene,
            gpu: None,
            window_surface: None,
            config: None,
            render_pipeline: None,
            close_requested: false,
            start: Instant::now(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cam_pos,
            cam_orientation,
            keys_held: KeysHeld::default(),
            last_cursor_x: 0.0,
            last_cursor_y: 0.0,
            last_frame: Instant::now(),
        }
    }
}

impl RustShaderSandboxApp {
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

        let render_pipeline = gpu.create_pipeline(swapchain_format, &self.scene);

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
        let push_constants = shared::ShaderConstants {
            width: current_size.width,
            height: current_size.height,
            time: self.start.elapsed().as_secs_f32(),
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            cam_pos: self.cam_pos.into(),
            cam_dir: cam_dir.into(),
            cam_vup: cam_vup.into(),
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

impl ApplicationHandler for RustShaderSandboxApp {
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

fn run_live(scene: &str) -> Result<(), Box<dyn Error>> {
    log::debug!("Running live with scene: {}", scene);
    let event_loop = EventLoop::new()?;
    let mut app = RustShaderSandboxApp::new(scene.to_string());
    event_loop.run_app(&mut app).map_err(Into::into)
}

fn run_test() -> Result<(), Box<dyn Error>> {
    log::debug!("Test mode: rendering all scenes to grid image...");

    let instance = GpuContext::create_instance();
    let gpu = block_on(GpuContext::new(instance, None))?;

    // All available scenes (fragment shader entry points)
    let scenes = [
        "cornell_box_fs",
        "quads_fs",
        "metal_test_fs",
        "dielectric_test_fs",
        "two_spheres_fs",
        "glass_debug_fs",
        "three_spheres_fs",
        "many_spheres_fs",
    ];

    // 720p per scene
    let scene_width = 1280u32;
    let scene_height = 720u32;

    // 4x4 grid
    let grid_cols = 4u32;
    let grid_rows = 4u32;
    let grid_width = scene_width * grid_cols;
    let grid_height = scene_height * grid_rows;

    // Create the final grid image with checkerboard background
    let mut grid_img = image::RgbaImage::new(grid_width, grid_height);

    // Fill with checkerboard pattern for empty slots
    let color_a = image::Rgba([0x17, 0x1d, 0x1c, 0xff]);
    let color_b = image::Rgba([0x3f, 0x50, 0x4d, 0xff]);
    let checker_size = 128u32;

    for y in 0..grid_height {
        for x in 0..grid_width {
            let checker_x = x / checker_size;
            let checker_y = y / checker_size;
            let color = if (checker_x + checker_y) % 2 == 0 {
                color_a
            } else {
                color_b
            };
            grid_img.put_pixel(x, y, color);
        }
    }

    // Render each scene and place in grid (top to bottom, left to right)
    for (i, scene) in scenes.iter().enumerate() {
        let col = (i as u32) % grid_cols;
        let row = (i as u32) / grid_cols;

        let start = Instant::now();
        let pixels = block_on(gpu.render_to_image(scene_width, scene_height, scene));
        let elapsed = start.elapsed();

        log::debug!(
            "Rendered {} ({}/{}) in {:.2?}",
            scene,
            i + 1,
            scenes.len(),
            elapsed
        );
        let scene_img = image::RgbaImage::from_raw(scene_width, scene_height, pixels)
            .expect("Failed to create image from pixel data");

        // Copy scene image into grid
        let x_offset = col * scene_width;
        let y_offset = row * scene_height;
        for y in 0..scene_height {
            for x in 0..scene_width {
                let pixel = scene_img.get_pixel(x, y);
                grid_img.put_pixel(x + x_offset, y + y_offset, *pixel);
            }
        }
    }

    std::fs::create_dir_all("renders")?;
    let path = "renders/render.png";

    log::debug!("Saving {}x{} grid to {}...", grid_width, grid_height, path);
    grid_img.save(path)?;

    log::info!("Saved {}", path);
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Live { scene }) => run_live(&scene),
        Some(Commands::Test) => run_test(),
        None => run_live("cornell_box_fs"),
    }
}
