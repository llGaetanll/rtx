use std::error::Error;

use wgpu::InstanceDescriptor;
use wgpu::include_spirv;
use wgpu::include_spirv_raw;
use wgpu::{self};

pub struct GpuContext {
    pub instance: wgpu::Instance,
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
            let spirv = include_spirv_raw!(env!("shader.spv"));
            unsafe { device.create_shader_module_passthrough(spirv) }
        } else {
            device.create_shader_module(include_spirv!(env!("shader.spv")))
        };

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            shader_module,
        })
    }

    /// Create a render pipeline for the given texture format.
    pub fn create_pipeline(&self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
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
                    entry_point: Some("main_fs"),
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
            })
    }
}
