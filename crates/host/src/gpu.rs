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
const SCENE_BINDINGS: u32 = 7;

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
    pub fn upload_scene(&self, scene: &crate::scene_data::SceneData) -> SceneBuffers {
        let buffers = vec![
            storage_buffer(&self.device, "instances", &scene.instances),
            storage_buffer(&self.device, "lambertians", &scene.lambertians),
            storage_buffer(&self.device, "metals", &scene.metals),
            storage_buffer(&self.device, "dielectrics", &scene.dielectrics),
            storage_buffer(&self.device, "diffuse_lights", &scene.diffuse_lights),
            storage_buffer(&self.device, "solids", &scene.solids),
            storage_buffer(&self.device, "lights", &scene.lights),
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
    /// would otherwise trip. `on_pass` is called with the number of passes finished.
    pub async fn render_to_image_accumulated(
        &self,
        req: &AccumulatedRender<'_>,
        mut on_pass: impl FnMut(u32),
    ) -> Result<Vec<f32>, Box<dyn Error>> {
        let mut accumulator = Accumulator::new(self, req.width, req.height)?;

        for _ in 0..req.passes {
            accumulator.pass(self, req.scene, &req.constants, req.samples_per_pass);
            on_pass(accumulator.passes_done());
        }

        Ok(accumulator.read(self))
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

/// A float image that passes of a render are summed into.
///
/// Passes are drawn one at a time so that no single draw call is long enough to
/// trip the GPU watchdog, and so that a caller showing progress has something to
/// look at between them.
pub struct Accumulator {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    width: u32,
    height: u32,
    passes_done: u32,
}

impl Accumulator {
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
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

        Ok(Self {
            texture,
            view,
            pipeline,
            width,
            height,
            passes_done: 0,
        })
    }

    /// The summed image, for a shader that wants to display it. Its values are
    /// `passes_done()` times brighter than the image being rendered.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn passes_done(&self) -> u32 {
        self.passes_done
    }

    /// Draw one more pass of `samples_per_pass` samples per pixel into the image.
    /// `constants` supplies the camera and bounce settings; the resolution, sample
    /// count and seed are this accumulator's to decide.
    pub fn pass(
        &mut self,
        gpu: &GpuContext,
        scene: &SceneBuffers,
        constants: &shared::ShaderConstants,
        samples_per_pass: u32,
    ) {
        let push_constants = shared::ShaderConstants {
            width: self.width,
            height: self.height,
            px_samples: samples_per_pass,
            // Seed 0 would match the live path, so start at 1
            seed: self.passes_done + 1,
            ..*constants
        };

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("accumulation_encoder"),
            });

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
            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::bytes_of(&push_constants),
            );
            rpass.draw(0..3, 0..1);
        }

        gpu.queue.submit(Some(encoder.finish()));
        gpu.device.poll(wgpu::PollType::Wait).unwrap();

        self.passes_done += 1;
    }

    /// Read the image back as linear RGBA floats, averaged over the passes drawn
    /// so far. A render stopped early is still a picture, only a noisier one.
    pub fn read(&self, gpu: &GpuContext) -> Vec<f32> {
        let bytes_per_pixel = ACCUM_FORMAT.block_copy_size(None).expect("Sized format");
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(256) * 256;

        let output_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("accumulation_readback"),
            size: (padded_bytes_per_row * self.height) as u64,
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
            self.width,
            self.height,
            padded_bytes_per_row,
        );
        gpu.queue.submit(Some(encoder.finish()));
        gpu.device.poll(wgpu::PollType::Wait).unwrap();

        let mut accum = gpu.read_floats(
            &output_buffer,
            self.width,
            self.height,
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
}
