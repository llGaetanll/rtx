//! Putting an accumulated float image on screen.
//!
//! Both windows hold their picture in a float texture that is summed or averaged
//! into over many passes, and neither can hand that straight to the swapchain.
//! This draws it over the window instead, scaled to fit and encoded by the
//! surface on the way out.

use crate::gpu::GpuContext;

/// The pipeline that puts the accumulated image on screen.
pub struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl Blit {
    pub fn new(
        gpu: &GpuContext,
        accumulated: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> Self {
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

        let bind_group = Self::bind_group(gpu, &layout, accumulated);

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
            layout,
            bind_group,
        }
    }

    /// Show a different image, such as one reallocated for a resized window,
    /// without building the pipeline again.
    pub fn rebind(&mut self, gpu: &GpuContext, accumulated: &wgpu::TextureView) {
        self.bind_group = Self::bind_group(gpu, &self.layout, accumulated);
    }

    fn bind_group(
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        accumulated: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(accumulated),
            }],
        })
    }

    /// Record drawing the image over the whole of `target`. The margin the
    /// image does not cover is cleared to black.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        constants: &shared::BlitConstants,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_push_constants(
            wgpu::ShaderStages::FRAGMENT,
            0,
            bytemuck::bytes_of(constants),
        );
        rpass.draw(0..3, 0..1);
    }
}

/// The swapchain format to blit into.
///
/// The blit shader writes linear color and lets the surface encode it, the same
/// transfer the offscreen render path gets from its sRGB target. Taking whatever
/// format the driver lists first would skip that on some drivers and show the
/// image too dark.
pub fn surface_format(surface: &wgpu::Surface<'_>, adapter: &wgpu::Adapter) -> wgpu::TextureFormat {
    let capabilities = surface.get_capabilities(adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .unwrap_or(capabilities.formats[0]);

    if !format.is_srgb() {
        log::warn!("no sRGB surface format available, the window will look dark");
    }

    format
}
