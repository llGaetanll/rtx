use std::error::Error;
use std::path::PathBuf;

use futures::executor::block_on;
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
use crate::gpu::Accumulator;
use crate::gpu::GpuContext;
use crate::gpu::SceneBuffers;
use crate::render_app::Progress;
use crate::render_app::RenderPlan;
use crate::scene_data;
use crate::window_surface::WindowSurface;
use crate::window_surface::WindowSurfaceBuilder;

/// The preview window fits inside this, keeping the render's aspect ratio. A
/// render is usually larger than a comfortable window and is never scaled up.
const MAX_WINDOW: (f64, f64) = (1280.0, 720.0);

/// The pipeline that puts the accumulated image on screen.
struct Blit {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

impl Blit {
    fn new(gpu: &GpuContext, accumulated: &wgpu::TextureView, format: wgpu::TextureFormat) -> Self {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        // A 32 bit float texture cannot be filtered, so the
                        // shader reads texels directly and needs no sampler
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(accumulated),
            }],
        });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("blit_pipeline_layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::FRAGMENT,
                    range: 0..std::mem::size_of::<shared::BlitConstants>() as u32,
                }],
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("blit_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gpu.shader_module,
                    entry_point: Some("main_vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &gpu.shader_module,
                    entry_point: Some("blit_fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
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

        Self {
            pipeline,
            bind_group,
        }
    }
}

/// Everything that only exists once the window is up and the GPU is chosen.
struct Ready {
    gpu: GpuContext,
    surface_config: wgpu::SurfaceConfiguration,
    scene: SceneBuffers,
    constants: shared::ShaderConstants,
    accumulator: Accumulator,
    blit: Blit,
    progress: Progress,
    /// Declared after everything holding a GPU handle. Fields drop in declaration
    /// order, and the surface has to go before the window it borrows.
    window_surface: WindowSurface,
}

pub struct PreviewApp {
    name: String,
    scene_path: PathBuf,
    image: ImageConfig,
    plan: RenderPlan,
    ready: Option<Ready>,
    saved: bool,
    /// Set when initialisation fails, so the error is returned rather than only
    /// logged from inside the event loop.
    error: Option<Box<dyn Error>>,
}

impl PreviewApp {
    fn new(name: String, scene_path: PathBuf, image: ImageConfig, plan: RenderPlan) -> Self {
        Self {
            name,
            scene_path,
            image,
            plan,
            ready: None,
            saved: false,
            error: None,
        }
    }

    fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<Ready, Box<dyn Error>> {
        let width = self.image.output.width;
        let height = self.image.output.height;

        let window_attributes = WindowAttributes::default()
            .with_title(format!("rtx render: {}", self.name))
            .with_inner_size(window_size(width, height));
        let window = event_loop.create_window(window_attributes)?;

        let instance = GpuContext::create_instance();
        let window_surface = WindowSurfaceBuilder {
            window: Box::new(window),
            surface_builder: |window| {
                instance
                    .create_surface(window)
                    .expect("Failed to create surface")
            },
        }
        .build();

        let surface = window_surface.borrow_surface();
        let gpu = block_on(GpuContext::new(instance, Some(surface)))?;

        // The blit shader writes linear color and lets the surface encode it,
        // the same transfer the offscreen render path gets from its sRGB target
        let capabilities = surface.get_capabilities(&gpu.adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(capabilities.formats[0]);
        if !format.is_srgb() {
            log::warn!("no sRGB surface format available, the preview will look dark");
        }

        let window_size = window_surface.borrow_window().inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: Default::default(),
        };
        surface.configure(&gpu.device, &surface_config);

        let scene_data = scene_data::load(&self.scene_path)?;
        let constants =
            self.image
                .camera
                .constants(width, height, self.image.quality, scene_data.background);
        let scene = gpu.upload_scene(&scene_data);

        let accumulator = Accumulator::new(&gpu, width, height)?;
        let blit = Blit::new(&gpu, accumulator.view(), format);

        crate::render_app::log_start(&self.name, &self.scene_path, &self.image, self.plan);

        Ok(Ready {
            gpu,
            surface_config,
            scene,
            constants,
            accumulator,
            blit,
            progress: Progress::new(self.plan, width, height),
            window_surface,
        })
    }

    fn finished(&self) -> bool {
        self.ready
            .as_ref()
            .is_some_and(|ready| ready.accumulator.passes_done() >= self.plan.passes)
    }

    /// Draw one more pass, then say so in the log and the title bar.
    fn render_pass(&mut self) {
        let Some(ready) = self.ready.as_mut() else {
            return;
        };

        ready.accumulator.pass(
            &ready.gpu,
            &ready.scene,
            &ready.constants,
            self.plan.samples_per_pass,
        );

        let done = ready.accumulator.passes_done();
        ready.progress.log_pass(done);
        ready.window_surface.borrow_window().set_title(&format!(
            "rtx render: {} — {}%",
            self.name,
            done * 100 / self.plan.passes
        ));
    }

    /// Show what has accumulated so far.
    fn present(&mut self) {
        let Some(ready) = self.ready.as_ref() else {
            return;
        };

        let surface = ready.window_surface.borrow_surface();
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                log::warn!("could not get the next frame: {e:?}");
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let done = ready.accumulator.passes_done();
        let constants = shared::BlitConstants {
            image_width: self.image.output.width,
            image_height: self.image.output.height,
            surface_width: ready.surface_config.width,
            surface_height: ready.surface_config.height,
            scale: if done == 0 { 0.0 } else { 1.0 / done as f32 },
        };

        let mut encoder =
            ready
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("preview_encoder"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("preview_render_pass"),
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

            rpass.set_pipeline(&ready.blit.pipeline);
            rpass.set_bind_group(0, &ready.blit.bind_group, &[]);
            rpass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::bytes_of(&constants),
            );
            rpass.draw(0..3, 0..1);
        }

        ready.gpu.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    /// Write the image out. A preview closed before the last pass still saves
    /// what it managed to render, only noisier than asked for.
    fn save(&mut self) {
        if self.saved {
            return;
        }
        let Some(ready) = self.ready.as_ref() else {
            return;
        };

        let done = ready.accumulator.passes_done();
        if done == 0 {
            return;
        }
        self.saved = true;

        if done < self.plan.passes {
            log::info!("stopped after {}/{} passes", done, self.plan.passes);
        }

        let accumulated = ready.accumulator.read(&ready.gpu);
        let saved = crate::render_app::save(
            &self.name,
            &accumulated,
            self.image.output.width,
            self.image.output.height,
        );

        match saved {
            Ok(path) => ready.progress.log_saved(&path, done),
            Err(e) => self.error = Some(e),
        }
    }
}

impl ApplicationHandler for PreviewApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match self.init(event_loop) {
            Ok(ready) => self.ready = Some(ready),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let closing = match &event {
            WindowEvent::CloseRequested => true,
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                matches!(event.logical_key, Key::Named(NamedKey::Escape))
                    || matches!(event.logical_key.as_ref(), Key::Character("q" | "Q"))
            }
            _ => false,
        };

        if closing {
            self.save();
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::Resized(new_size) => {
                if let Some(ready) = self.ready.as_mut() {
                    ready.surface_config.width = new_size.width.max(1);
                    ready.surface_config.height = new_size.height.max(1);
                    ready
                        .window_surface
                        .borrow_surface()
                        .configure(&ready.gpu.device, &ready.surface_config);
                }
            }
            WindowEvent::RedrawRequested => self.present(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished() {
            // The picture is done, so the window is only waiting to be closed
            self.save();
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        self.render_pass();
        event_loop.set_control_flow(ControlFlow::Poll);

        if let Some(ready) = self.ready.as_ref() {
            ready.window_surface.borrow_window().request_redraw();
        }
    }
}

/// A window that fits the render's aspect ratio inside [`MAX_WINDOW`].
fn window_size(width: u32, height: u32) -> LogicalSize<f64> {
    let (max_width, max_height) = MAX_WINDOW;
    let scale = (max_width / width as f64)
        .min(max_height / height as f64)
        .min(1.0);

    LogicalSize::new(width as f64 * scale, height as f64 * scale)
}

/// Render in a window, showing the image as it accumulates.
pub fn run_preview(
    name: String,
    scene_path: PathBuf,
    image: ImageConfig,
    plan: RenderPlan,
) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = PreviewApp::new(name, scene_path, image, plan);
    event_loop.run_app(&mut app)?;

    match app.error.take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
