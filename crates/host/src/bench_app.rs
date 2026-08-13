use std::error::Error;
use std::fs;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use chrono::DateTime;
use chrono::Utc;
use futures::executor::block_on;
use glam::Vec3;
use rtx_bench::BenchmarkMetadata;
use rtx_bench::CameraPath;
use rtx_bench::FrameRecord;
use rtx_bench::GpuInfo;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::ElementState;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
use winit::window::WindowAttributes;
use winit::window::WindowId;

use crate::config;
use crate::config::BenchManifest;
use crate::config::CameraTracks;
use crate::config::Quality;
use crate::config::VideoConfig;
use crate::gpu::GpuContext;
use crate::gpu::SceneBuffers;
use crate::scene_data;
use crate::window_surface::WindowSurface;
use crate::window_surface::WindowSurfaceBuilder;

/// Extract GPU info from a wgpu adapter.
fn gpu_info_from_adapter(adapter: &wgpu::Adapter) -> GpuInfo {
    let info = adapter.get_info();
    GpuInfo::new(info.name, info.driver, format!("{:?}", info.backend))
}

/// Git SHA baked in at build time via build.rs.
const GIT_SHA: &str = env!("GIT_SHA");

/// How often to report progress. A benchmark otherwise prints nothing between
/// its start and its results, which is indistinguishable from a hang when a
/// frame takes seconds.
const PROGRESS_INTERVAL: u32 = 10;

/// A queued benchmark: what to look at, and the moving camera that looks at it.
struct QueuedBenchmark {
    name: String,
    scene_path: PathBuf,
    def: VideoConfig,
}

/// Application for benchmark mode with animated camera path.
pub struct BenchApp {
    gpu: Option<GpuContext>,
    gpu_info: Option<GpuInfo>,
    config: Option<wgpu::SurfaceConfiguration>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    swapchain_format: Option<wgpu::TextureFormat>,
    scene_buffers: Option<SceneBuffers>,
    background: [f32; 3],
    /// Declared after everything holding a GPU handle. Fields drop in declaration
    /// order, and the surface has to go before the window it borrows.
    window_surface: Option<WindowSurface>,
    close_requested: bool,
    camera_path: CameraPath,
    /// The camera settings the path does not record: which way is up, and the
    /// lens. Sampled per frame like the position is.
    tracks: CameraTracks,
    frame_records: Vec<FrameRecord>,
    frame_count: u32,
    name: String,
    scene_path: PathBuf,
    width: u32,
    height: u32,
    quality: Quality,
    timestamp: DateTime<Utc>,
    queue: Vec<QueuedBenchmark>,
}

impl BenchApp {
    fn new(benchmarks: Vec<QueuedBenchmark>, timestamp: DateTime<Utc>) -> Self {
        let mut queue = benchmarks;

        // Pop the first benchmark to run immediately
        let first = queue.remove(0);
        let camera_path = first.def.camera.path(first.def.output.frames);
        let tracks = first.def.camera.tracks();

        Self {
            gpu: None,
            gpu_info: None,
            window_surface: None,
            config: None,
            render_pipeline: None,
            swapchain_format: None,
            scene_buffers: None,
            background: [0.; 3],
            close_requested: false,
            camera_path,
            tracks,
            frame_records: Vec::new(),
            frame_count: 0,
            name: first.name,
            width: first.def.output.width,
            height: first.def.output.height,
            quality: first.def.quality,
            scene_path: first.scene_path,
            timestamp,
            queue,
        }
    }

    /// Advance to the next benchmark in the queue. Returns false if queue is empty.
    fn advance_to_next(&mut self) -> bool {
        if self.queue.is_empty() {
            return false;
        }

        let next = self.queue.remove(0);
        log::info!(
            "Running benchmark '{}' (scene: {})",
            next.name,
            next.scene_path.display()
        );

        // Update camera path
        self.camera_path = next.def.camera.path(next.def.output.frames);
        self.tracks = next.def.camera.tracks();

        // Reset frame tracking
        self.frame_records.clear();
        self.frame_count = 0;

        // Update scene info
        self.name = next.name;
        self.scene_path = next.scene_path;
        self.width = next.def.output.width;
        self.height = next.def.output.height;
        self.quality = next.def.quality;

        // Benchmarks may render at different sizes, so resize before the next one
        if let Some(ws) = &self.window_surface {
            let _ = ws
                .borrow_window()
                .request_inner_size(PhysicalSize::new(self.width, self.height));
        }

        // Upload the new scene. The pipeline is shared across scenes now, so only
        // the buffers behind it change
        if let Some(gpu) = &self.gpu {
            match scene_data::load(&self.scene_path) {
                Ok(scene) => {
                    self.background = scene.background;
                    self.scene_buffers = Some(gpu.upload_scene(&scene));
                }
                Err(e) => log::error!("{e}"),
            }
        }

        true
    }

    async fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("RTX Benchmark")
            .with_inner_size(PhysicalSize::new(self.width, self.height));
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
        let gpu_info = gpu_info_from_adapter(&gpu.adapter);

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
            // Vsync would clamp every frame time to the refresh interval, hiding
            // any improvement that takes a frame below it
            present_mode: wgpu::PresentMode::AutoNoVsync,
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
        self.swapchain_format = Some(swapchain_format);
        self.scene_buffers = Some(scene_buffers);
        log::info!(
            "Running benchmark '{}' (scene: {})",
            self.name,
            self.scene_path.display()
        );
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

        // When all frames are rendered, write results and advance to next benchmark
        if self.frame_count >= self.camera_path.frame_count() {
            match self.write_results() {
                Ok(path) => log::info!("Benchmark results written to {}", path.display()),
                Err(e) => log::error!("Failed to write benchmark results: {e}"),
            }
            if !self.advance_to_next() {
                self.close_requested = true;
            }
            return;
        }

        // Evaluate the camera at the current frame
        let t = self.camera_path.frame_t(self.frame_count);
        let camera = self.tracks.at(t);

        let cam_pos = Vec3::from(camera.position);
        let cam_dir = Vec3::from(camera.direction());
        let cam_vup = Vec3::from(camera.vup);

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
            time: t,
            ..camera.constants(
                current_size.width,
                current_size.height,
                self.quality,
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

        // Without vsync the CPU would otherwise race ahead and the elapsed time
        // would measure queue submission rather than the render itself
        gpu.device.poll(wgpu::PollType::Wait).ok();

        let frame_time_us = frame_start.elapsed().as_micros() as u64;

        frame.present();

        self.frame_records.push(FrameRecord {
            frame: self.frame_count,
            t,
            time_us: frame_time_us,
            cam_pos: cam_pos.into(),
            cam_dir: cam_dir.into(),
            cam_vup: cam_vup.into(),
        });
        self.frame_count += 1;

        let total = self.camera_path.frame_count();
        if self.frame_count % PROGRESS_INTERVAL == 0 || self.frame_count == total {
            let sum: u64 = self.frame_records.iter().map(|r| r.time_us).sum();
            let mean = sum as f64 / self.frame_records.len() as f64;
            let eta = mean * (total - self.frame_count) as f64 / 1e6;

            log::info!(
                "{}: frame {}/{}  {:.1} ms  mean {:.1} ms  eta {:.0}s",
                self.name,
                self.frame_count,
                total,
                frame_time_us as f64 / 1e3,
                mean / 1e3,
                eta
            );
        }
    }

    /// Write benchmark results to a JSONL file.
    fn write_results(&self) -> Result<PathBuf, Box<dyn Error>> {
        let gpu_info = self.gpu_info.as_ref().ok_or("No GPU info")?;
        let config = self.config.as_ref().ok_or("No surface config")?;

        // Create output directory: bench/results/<git-sha>/
        let output_dir = PathBuf::from("bench/results").join(GIT_SHA);
        fs::create_dir_all(&output_dir)?;

        // Output file: bench/results/<git-sha>/<datetime>-<name>.jsonl
        let filename_timestamp = self.timestamp.format("%Y-%m-%d-%H-%M-%S");
        let output_path = output_dir.join(format!("{}-{}.jsonl", filename_timestamp, self.name));
        let file = fs::File::create(&output_path)?;
        let mut writer = BufWriter::new(file);

        // Write metadata as first line
        let metadata = BenchmarkMetadata {
            version: 1,
            timestamp: self.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            git_sha: GIT_SHA.to_string(),
            scene: scene_data::name_of(&self.scene_path),
            resolution: [config.width, config.height],
            samples: self.quality.samples,
            bounces: self.quality.bounces,
            gpu: gpu_info.clone(),
            camera_path: self.camera_path.clone(),
        };
        serde_json::to_writer(&mut writer, &metadata)?;
        writeln!(writer)?;

        // Write frame records
        for record in &self.frame_records {
            serde_json::to_writer(&mut writer, record)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        Ok(output_path)
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

fn queue(scene_path: PathBuf, config_path: &Path) -> Result<QueuedBenchmark, Box<dyn Error>> {
    Ok(QueuedBenchmark {
        name: config::name_of(config_path),
        scene_path,
        def: VideoConfig::load(config_path)?,
    })
}

/// Time one scene through one video config.
pub fn run_bench(scene_path: &Path, config_path: &Path) -> Result<(), Box<dyn Error>> {
    run_benchmarks(vec![queue(scene_path.to_path_buf(), config_path)?])
}

/// Time every benchmark listed in the manifest.
pub fn run_all_benchmarks() -> Result<(), Box<dyn Error>> {
    let manifest = BenchManifest::load(Path::new(config::BENCH_MANIFEST))?;
    let benchmarks = manifest
        .benchmarks
        .into_iter()
        .map(|entry| queue(entry.scene, &entry.config))
        .collect::<Result<Vec<_>, _>>()?;

    log::info!(
        "Found {} benchmark(s): {}",
        benchmarks.len(),
        benchmarks
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    run_benchmarks(benchmarks)
}

fn run_benchmarks(benchmarks: Vec<QueuedBenchmark>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = BenchApp::new(benchmarks, Utc::now());
    event_loop.run_app(&mut app).map_err(Into::into)
}
