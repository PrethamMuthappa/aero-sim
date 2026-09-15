use wgpu::{Device, Queue, ShaderModuleDescriptor, ShaderSource};
use std::sync::Arc;

pub struct Renderer {
    pipe: wgpu::RenderPipeline,
    bg: wgpu::BindGroup,
    _rparams: wgpu::Buffer,
    queue: Arc<Queue>,
}

impl Renderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, format: wgpu::TextureFormat, w: u32, h: u32, macro_buf: &wgpu::Buffer) -> Self {
        let sm = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("render"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/render.wgsl").into()),
        });
        let rparams = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rparams"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&w.to_le_bytes());
        bytes[4..8].copy_from_slice(&h.to_le_bytes());
        bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&0.1f32.to_le_bytes());
        queue.write_buffer(&rparams, 0, &bytes);
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: rparams.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: macro_buf.as_entire_binding() },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-pipe"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &sm,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sm,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        Self { pipe, bg, _rparams: rparams, queue }
    }

    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_pipeline(&self.pipe);
        rpass.set_bind_group(0, &self.bg, &[]);
        rpass.draw(0..3, 0..1);
    }

    pub fn set_viz(&self, mode: u32, u_scale: f32, w: u32, h: u32) {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&w.to_le_bytes());
        bytes[4..8].copy_from_slice(&h.to_le_bytes());
        bytes[8..12].copy_from_slice(&mode.to_le_bytes());
        bytes[12..16].copy_from_slice(&u_scale.to_le_bytes());
        self.queue.write_buffer(&self._rparams, 0, &bytes);
    }
}
