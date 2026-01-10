use std::error::Error;
use std::time::Instant;

use clap::Parser;
use clap::Subcommand;
use futures::executor::block_on;
use ouroboros::self_referencing;
use wgpu::InstanceDescriptor;
use wgpu::include_spirv;
use wgpu::include_spirv_raw;
use wgpu::{self};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::ElementState;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::keyboard::NamedKey;
use winit::window::Window;
use winit::window::WindowAttributes;
use winit::window::WindowId;

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
        /// Which scene to render
        #[arg(short, long, default_value = "cornell")]
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

struct RustShaderSandboxApp {
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    window_surface: Option<WindowSurface>,
    config: Option<wgpu::SurfaceConfiguration>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    shader_module: Option<wgpu::ShaderModule>,
    close_requested: bool,
    start: Instant,
    cursor_x: f32,
    cursor_y: f32,
}

impl Default for RustShaderSandboxApp {
    fn default() -> Self {
        Self {
            device: None,
            queue: None,
            window_surface: None,
            config: None,
            render_pipeline: None,
            shader_module: None,
            close_requested: false,
            start: Instant::now(),
            cursor_x: 0.0,
            cursor_y: 0.0,
        }
    }
}

impl RustShaderSandboxApp {
    async fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("Rust Shader Sandbox")
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window_box = event_loop.create_window(window_attributes)?;

        let mut instance_flags = wgpu::InstanceFlags::default();
        instance_flags.remove(wgpu::InstanceFlags::VALIDATION);
        instance_flags.remove(wgpu::InstanceFlags::DEBUG);

        let instance = wgpu::Instance::new(&InstanceDescriptor {
            flags: instance_flags,
            ..Default::default()
        });

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

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(surface),
                force_fallback_adapter: false,
            })
            .await?;

        let mut required_features = wgpu::Features::PUSH_CONSTANTS;
        if adapter
            .features()
            .contains(wgpu::Features::SPIRV_SHADER_PASSTHROUGH)
        {
            required_features |= wgpu::Features::SPIRV_SHADER_PASSTHROUGH;
        }

        let required_limits = wgpu::Limits {
            max_push_constant_size: 256,
            ..Default::default()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits,
                ..Default::default()
            })
            .await?;

        let shader_module = if device
            .features()
            .contains(wgpu::Features::SPIRV_SHADER_PASSTHROUGH)
        {
            let x = include_spirv_raw!(env!("shader.spv"));
            unsafe { device.create_shader_module_passthrough(x) }
        } else {
            device.create_shader_module(include_spirv!(env!("shader.spv")))
        };

        let swapchain_format = surface.get_capabilities(&adapter).formats[0];
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
                range: 0..std::mem::size_of::<shared::ShaderConstants>() as u32,
            }],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("main_vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("main_fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: swapchain_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

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
        surface.configure(&device, &config);

        self.device = Some(device);
        self.queue = Some(queue);
        self.window_surface = Some(window_surface);
        self.config = Some(config);
        self.render_pipeline = Some(render_pipeline);
        self.shader_module = Some(shader_module);
        self.start = Instant::now();
        Ok(())
    }

    fn render(&mut self) {
        let window_surface = match &self.window_surface {
            Some(ws) => ws,
            None => return,
        };

        let window = window_surface.borrow_window();
        let current_size = window.inner_size();
        let surface = window_surface.borrow_surface();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

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

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let push_constants = shared::ShaderConstants {
            width: current_size.width,
            height: current_size.height,
            time: self.start.elapsed().as_secs_f32(),
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
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

        queue.submit(Some(encoder.finish()));
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
                        if let Some(device) = self.device.as_ref() {
                            surface.configure(device, config);
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_x = position.x as f32;
                self.cursor_y = position.y as f32;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == NamedKey::Escape && event.state == ElementState::Pressed {
                    self.close_requested = true;
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
    let mut app = RustShaderSandboxApp::default();
    event_loop.run_app(&mut app).map_err(Into::into)
}

fn run_test() -> Result<(), Box<dyn Error>> {
    log::debug!("Test mode: rendering all scenes to grid image...");
    log::debug!("(not yet implemented)");
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Live { scene }) => run_live(&scene),
        Some(Commands::Test) => run_test(),
        None => run_live("cornell"),
    }
}
