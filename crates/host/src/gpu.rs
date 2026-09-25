use std::error::Error;

use wgpu::InstanceDescriptor;
use wgpu::include_spirv;
use wgpu::include_spirv_raw;
use wgpu::{self};

/// Accumulation target format. Rendering sums many passes, so the intermediate
/// image has to hold unclamped linear values.
const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// A multi-pass render request. Each pass draws `samples_per_pass` rays per pixel
/// with a distinct RNG seed, and the passes are averaged into one image.
pub struct AccumulatedRender<'a> {
    /// The scene to trace, already resident on the GPU.
    pub scene: &'a SceneBuffers,
    pub width: u32,
    pub height: u32,
    pub passes: u32,
    pub samples_per_pass: u32,
    /// Camera and bounce settings. `width`, `height`, `px_samples` and `seed`
    /// are filled in per pass and may be left at their defaults here.
    pub constants: shared::ShaderConstants,
}

/// The one fragment entry point. Scenes are data now, not code, so there is no
/// longer an entry point per scene.
pub const FRAGMENT_ENTRY: &str = "trace_fs";

/// The scene buffers bound to a pipeline, in binding order.
const SCENE_BINDINGS: u32 = 8;

pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub shader_module: wgpu::ShaderModule,
    pub scene_layout: wgpu::BindGroupLayout,
}

/// A scene resident on the GPU. The buffers are kept alive alongside the bind
/// group that refers to them.
pub struct SceneBuffers {
    pub bind_group: wgpu::BindGroup,
    _buffers: Vec<wgpu::Buffer>,
}

/// Upload one array as a read-only storage buffer.
///
/// An empty array becomes a single zeroed element: a scene need not use every
/// material kind, but a zero sized binding is not allowed.
fn storage_buffer<T: bytemuck::Pod + bytemuck::Zeroable>(
    device: &wgpu::Device,
    label: &str,
    data: &[T],
) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    let fallback = [T::zeroed()];
    let contents = if data.is_empty() { &fallback[..] } else { data };

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(contents),
        usage: wgpu::BufferUsages::STORAGE,
    })
}

impl GpuContext {
    pub fn create_instance() -> wgpu::Instance {
        let mut instance_flags = wgpu::InstanceFlags::default();
        instance_flags.remove(wgpu::InstanceFlags::VALIDATION);
        instance_flags.remove(wgpu::InstanceFlags::DEBUG);

        // Vulkan only. The GL backend cannot take our SPIR-V as-is, so it goes
        // through naga and renders several entry points incorrectly, and tearing
        // down its EGL instance after the window is gone segfaults in the Wayland
        // client library. Asking for Vulkan alone turns a missing driver into a
        // clear failure at adapter selection instead of silently wrong output.
        wgpu::Instance::new(&InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: instance_flags,
            ..Default::default()
        })
    }

    /// Create a new GPU context. If a surface is provided, the adapter will be
    /// selected for compatibility with that surface.
    pub async fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, Box<dyn Error>> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
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
        // Blending a 32 bit float target is forbidden by the WebGPU spec, so wgpu
        // only allows it when validation is told to consult the real adapter
        // capabilities. Accumulation needs it to sum passes on the GPU.
        if adapter
            .features()
            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        {
            required_features |= wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        }

        // The adapter's own limits rather than the portable defaults, whose 8192
        // texture bound and 256MB buffer bound are both well under what a render
        // of any size needs from a tile. What the hardware will not raise is what
        // decides the tile size, so asking for less only makes tiles smaller
        let required_limits = wgpu::Limits {
            max_push_constant_size: 256,
            ..adapter.limits()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits,
                ..Default::default()
            })
            .await?;

        let info = adapter.get_info();
        log::debug!(
            "Adapter: {} ({:?}, {})",
            info.name,
            info.backend,
            info.driver
        );

        let shader_module = if device
            .features()
            .contains(wgpu::Features::SPIRV_SHADER_PASSTHROUGH)
        {
            let spirv = include_spirv_raw!(env!("shader.spv"));
            unsafe { device.create_shader_module_passthrough(spirv) }
        } else {
            device.create_shader_module(include_spirv!(env!("shader.spv")))
        };

        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_layout"),
            entries: &(0..SCENE_BINDINGS)
                .map(|binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                })
                .collect::<Vec<_>>(),
        });

        Ok(Self {
            adapter,
            device,
            queue,
            shader_module,
            scene_layout,
        })
    }

    /// Upload a scene and build the bind group the shader reads it through.
    ///
    /// A ray reaches an instance only through a leaf of the hierarchy, so an
    /// instance added after the hierarchy was built is one nothing can hit. That
    /// misrenders quietly - the surface is simply not there - which is worth a
    /// check here, where every scene passes on its way to the GPU.
    pub fn upload_scene(&self, scene: &crate::scene_data::SceneData) -> SceneBuffers {
        let covered: u32 = scene.bvh.iter().map(|node| node.count).sum();
        assert_eq!(
            covered as usize,
            scene.instances.len(),
            "the hierarchy covers {covered} of {} instances, so it was built \
             before the last of them was added",
            scene.instances.len(),
        );

        let buffers = vec![
            storage_buffer(&self.device, "instances", &scene.instances),
            storage_buffer(&self.device, "lambertians", &scene.lambertians),
            storage_buffer(&self.device, "metals", &scene.metals),
            storage_buffer(&self.device, "dielectrics", &scene.dielectrics),
            storage_buffer(&self.device, "diffuse_lights", &scene.diffuse_lights),
            storage_buffer(&self.device, "solids", &scene.solids),
            storage_buffer(&self.device, "lights", &scene.lights),
            storage_buffer(&self.device, "bvh", &scene.bvh),
        ];

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene"),
            layout: &self.scene_layout,
            entries: &buffers
                .iter()
                .enumerate()
                .map(|(i, buffer)| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: buffer.as_entire_binding(),
                })
                .collect::<Vec<_>>(),
        });

        SceneBuffers {
            bind_group,
            _buffers: buffers,
        }
    }

    /// Create a render pipeline for the given texture format and fragment entry point.
    pub fn create_pipeline(&self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        self.create_pipeline_with_blend(format, Some(wgpu::BlendState::REPLACE))
    }

    /// Create a render pipeline with an explicit blend state. Formats that are
    /// renderable but not blendable, such as `Rgba32Float`, require `None`.
    pub fn create_pipeline_with_blend(
        &self,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&self.scene_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    range: 0..std::mem::size_of::<shared::ShaderConstants>() as u32,
                }],
            });

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader_module,
                    entry_point: Some("main_vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader_module,
                    entry_point: Some(FRAGMENT_ENTRY),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
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
            })
    }

    /// Render a scene as several accumulation passes and return the averaged image
    /// as linear RGBA floats. Splitting the work keeps any single draw call short
    /// enough to avoid the GPU watchdog, which a few thousand samples in one draw
    /// would otherwise trip.
    ///
    /// The image is rendered a tile at a time, so its size is not limited by what
    /// the adapter will accept as a texture. `on_tile_pass` is called after every
    /// pass with the tile it belongs to, and `on_tile_done` once a tile is
    /// finished and has been placed in the image.
    pub async fn render_to_image_accumulated(
        &self,
        req: &AccumulatedRender<'_>,
        mut on_tile_pass: impl FnMut(&Accumulator),
        mut on_tile_done: impl FnMut(&Accumulator),
    ) -> Result<Vec<f32>, Box<dyn Error>> {
        let tiling = Tiling::new(&self.device, req.width, req.height);
        let mut accumulator = Accumulator::tiled(self, &tiling)?;

        let pixels = (req.width as usize) * (req.height as usize) * 4;
        let mut image = vec![0.0; pixels];

        for tile in tiling.tiles() {
            accumulator.start_tile(tile);

            for _ in 0..req.passes {
                accumulator.pass(self, req.scene, &req.constants, req.samples_per_pass);
                on_tile_pass(&accumulator);
            }

            accumulator.read_into(self, &mut image);
            on_tile_done(&accumulator);
        }

        Ok(image)
    }

    /// Accumulation needs a float target it can both render to and blend into.
    fn check_can_accumulate(&self) -> Result<(), Box<dyn Error>> {
        let features = self.adapter.get_texture_format_features(ACCUM_FORMAT);

        if !features
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return Err(format!("This adapter cannot render to {ACCUM_FORMAT:?}").into());
        }
        if !features
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
            || !self
                .device
                .features()
                .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        {
            return Err(format!("This adapter cannot blend into {ACCUM_FORMAT:?}").into());
        }

        Ok(())
    }

    fn copy_texture_to_buffer(
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        padded_bytes_per_row: u32,
    ) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Map a readback buffer and return its contents as floats, dropping row padding.
    fn read_floats(
        &self,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        padded_bytes_per_row: u32,
        unpadded_bytes_per_row: u32,
    ) -> Vec<f32> {
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        self.device.poll(wgpu::PollType::Wait).unwrap();
        rx.recv().unwrap().unwrap();

        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(bytemuck::cast_slice::<u8, f32>(&data[start..end]));
        }

        drop(data);
        buffer.unmap();

        pixels
    }
}

/// One tile of an image: where it starts and how big it is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// How an image is cut into tiles that the GPU will accept as render targets.
///
/// An image is not required to be a texture. It is required to be *made of*
/// textures, and the largest one this adapter allows is what sets the tile size.
/// A small image comes out as a single tile and nothing about the render changes.
#[derive(Copy, Clone, Debug)]
pub struct Tiling {
    pub image_width: u32,
    pub image_height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
}

impl Tiling {
    /// Cut an image into the largest tiles this device can render and read back.
    ///
    /// Two limits bind. A tile is a texture, so neither side may pass
    /// `max_texture_dimension_2d`; and a tile is read back through one buffer, so
    /// its padded rows may not add up past `max_buffer_size`. The second is what
    /// usually decides the height, since a row of a float image is wide.
    ///
    /// `RTX_MAX_TILE` overrides the texture limit downwards. On hardware that
    /// will happily render A1 in one piece, that is the only way to make the
    /// tiling path run over an image small enough to check by eye: set it low and
    /// the result should still come out identical to the untiled render.
    pub fn new(device: &wgpu::Device, image_width: u32, image_height: u32) -> Self {
        let limits = device.limits();

        let max_dimension = std::env::var("RTX_MAX_TILE")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&v| v > 0)
            .unwrap_or(limits.max_texture_dimension_2d);

        Self::within(
            image_width,
            image_height,
            max_dimension,
            limits.max_buffer_size,
        )
    }

    /// The tiling an image gets under the given limits, apart from any device.
    fn within(
        image_width: u32,
        image_height: u32,
        max_dimension: u32,
        max_buffer_size: u64,
    ) -> Self {
        let tile_width = image_width.min(max_dimension);

        // What one row of a tile costs to read back, padded as the copy requires
        let bytes_per_pixel = ACCUM_FORMAT.block_copy_size(None).expect("Sized format");
        let padded_row = (tile_width * bytes_per_pixel).div_ceil(256) * 256;
        let rows_that_fit = (max_buffer_size / padded_row as u64).max(1);

        let tile_height = image_height
            .min(max_dimension)
            .min(rows_that_fit.try_into().unwrap_or(u32::MAX));

        Self {
            image_width,
            image_height,
            tile_width,
            tile_height,
        }
    }

    pub fn tiles_across(&self) -> u32 {
        self.image_width.div_ceil(self.tile_width)
    }

    pub fn tiles_down(&self) -> u32 {
        self.image_height.div_ceil(self.tile_height)
    }

    pub fn tile_count(&self) -> u32 {
        self.tiles_across() * self.tiles_down()
    }

    /// Every tile of the image, left to right and top to bottom. Tiles at the
    /// right and bottom edges are clipped to what is left of the image rather
    /// than hanging over it, so no ray is traced for a pixel that does not exist.
    pub fn tiles(&self) -> impl Iterator<Item = Tile> + '_ {
        (0..self.tiles_down()).flat_map(move |row| {
            (0..self.tiles_across()).map(move |col| {
                let x = col * self.tile_width;
                let y = row * self.tile_height;

                Tile {
                    x,
                    y,
                    width: self.tile_width.min(self.image_width - x),
                    height: self.tile_height.min(self.image_height - y),
                }
            })
        })
    }
}

/// A float image that passes of a render are summed into.
///
/// Passes are drawn one at a time so that no single draw call is long enough to
/// trip the GPU watchdog, and so that a caller showing progress has something to
/// look at between them.
///
/// The target is one tile of the image being rendered, which for an image small
/// enough to be a texture is the whole of it. The texture is sized for the
/// largest tile once and then reused: [`Accumulator::start_tile`] moves it to the
/// next tile without allocating again.
pub struct Accumulator {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    /// The size of the image being rendered, which every tile shares because it
    /// is what the camera is built from.
    image_width: u32,
    image_height: u32,
    tile: Tile,
    passes_done: u32,
}

impl Accumulator {
    /// An accumulator that will be moved across the tiles of `tiling`, starting
    /// at the first of them.
    pub fn tiled(gpu: &GpuContext, tiling: &Tiling) -> Result<Self, Box<dyn Error>> {
        let first = tiling.tiles().next().expect("An image has at least one tile");

        Self::for_tiles(
            gpu,
            tiling.image_width,
            tiling.image_height,
            tiling.tile_width,
            tiling.tile_height,
            first,
        )
    }

    /// The texture is allocated at the full tile size and kept for every tile,
    /// including the clipped ones along the right and bottom edges. Those draw
    /// into the top left of it and read back only the part they filled.
    fn for_tiles(
        gpu: &GpuContext,
        image_width: u32,
        image_height: u32,
        tile_width: u32,
        tile_height: u32,
        tile: Tile,
    ) -> Result<Self, Box<dyn Error>> {
        let (width, height) = (tile_width, tile_height);

        gpu.check_can_accumulate()?;

        // Each pass adds its samples straight into the target, so the image never
        // leaves the GPU until it is asked for
        let additive = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = gpu.create_pipeline_with_blend(
            ACCUM_FORMAT,
            Some(wgpu::BlendState {
                color: additive,
                alpha: additive,
            }),
        );

        let (texture, view) = Self::target(gpu, width, height);

        Ok(Self {
            texture,
            view,
            pipeline,
            image_width,
            image_height,
            tile,
            passes_done: 0,
        })
    }

    fn target(gpu: &GpuContext, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("accumulation_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ACCUM_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Make this an accumulator for a whole image of a new size, emptied. The
    /// pipeline does not depend on the size and is kept, so this is cheap enough
    /// to do on every step of a window being dragged larger.
    ///
    /// Only for an image that is one tile, which is what a window is. The view
    /// changes, so anything bound to the old one has to be bound again.
    pub fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        let (texture, view) = Self::target(gpu, width, height);

        self.texture = texture;
        self.view = view;
        self.image_width = width;
        self.image_height = height;
        self.start_tile(Tile {
            x: 0,
            y: 0,
            width,
            height,
        });
    }

    /// Throw away what has been summed, so the next pass starts from nothing.
    pub fn reset(&mut self) {
        self.passes_done = 0;
    }

    /// The summed image, for a shader that wants to display it. Its values are
    /// `passes_done()` times brighter than the image being rendered.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn passes_done(&self) -> u32 {
        self.passes_done
    }

    pub fn tile(&self) -> Tile {
        self.tile
    }

    /// Point the accumulator at another tile and empty it, so the passes drawn
    /// next start from nothing rather than from the tile before.
    pub fn start_tile(&mut self, tile: Tile) {
        self.tile = tile;
        self.passes_done = 0;
    }

    /// Draw one more pass of `samples_per_pass` samples per pixel into the image.
    /// `constants` supplies the camera and bounce settings; the resolution, sample
    /// count and seed are this accumulator's to decide.
    ///
    /// Waits for the pass to finish, which keeps a long render from queueing
    /// more work than the watchdog will allow.
    pub fn pass(
        &mut self,
        gpu: &GpuContext,
        scene: &SceneBuffers,
        constants: &shared::ShaderConstants,
        samples_per_pass: u32,
    ) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("accumulation_encoder"),
            });

        self.record_pass(&mut encoder, scene, constants, samples_per_pass);

        gpu.queue.submit(Some(encoder.finish()));
        gpu.device.poll(wgpu::PollType::Wait).unwrap();
    }

    /// Record one more pass into `encoder` without submitting or waiting on it,
    /// for a caller that has more to draw in the same submission, such as a
    /// window showing the result in the same frame.
    pub fn record_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &SceneBuffers,
        constants: &shared::ShaderConstants,
        samples_per_pass: u32,
    ) {
        let push_constants = shared::ShaderConstants {
            // The whole image, not this tile. The camera is built from these, so
            // sending the tile's size would give each tile its own framing and
            // the pieces would not join up
            width: self.image_width,
            height: self.image_height,
            tile_x: self.tile.x,
            tile_y: self.tile.y,
            px_samples: samples_per_pass,
            // Seed 0 would match the live path, so start at 1
            seed: self.passes_done + 1,
            ..*constants
        };

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("accumulation_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Only the first pass starts from an empty image
                        load: if self.passes_done == 0 {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &scene.bind_group, &[]);
            // A tile clipped by the edge of the image is smaller than the
            // texture it is drawn into, and the rest of that texture is not part
            // of the picture. Tracing it would be work thrown away
            rpass.set_scissor_rect(0, 0, self.tile.width, self.tile.height);
            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::bytes_of(&push_constants),
            );
            rpass.draw(0..3, 0..1);
        }

        self.passes_done += 1;
    }

    /// Read this tile back as linear RGBA floats, averaged over the passes drawn
    /// so far. A render stopped early is still a picture, only a noisier one.
    pub fn read(&self, gpu: &GpuContext) -> Vec<f32> {
        let tile = self.tile;
        let bytes_per_pixel = ACCUM_FORMAT.block_copy_size(None).expect("Sized format");
        let unpadded_bytes_per_row = tile.width * bytes_per_pixel;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(256) * 256;

        let output_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("accumulation_readback"),
            size: (padded_bytes_per_row as u64) * (tile.height as u64),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("accumulation_readback_encoder"),
            });
        GpuContext::copy_texture_to_buffer(
            &mut encoder,
            &self.texture,
            &output_buffer,
            tile.width,
            tile.height,
            padded_bytes_per_row,
        );
        gpu.queue.submit(Some(encoder.finish()));
        gpu.device.poll(wgpu::PollType::Wait).unwrap();

        let mut accum = gpu.read_floats(
            &output_buffer,
            tile.width,
            tile.height,
            padded_bytes_per_row,
            unpadded_bytes_per_row,
        );

        let scale = if self.passes_done == 0 {
            0.0
        } else {
            1.0 / self.passes_done as f32
        };
        for value in &mut accum {
            *value *= scale;
        }

        accum
    }

    /// Read this tile back and place it in `image`, a buffer holding the whole
    /// picture as RGBA floats. The tile's rows are not contiguous there, so each
    /// goes to its own offset.
    pub fn read_into(&self, gpu: &GpuContext, image: &mut [f32]) {
        const CHANNELS: usize = 4;

        let tile = self.tile;
        let pixels = self.read(gpu);

        for row in 0..tile.height as usize {
            let src = row * tile.width as usize * CHANNELS;
            let dst = ((tile.y as usize + row) * self.image_width as usize + tile.x as usize)
                * CHANNELS;
            let len = tile.width as usize * CHANNELS;

            image[dst..dst + len].copy_from_slice(&pixels[src..src + len]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5090's limits, which is what the A1 render is cut up under.
    const MAX_DIM: u32 = 16384;
    const MAX_BUFFER: u64 = 1 << 31;

    /// An image inside every limit is one tile, and tiling changes nothing about
    /// how it is rendered.
    #[test]
    fn small_image_is_a_single_tile() {
        let tiling = Tiling::within(800, 600, MAX_DIM, MAX_BUFFER);

        assert_eq!(tiling.tile_count(), 1);
        assert_eq!(
            tiling.tiles().collect::<Vec<_>>(),
            vec![Tile {
                x: 0,
                y: 0,
                width: 800,
                height: 600
            }]
        );
    }

    /// The case that started this: A1 at 300dpi is past the 8192 a texture is
    /// allowed to be under the portable limits.
    #[test]
    fn a1_is_tiled_under_portable_limits() {
        let tiling = Tiling::within(7016, 9933, 8192, MAX_BUFFER);

        assert_eq!(tiling.tile_width, 7016, "width already fits");
        assert_eq!(tiling.tile_height, 8192);
        assert_eq!(tiling.tiles_across(), 1);
        assert_eq!(tiling.tiles_down(), 2);
    }

    /// Tiles have to cover every pixel exactly once, however the image divides.
    #[test]
    fn tiles_cover_the_image_exactly() {
        for (w, h) in [(7016, 9933), (100, 100), (1, 1), (513, 97), (4096, 4096)] {
            let tiling = Tiling::within(w, h, 256, MAX_BUFFER);
            let mut covered = vec![0u8; (w as usize) * (h as usize)];

            for tile in tiling.tiles() {
                for row in 0..tile.height {
                    for col in 0..tile.width {
                        let x = tile.x + col;
                        let y = tile.y + row;

                        assert!(x < w && y < h, "{w}x{h}: tile runs past the image");
                        covered[(y as usize) * (w as usize) + x as usize] += 1;
                    }
                }
            }

            assert!(
                covered.iter().all(|&n| n == 1),
                "{w}x{h}: every pixel belongs to exactly one tile"
            );
        }
    }

    /// A tile is never bigger than the texture the accumulator allocates for it,
    /// which is what the edge tiles are clipped to avoid.
    #[test]
    fn tiles_never_exceed_the_tile_size() {
        let tiling = Tiling::within(7016, 9933, 4096, MAX_BUFFER);

        for tile in tiling.tiles() {
            assert!(tile.width <= tiling.tile_width);
            assert!(tile.height <= tiling.tile_height);
        }
    }

    /// A buffer too small for a whole texture's worth of rows is what decides the
    /// tile height, since a float row of a wide image is expensive to read back.
    #[test]
    fn readback_limit_shortens_tiles() {
        // 4096 pixels of RGBA32F is 65536 bytes a row, so 64MB is 1024 rows
        let tiling = Tiling::within(4096, 4096, MAX_DIM, 64 << 20);

        assert_eq!(tiling.tile_width, 4096);
        assert_eq!(tiling.tile_height, 1024);
        assert_eq!(tiling.tile_count(), 4);
    }
}
