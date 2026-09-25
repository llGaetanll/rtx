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

/// Format of the offscreen target a headless benchmark draws into. The same format
/// a window's swapchain gets on the Vulkan drivers this runs on, so the fragment
/// shader writes the same bytes either way.
const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// A queued benchmark: what to look at, and the moving camera that looks at it.
struct QueuedBenchmark {
    name: String,
    scene_path: PathBuf,
    def: VideoConfig,
}

/// A run through the queued benchmarks, independent of where the frames end up.
/// The window and headless modes both drive one of these, handing it a view to
/// draw each frame into.
struct Session {
    gpu: GpuContext,
    gpu_info: GpuInfo,
    render_pipeline: wgpu::RenderPipeline,
    scene_buffers: SceneBuffers,
    scene: crate::scene_data::SceneInfo,
    current: QueuedBenchmark,
    camera_path: CameraPath,
    /// The camera settings the path does not record: which way is up, and the
    /// lens. Sampled per frame like the position is.
    tracks: CameraTracks,
    frame_records: Vec<FrameRecord>,
    queue: Vec<QueuedBenchmark>,
    timestamp: DateTime<Utc>,
    headless: bool,
}

impl Session {
    fn new(
        gpu: GpuContext,
        format: wgpu::TextureFormat,
        mut benchmarks: Vec<QueuedBenchmark>,
        timestamp: DateTime<Utc>,
        headless: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let current = benchmarks.remove(0);
        let gpu_info = gpu_info_from_adapter(&gpu.adapter);
        let render_pipeline = gpu.create_pipeline(format);

        let scene = scene_data::load(&current.scene_path)?;
        let scene_buffers = gpu.upload_scene(&scene);

        log::info!(
            "Running benchmark '{}' (scene: {})",
            current.name,
            current.scene_path.display()
        );

        Ok(Self {
            camera_path: current.def.camera.path(current.def.output.frames),
            tracks: current.def.camera.tracks(),
            gpu,
            gpu_info,
            render_pipeline,
            scene_buffers,
            scene: scene.info(),
            current,
            frame_records: Vec::new(),
            queue: benchmarks,
            timestamp,
            headless,
        })
    }

    /// The size the current benchmark asks to be rendered at.
    fn size(&self) -> (u32, u32) {
        (
            self.current.def.output.width,
            self.current.def.output.height,
        )
    }

    /// Whether every frame of the current benchmark has been rendered.
    fn current_done(&self) -> bool {
        self.frame_records.len() as u32 >= self.camera_path.frame_count()
    }

    /// Write the current benchmark's results and move on to the next one.
    /// Returns false once the queue is empty.
    fn finish_current(&mut self, resolution: [u32; 2]) -> Result<bool, Box<dyn Error>> {
        match self.write_results(resolution) {
            Ok(path) => log::info!("Benchmark results written to {}", path.display()),
            Err(e) => log::error!("Failed to write benchmark results: {e}"),
        }

        if self.queue.is_empty() {
            return Ok(false);
        }

        let next = self.queue.remove(0);
        log::info!(
            "Running benchmark '{}' (scene: {})",
            next.name,
            next.scene_path.display()
        );

        // The pipeline is shared across scenes, so only the buffers behind it change
        let scene = scene_data::load(&next.scene_path)?;
        self.scene = scene.info();
        self.scene_buffers = self.gpu.upload_scene(&scene);

        self.camera_path = next.def.camera.path(next.def.output.frames);
        self.tracks = next.def.camera.tracks();
        self.frame_records.clear();
        self.current = next;

        Ok(true)
    }

    /// Render the next frame of the current benchmark into `view` and record how
    /// long it took, counting from `frame_start`.
    fn render_frame(
        &mut self,
        frame_start: Instant,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let frame_index = self.frame_records.len() as u32;

        // Evaluate the camera at the current frame
        let t = self.camera_path.frame_t(frame_index);
        let camera = self.tracks.at(t);

        let cam_pos = Vec3::from(camera.position);
        let cam_dir = Vec3::from(camera.direction());
        let cam_vup = Vec3::from(camera.vup);

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let push_constants = shared::ShaderConstants {
            time: t,
            ..camera.constants(width, height, self.current.def.quality, self.scene)
        };

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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

            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.scene_buffers.bind_group, &[]);
            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::bytes_of(&push_constants),
            );
            rpass.draw(0..3, 0..1);
        }

        self.gpu.queue.submit(Some(encoder.finish()));

        // Without vsync the CPU would otherwise race ahead and the elapsed time
        // would measure queue submission rather than the render itself
        self.gpu.device.poll(wgpu::PollType::Wait).ok();

        let frame_time_us = frame_start.elapsed().as_micros() as u64;

        self.frame_records.push(FrameRecord {
            frame: frame_index,
            t,
            time_us: frame_time_us,
            cam_pos: cam_pos.into(),
            cam_dir: cam_dir.into(),
            cam_vup: cam_vup.into(),
        });

        let done = self.frame_records.len() as u32;
        let total = self.camera_path.frame_count();
        if done % PROGRESS_INTERVAL == 0 || done == total {
            let sum: u64 = self.frame_records.iter().map(|r| r.time_us).sum();
            let mean = sum as f64 / self.frame_records.len() as f64;
            let eta = mean * (total - done) as f64 / 1e6;

            log::info!(
                "{}: frame {}/{}  {:.1} ms  mean {:.1} ms  eta {:.0}s",
                self.current.name,
                done,
                total,
                frame_time_us as f64 / 1e3,
                mean / 1e3,
                eta
            );
        }
    }

    /// Write benchmark results to a JSONL file.
    fn write_results(&self, resolution: [u32; 2]) -> Result<PathBuf, Box<dyn Error>> {
        // Create output directory: bench/results/<git-sha>/
        let output_dir = PathBuf::from("bench/results").join(GIT_SHA);
        fs::create_dir_all(&output_dir)?;

        // Output file: bench/results/<git-sha>/<datetime>-<name>.jsonl
        let filename_timestamp = self.timestamp.format("%Y-%m-%d-%H-%M-%S");
        let output_path = output_dir.join(format!(
            "{}-{}.jsonl",
            filename_timestamp, self.current.name
        ));
        let file = fs::File::create(&output_path)?;
        let mut writer = BufWriter::new(file);

        // Write metadata as first line
        let quality = self.current.def.quality;
        let metadata = BenchmarkMetadata {
            version: 1,
            timestamp: self.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            git_sha: GIT_SHA.to_string(),
            scene: scene_data::name_of(&self.current.scene_path),
            resolution,
            samples: quality.samples,
            bounces: quality.bounces,
            headless: self.headless,
            gpu: self.gpu_info.clone(),
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

/// Application for benchmark mode with animated camera path.
pub struct BenchApp {
    /// Waiting for the window to exist, after which they move into the session.
    benchmarks: Vec<QueuedBenchmark>,
    timestamp: DateTime<Utc>,
    session: Option<Session>,
    config: Option<wgpu::SurfaceConfiguration>,
    /// Declared after everything holding a GPU handle. Fields drop in declaration
    /// order, and the surface has to go before the window it borrows.
    window_surface: Option<WindowSurface>,
    close_requested: bool,
}

impl BenchApp {
    fn new(benchmarks: Vec<QueuedBenchmark>, timestamp: DateTime<Utc>) -> Self {
        Self {
            benchmarks,
            timestamp,
            session: None,
            config: None,
            window_surface: None,
            close_requested: false,
        }
    }

    async fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let first = &self.benchmarks[0].def.output;
        let window_attributes = WindowAttributes::default()
            .with_title("RTX Benchmark")
            .with_inner_size(PhysicalSize::new(first.width, first.height));
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
        let swapchain_format = surface.get_capabilities(&gpu.adapter).formats[0];

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

        let benchmarks = std::mem::take(&mut self.benchmarks);
        self.session = Some(Session::new(
            gpu,
            swapchain_format,
            benchmarks,
            self.timestamp,
            false,
        )?);
        self.window_surface = Some(window_surface);
        self.config = Some(config);
        Ok(())
    }

    fn render(&mut self) {
        let frame_start = Instant::now();

        let (Some(window_surface), Some(session), Some(config)) =
            (&self.window_surface, &mut self.session, &self.config)
        else {
            return;
        };

        // When all frames are rendered, write results and advance to next benchmark
        if session.current_done() {
            match session.finish_current([config.width, config.height]) {
                Ok(true) => {
                    // Benchmarks may render at different sizes, so resize before the next one
                    let (width, height) = session.size();
                    let _ = window_surface
                        .borrow_window()
                        .request_inner_size(PhysicalSize::new(width, height));
                }
                Ok(false) => self.close_requested = true,
                Err(e) => {
                    log::error!("{e}");
                    self.close_requested = true;
                }
            }
            return;
        }

        let current_size = window_surface.borrow_window().inner_size();
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

        session.render_frame(frame_start, &view, current_size.width, current_size.height);

        frame.present();
    }
}

impl ApplicationHandler for BenchApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.session.is_some() {
            return;
        }
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
                    if let (Some(ws), Some(session)) = (&self.window_surface, &self.session) {
                        ws.borrow_surface().configure(&session.gpu.device, config);
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

/// Run the benchmarks without a window, drawing each frame into an offscreen
/// texture at exactly the size its config asks for. Nothing is shown, so the
/// machine stays usable while it runs.
fn run_headless(
    benchmarks: Vec<QueuedBenchmark>,
    timestamp: DateTime<Utc>,
) -> Result<(), Box<dyn Error>> {
    let instance = GpuContext::create_instance();
    let gpu = block_on(GpuContext::new(instance, None))?;
    let mut session = Session::new(gpu, HEADLESS_FORMAT, benchmarks, timestamp, true)?;

    loop {
        let (width, height) = session.size();
        let target = session.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bench_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        while !session.current_done() {
            session.render_frame(Instant::now(), &view, width, height);
        }

        if !session.finish_current([width, height])? {
            return Ok(());
        }
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
pub fn run_bench(
    scene_path: &Path,
    config_path: &Path,
    headless: bool,
) -> Result<(), Box<dyn Error>> {
    run_benchmarks(
        vec![queue(scene_path.to_path_buf(), config_path)?],
        headless,
    )
}

/// Time every benchmark listed in the manifest.
pub fn run_all_benchmarks(headless: bool) -> Result<(), Box<dyn Error>> {
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

    run_benchmarks(benchmarks, headless)
}

fn run_benchmarks(benchmarks: Vec<QueuedBenchmark>, headless: bool) -> Result<(), Box<dyn Error>> {
    if benchmarks.is_empty() {
        return Err("No benchmarks to run".into());
    }

    if headless {
        return run_headless(benchmarks, Utc::now());
    }

    let event_loop = EventLoop::new()?;
    let mut app = BenchApp::new(benchmarks, Utc::now());
    event_loop.run_app(&mut app).map_err(Into::into)
}
