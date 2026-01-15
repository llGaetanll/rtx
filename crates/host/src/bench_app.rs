use std::error::Error;
use std::time::Instant;

use futures::executor::block_on;
use glam::Vec3;
use serde::Serialize;
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

use crate::camera_path::CameraPath;
use crate::gpu::GpuContext;
use crate::window_surface::WindowSurface;
use crate::window_surface::WindowSurfaceBuilder;

/// GPU information captured from the wgpu adapter.
#[derive(Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub driver: String,
    pub backend: String,
}

/// Per-frame timing and camera data.
#[derive(Serialize)]
pub struct FrameRecord {
    pub frame: u32,
    pub t: f32,
    pub time_us: u64,
    pub cam_pos: [f32; 3],
    pub cam_dir: [f32; 3],
    pub cam_vup: [f32; 3],
}

impl GpuInfo {
    /// Extract GPU info from a wgpu adapter.
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let info = adapter.get_info();
        Self {
            name: info.name,
            driver: info.driver,
            backend: format!("{:?}", info.backend),
        }
    }
}

/// Git SHA baked in at build time via build.rs.
const GIT_SHA: &str = env!("GIT_SHA");

/// Application for benchmark mode with animated camera path.
pub struct BenchApp {
    gpu: Option<GpuContext>,
    gpu_info: Option<GpuInfo>,
    window_surface: Option<WindowSurface>,
    config: Option<wgpu::SurfaceConfiguration>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    close_requested: bool,
    start: Instant,
    camera_path: CameraPath,
    frame_records: Vec<FrameRecord>,
    frame_count: u32,
}

impl BenchApp {
    pub fn new() -> Self {
        // Hardcoded camera path for two_spheres scene
        // Camera orbits around the scene, looking at the origin
        let position_points = vec![
            Vec3::new(5.0, 2.0, 5.0),   // Start: front-right
            Vec3::new(5.0, 1.5, 0.0),   // Right side
            Vec3::new(5.0, 2.0, -5.0),  // Back-right
            Vec3::new(0.0, 3.0, -6.0),  // Back center, higher
            Vec3::new(-5.0, 2.0, -5.0), // Back-left
            Vec3::new(-5.0, 1.5, 0.0),  // Left side
            Vec3::new(-5.0, 2.0, 5.0),  // Front-left
            Vec3::new(0.0, 1.0, 7.0),   // Front center, lower
            Vec3::new(5.0, 2.0, 5.0),   // Back to start
            Vec3::new(5.0, 1.5, 0.0),   // (duplicate for spline)
        ];

        // Look at origin throughout, with slight vertical variation
        let look_at_points = vec![
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 0.3, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 0.8, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 0.3, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 0.3, 0.0),
        ];

        let camera_path = CameraPath::new(position_points, look_at_points, 10.0);

        Self {
            gpu: None,
            gpu_info: None,
            window_surface: None,
            config: None,
            render_pipeline: None,
            close_requested: false,
            start: Instant::now(),
            camera_path,
            frame_records: Vec::new(),
            frame_count: 0,
        }
    }

    async fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("RTX Benchmark")
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

        let window_size = window_surface.borrow_window().inner_size();
        let surface = window_surface.borrow_surface();

        let gpu = GpuContext::new(instance, Some(surface)).await?;
        let gpu_info = GpuInfo::from_adapter(&gpu.adapter);

        let swapchain_format = surface.get_capabilities(&gpu.adapter).formats[0];

        let render_pipeline = gpu.create_pipeline(swapchain_format, "two_spheres_fs");

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
        self.gpu_info = Some(gpu_info);
        self.window_surface = Some(window_surface);
        self.config = Some(config);
        self.render_pipeline = Some(render_pipeline);
        self.start = Instant::now();
        Ok(())
    }

    fn render(&mut self) {
        let frame_start = Instant::now();

        let window_surface = match &self.window_surface {
            Some(ws) => ws,
            None => return,
        };
        let gpu = match &self.gpu {
            Some(gpu) => gpu,
            None => return,
        };

        // Evaluate camera path based on elapsed time
        let elapsed = self.start.elapsed().as_secs_f32();
        let duration = self.camera_path.duration();

        // Exit when camera path completes
        if elapsed >= duration {
            self.close_requested = true;
            return;
        }

        let t = elapsed / duration;
        let pose = self.camera_path.evaluate(elapsed);

        let cam_pos = pose.position;
        let cam_dir = pose.direction();
        let cam_vup = pose.up(Vec3::Y);

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

        let push_constants = shared::ShaderConstants {
            width: current_size.width,
            height: current_size.height,
            time: elapsed,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cam_pos: cam_pos.into(),
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

        // Record frame timing
        let frame_time_us = frame_start.elapsed().as_micros() as u64;
        self.frame_records.push(FrameRecord {
            frame: self.frame_count,
            t,
            time_us: frame_time_us,
            cam_pos: cam_pos.into(),
            cam_dir: cam_dir.into(),
            cam_vup: cam_vup.into(),
        });
        self.frame_count += 1;
    }
}

impl ApplicationHandler for BenchApp {
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => self.close_requested = true,
                        Key::Character(c) if c.as_str() == "q" || c.as_str() == "Q" => {
                            self.close_requested = true
                        }
                        _ => {}
                    }
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

pub fn run_bench() -> Result<(), Box<dyn Error>> {
    log::debug!("Running benchmark with camera path");
    let event_loop = EventLoop::new()?;
    let mut app = BenchApp::new();
    event_loop.run_app(&mut app).map_err(Into::into)
}
