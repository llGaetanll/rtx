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
    pub width: u32,
    pub height: u32,
    pub entry_point: &'a str,
    pub passes: u32,
    pub samples_per_pass: u32,
    /// Camera and bounce settings. `width`, `height`, `px_samples` and `seed`
    /// are filled in per pass and may be left at their defaults here.
    pub constants: shared::ShaderConstants,
}

pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub shader_module: wgpu::ShaderModule,
}

impl GpuContext {
    pub fn create_instance() -> wgpu::Instance {
        let mut instance_flags = wgpu::InstanceFlags::default();
        instance_flags.remove(wgpu::InstanceFlags::VALIDATION);
        instance_flags.remove(wgpu::InstanceFlags::DEBUG);

        wgpu::Instance::new(&InstanceDescriptor {
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

        Ok(Self {
            adapter,
            device,
            queue,
            shader_module,
        })
    }

    /// Create a render pipeline for the given texture format and fragment entry point.
    pub fn create_pipeline(
        &self,
        format: wgpu::TextureFormat,
        fragment_entry_point: &str,
    ) -> wgpu::RenderPipeline {
        self.create_pipeline_with_blend(
            format,
            fragment_entry_point,
            Some(wgpu::BlendState::REPLACE),
        )
    }

    /// Create a render pipeline with an explicit blend state. Formats that are
    /// renderable but not blendable, such as `Rgba32Float`, require `None`.
    pub fn create_pipeline_with_blend(
        &self,
        format: wgpu::TextureFormat,
        fragment_entry_point: &str,
        blend: Option<wgpu::BlendState>,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
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
                    entry_point: Some(fragment_entry_point),
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

    /// Render the scene in a single pass and return the pixel data as RGBA bytes.
    pub async fn render_to_image(
        &self,
        fragment_entry_point: &str,
        constants: shared::ShaderConstants,
    ) -> Vec<u8> {
        let width = constants.width;
        let height = constants.height;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        // Create texture to render to
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ACCUM_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create pipeline for this format
        let pipeline = self.create_pipeline(format, fragment_entry_point);

        // Bytes per row must be aligned to 256
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let padded_bytes_per_row = (unpadded_bytes_per_row + 255) & !255;

        // Create buffer to copy texture data into
        let buffer_size = (padded_bytes_per_row * height) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Create command encoder and render
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offscreen_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
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

            rpass.set_pipeline(&pipeline);
            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::bytes_of(&constants),
            );
            rpass.draw(0..3, 0..1);
        }

        // Copy texture to buffer
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
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

        self.queue.submit(Some(encoder.finish()));

        // Map the buffer and read the data
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        self.device.poll(wgpu::PollType::Wait).unwrap();
        rx.recv().unwrap().unwrap();

        // Copy data out, removing row padding if necessary
        let data = buffer_slice.get_mapped_range();
        let mut result = Vec::with_capacity((width * height * bytes_per_pixel) as usize);

        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            result.extend_from_slice(&data[start..end]);
        }

        drop(data);
        output_buffer.unmap();

        result
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
        let width = req.width;
        let height = req.height;

        self.check_can_accumulate()?;

        // Each pass adds its samples straight into the target, so the image never
        // leaves the GPU until it is complete
        let additive = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = self.create_pipeline_with_blend(
            ACCUM_FORMAT,
            req.entry_point,
            Some(wgpu::BlendState {
                color: additive,
                alpha: additive,
            }),
        );

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bytes_per_pixel = ACCUM_FORMAT.block_copy_size(None).expect("Sized format");
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(256) * 256;

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("accumulation_readback"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        for pass in 0..req.passes {
            let push_constants = shared::ShaderConstants {
                width,
                height,
                px_samples: req.samples_per_pass,
                // Seed 0 would match the live/test path, so start at 1
                seed: pass + 1,
                ..req.constants
            };

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("accumulation_encoder"),
                });

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("accumulation_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // Only the first pass starts from an empty image
                            load: if pass == 0 {
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

                rpass.set_pipeline(&pipeline);
                rpass.set_push_constants(
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    0,
                    bytemuck::bytes_of(&push_constants),
                );
                rpass.draw(0..3, 0..1);
            }

            self.queue.submit(Some(encoder.finish()));
            self.device.poll(wgpu::PollType::Wait).unwrap();

            on_pass(pass + 1);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("accumulation_readback_encoder"),
            });
        Self::copy_texture_to_buffer(
            &mut encoder,
            &texture,
            &output_buffer,
            width,
            height,
            padded_bytes_per_row,
        );
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::PollType::Wait).unwrap();

        let mut accum = self.read_floats(
            &output_buffer,
            width,
            height,
            padded_bytes_per_row,
            unpadded_bytes_per_row,
        );

        let scale = 1.0 / req.passes as f32;
        for value in &mut accum {
            *value *= scale;
        }

        Ok(accum)
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
