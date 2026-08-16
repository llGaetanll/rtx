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
use crate::gpu::Tiling;
use crate::render_app::Progress;
use crate::render_app::RenderPlan;
use crate::scene_data;
use crate::window_surface::WindowSurface;
use crate::window_surface::WindowSurfaceBuilder;

/// The preview window fits inside this, keeping the render's aspect ratio. A
/// render is usually larger than a comfortable window and is never scaled up.
const MAX_WINDOW: (f64, f64) = (1280.0, 720.0);

/// The preview image fits inside this, keeping the render's aspect ratio.
///
/// The window shows the render being worked on rather than a render of its own,
/// and what it shows is this standing copy that finished tiles are shrunk into.
/// It is sized for looking at, not for printing, so a render of any size can be
/// watched in a window without either one constraining the other.
const MAX_PREVIEW: (f64, f64) = (1600.0, 1600.0);

/// The preview holds averaged color, which the blit to the window then encodes.
/// It is only ever looked at, so it needs none of the range the render's own
/// float target does.
const PREVIEW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// A preview image that fits inside [`MAX_PREVIEW`] with the render's aspect
/// ratio. Never larger than the render itself, which would be inventing detail.
fn preview_size(width: u32, height: u32) -> (u32, u32) {
    let (max_width, max_height) = MAX_PREVIEW;
    let scale = (max_width / width as f64)
        .min(max_height / height as f64)
        .min(1.0);

    (
        ((width as f64 * scale) as u32).max(1),
        ((height as f64 * scale) as u32).max(1),
    )
}

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

/// The standing preview image, and the pipeline that shrinks tiles into it.
///
/// The render's own target moves from tile to tile and is emptied each time, so
/// it is never a picture of the whole image. This is: every tile is copied here
/// as it finishes and stays, which is what makes the window fill in.
struct Preview {
    /// Only the view is ever bound, but the texture it looks into has to outlive
    /// it, so it is kept here rather than dropped at the end of construction.
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    width: u32,
    height: u32,
}

impl Preview {
    fn new(gpu: &GpuContext, image_width: u32, image_height: u32) -> Self {
        let (width, height) = preview_size(image_width, image_height);

        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("preview_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PREVIEW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tile_blit_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tile_blit_pipeline_layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::FRAGMENT,
                    range: 0..std::mem::size_of::<shared::TileBlitConstants>() as u32,
                }],
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tile_blit_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gpu.shader_module,
                    entry_point: Some("main_vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &gpu.shader_module,
                    entry_point: Some("tile_blit_fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: PREVIEW_FORMAT,
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

        let preview = Self {
            _texture: texture,
            view,
            pipeline,
            layout,
            width,
            height,
        };
        preview.clear(gpu);

        preview
    }

    /// Black out the preview, so the tiles not yet drawn are black rather than
    /// whatever the texture happened to be allocated over.
    fn clear(&self, gpu: &GpuContext) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("preview_clear_encoder"),
            });

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("preview_clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
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

        gpu.queue.submit(Some(encoder.finish()));
    }

    /// Copy what has accumulated in `accumulator` into this tile's place.
    ///
    /// Called while the tile is still being worked on as well as when it is
    /// finished, so the window shows the tile in progress converging rather than
    /// staying black until it is done.
    fn update_tile(&self, gpu: &GpuContext, accumulator: &Accumulator, image: (u32, u32)) {
        let tile = accumulator.tile();
        let done = accumulator.passes_done();

        let scale_x = self.width as f32 / image.0 as f32;
        let scale_y = self.height as f32 / image.1 as f32;

        let constants = shared::TileBlitConstants {
            dst_x: tile.x as f32 * scale_x,
            dst_y: tile.y as f32 * scale_y,
            dst_width: tile.width as f32 * scale_x,
            dst_height: tile.height as f32 * scale_y,
            tile_width: tile.width,
            tile_height: tile.height,
            scale: if done == 0 { 0.0 } else { 1.0 / done as f32 },
        };

        // The accumulator's texture is sized for the largest tile, so a clipped
        // edge tile is a window onto the top left of it. The shader is told the
        // tile's real size and reads no further
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tile_blit"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(accumulator.view()),
            }],
        });

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tile_blit_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tile_blit_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Every other tile already drawn here has to survive
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT,
                0,
                bytemuck::bytes_of(&constants),
            );
            rpass.draw(0..3, 0..1);
        }

        gpu.queue.submit(Some(encoder.finish()));
    }
}

/// Everything that only exists once the window is up and the GPU is chosen.
struct Ready {
    gpu: GpuContext,
    surface_config: wgpu::SurfaceConfiguration,
    scene: SceneBuffers,
    constants: shared::ShaderConstants,
    accumulator: Accumulator,
    /// The tiles of the image, and how far through them the render is. The
    /// accumulator draws one at a time and is emptied between them.
    tiling: Tiling,
    tile: u32,
    /// The image being built up, which finished tiles are read back into. This is
    /// what gets saved, and it is the reason the render outlives the window's
    /// idea of what it can display.
    image: Vec<f32>,
    preview: Preview,
    blit: Blit,
    progress: Progress,
    /// Declared after everything holding a GPU handle. Fields drop in declaration
    /// order, and the surface has to go before the window it borrows.
    window_surface: WindowSurface,
}

impl Ready {
    /// The tile being drawn, or `None` once every tile is finished.
    fn current_tile(&self) -> Option<crate::gpu::Tile> {
        self.tiling.tiles().nth(self.tile as usize)
    }
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
                .constants(width, height, self.image.quality, scene_data.info());
        let scene = gpu.upload_scene(&scene_data);

        let tiling = Tiling::new(&gpu.device, width, height);
        let accumulator = Accumulator::tiled(&gpu, &tiling)?;

        // The window shows the standing preview, not the tile being worked on
        let preview = Preview::new(&gpu, width, height);
        let blit = Blit::new(&gpu, &preview.view, format);

        crate::render_app::log_start(&self.name, &self.scene_path, &self.image, self.plan);

        let progress = Progress::new(self.plan, width, height, tiling.tile_count());
        progress.log_tiling(tiling.tile_width, tiling.tile_height);

        Ok(Ready {
            gpu,
            surface_config,
            scene,
            constants,
            accumulator,
            tiling,
            tile: 0,
            image: vec![0.0; (width as usize) * (height as usize) * 4],
            preview,
            blit,
            progress,
            window_surface,
        })
    }

    /// True once every tile has had all its passes and been read back.
    fn finished(&self) -> bool {
        self.ready
            .as_ref()
            .is_some_and(|ready| ready.tile >= ready.tiling.tile_count())
    }

    /// Draw one more pass of the tile being worked on, show it, and move on to
    /// the next tile once this one has had all of them.
    fn render_pass(&mut self) {
        let Some(ready) = self.ready.as_mut() else {
            return;
        };

        let Some(tile) = ready.current_tile() else {
            return;
        };

        // Starting a tile is deferred to its first pass, so the accumulator is
        // pointed at it here rather than when the tile before it finished
        if ready.accumulator.tile() != tile {
            ready.accumulator.start_tile(tile);
        }

        ready.accumulator.pass(
            &ready.gpu,
            &ready.scene,
            &ready.constants,
            self.plan.samples_per_pass,
        );

        let image = (self.image.output.width, self.image.output.height);
        ready
            .preview
            .update_tile(&ready.gpu, &ready.accumulator, image);

        let done = ready.accumulator.passes_done();
        ready.progress.log_pass(done);

        // Counted before the tile is retired, so that finishing a tile does not
        // count its passes once here and again as a completed tile
        let total = ready.tiling.tile_count() * self.plan.passes;
        let so_far = ready.tile * self.plan.passes + done;

        if done >= self.plan.passes {
            // The tile is finished, so it joins the picture and the accumulator
            // is free to be emptied for the next one
            ready.accumulator.read_into(&ready.gpu, &mut ready.image);
            ready.tile += 1;
            ready.progress.next_tile();
        }

        ready.window_surface.borrow_window().set_title(&format!(
            "rtx render: {} — {}%",
            self.name,
            so_far * 100 / total.max(1)
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

        let constants = shared::BlitConstants {
            // What is being blitted is the preview, not the render. The tiles
            // were averaged on their way in, so there is nothing left to scale
            image_width: ready.preview.width,
            image_height: ready.preview.height,
            surface_width: ready.surface_config.width,
            surface_height: ready.surface_config.height,
            scale: 1.0,
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

    /// Write the finished image out.
    ///
    /// Only a render that got through every tile is a picture. One stopped early
    /// has tiles that were never drawn, and saving it would mean writing a file
    /// with blank rectangles in it and calling that a render, so closing the
    /// window before the end throws the work away instead.
    fn save(&mut self) {
        if self.saved || !self.finished() {
            return;
        }
        let Some(ready) = self.ready.as_ref() else {
            return;
        };
        self.saved = true;

        let saved = crate::render_app::save(
            &self.name,
            &ready.image,
            self.image.output.width,
            self.image.output.height,
        );

        match saved {
            Ok(path) => ready.progress.log_saved(&path),
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
            if !self.finished() {
                log::info!("closed before the render finished, nothing saved");
            }
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
