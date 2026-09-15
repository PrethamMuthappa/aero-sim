use wgpu::{Device, Queue, ShaderModuleDescriptor, ShaderSource};
use std::sync::Arc;

pub const N_TRACERS: u32 = 20000;

pub struct Tracers {
    device: Arc<Device>,
    queue: Arc<Queue>,
    particles: wgpu::Buffer,
    params: wgpu::Buffer,
    compute_pipe: wgpu::ComputePipeline,
    compute_bg: wgpu::BindGroup,
    render_pipe: wgpu::RenderPipeline,
    render_bg: wgpu::BindGroup,
    render_params: wgpu::Buffer,
    frame: u32,
    w: u32,
    h: u32,
}

impl Tracers {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        format: wgpu::TextureFormat,
        w: u32,
        h: u32,
        macro_buf: &wgpu::Buffer,
    ) -> Self {
        let n_bytes = N_TRACERS as u64 * 16;
        let particles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tracers"),
            size: n_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut init = Vec::with_capacity(N_TRACERS as usize * 4);
        let mut rng: u32 = 0x12345678;
        for _ in 0..N_TRACERS {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let fy = (rng >> 8) as f32 / 16777216.0;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let fx = (rng >> 8) as f32 / 16777216.0;
            init.push(1.0 + fx * (w as f32 - 2.0));
            init.push(1.0 + fy * (h as f32 - 2.0));
            init.push(fx * 600.0);
            init.push(f32::from_bits(rng));
        }
        queue.write_buffer(&particles, 0, bytemuck::cast_slice::<f32, u8>(&init));
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tracer-params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let csm = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("tracers"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/tracers.wgsl").into()),
        });
        let cbgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &cbgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: macro_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: particles.as_entire_binding() },
            ],
        });
        let cpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&cbgl],
            push_constant_ranges: &[],
        });
        let compute_pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("tracer-compute"),
            layout: Some(&cpl),
            module: &csm,
            entry_point: "main",
            compilation_options: Default::default(),
        });
        let rsm = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("tracer-render"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/tracer_render.wgsl").into()),
        });
        let render_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tracer-render-params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut rb = [0u8; 16];
        rb[0..4].copy_from_slice(&w.to_le_bytes());
        rb[4..8].copy_from_slice(&h.to_le_bytes());
        queue.write_buffer(&render_params, 0, &rb);
        let rbgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &rbgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: render_params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: particles.as_entire_binding() },
            ],
        });
        let rpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&rbgl],
            push_constant_ranges: &[],
        });
        let render_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tracer-render-pipe"),
            layout: Some(&rpl),
            vertex: wgpu::VertexState {
                module: &rsm,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rsm,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let mut s = Self {
            device, queue, particles, params, compute_pipe, compute_bg,
            render_pipe, render_bg, render_params, frame: 0, w, h,
        };
        s.write_params(1.0, 0.05);
        s
    }

    fn write_params(&self, dt: f32, u_inlet: f32) {
        let mut b = [0u8; 32];
        b[0..4].copy_from_slice(&self.w.to_le_bytes());
        b[4..8].copy_from_slice(&self.h.to_le_bytes());
        b[8..12].copy_from_slice(&self.frame.to_le_bytes());
        b[12..16].copy_from_slice(&dt.to_le_bytes());
        b[16..20].copy_from_slice(&600.0f32.to_le_bytes());
        b[20..24].copy_from_slice(&u_inlet.to_le_bytes());
        self.queue.write_buffer(&self.params, 0, &b);
        let _ = &self.device;
        let _ = &self.render_params;
    }

    pub fn advect_into(&mut self, enc: &mut wgpu::CommandEncoder, steps: u32, u_inlet: f32) {
        self.frame = self.frame.wrapping_add(1);
        self.write_params(steps as f32, u_inlet);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("tracers"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compute_pipe);
        pass.set_bind_group(0, &self.compute_bg, &[]);
        pass.dispatch_workgroups((N_TRACERS + 63) / 64, 1, 1);
    }

    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_pipeline(&self.render_pipe);
        rpass.set_bind_group(0, &self.render_bg, &[]);
        rpass.draw(0..N_TRACERS, 0..1);
    }
}
