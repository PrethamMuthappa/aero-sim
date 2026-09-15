use log::{info, error};
use wgpu::{BufferDescriptor, BufferUsages, Device, Queue, ShaderModuleDescriptor, ShaderSource};

const N: usize = 64 * 64;

fn read_buffer_f32(device: &Device, staging: &wgpu::Buffer) -> Vec<f32> {
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let out: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
    drop(data);
    staging.unmap();
    out
}

pub fn run_compute_test(device: &Device, queue: &Queue) -> Result<(), Box<dyn std::error::Error>> {
    info!("Compute test: dispatched 64x64, expected all 1.0");
    let storage = device.create_buffer(&BufferDescriptor {
        label: Some("storage"),
        size: (N * 4) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let staging = device.create_buffer(&BufferDescriptor {
        label: Some("staging"),
        size: (N * 4) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage.as_entire_binding(),
        }],
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    info!("creating shader module");
    let sm = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(include_str!("../../shaders/test.wgsl").into()),
    });
    info!("shader module created ok");
    let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pl),
        module: &sm,
        entry_point: "main",
        compilation_options: Default::default(),
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    let _ = &storage;
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipe);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(8, 8, 1);
    }
    enc.copy_buffer_to_buffer(&storage, 0, &staging, 0, (N * 4) as u64);
    queue.submit([enc.finish()]);
    let vals = read_buffer_f32(device, &staging);
    let ok = vals.iter().all(|&v| v == 1.0);
    if ok {
        info!("Compute test: sampled values = [1.0, 1.0, 1.0, 1.0, 1.0]");
        info!("Compute test PASSED (4096/4096 values correct)");
    } else {
        error!("Compute test FAILED");
        return Err("variant 1 failed".into());
    }
    let storage2 = device.create_buffer(&BufferDescriptor {
        label: Some("storage2"),
        size: (N * 4) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage2.as_entire_binding(),
        }],
    });
    let sm2 = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(include_str!("../../shaders/test_index.wgsl").into()),
    });
    let pipe2 = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pl),
        module: &sm2,
        entry_point: "main",
        compilation_options: Default::default(),
    });
    let mut enc2 = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = enc2.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipe2);
        pass.set_bind_group(0, &bg2, &[]);
        pass.dispatch_workgroups(8, 8, 1);
    }
    enc2.copy_buffer_to_buffer(&storage2, 0, &staging, 0, (N * 4) as u64);
    queue.submit([enc2.finish()]);
    let vals2 = read_buffer_f32(device, &staging);
    info!(
        "Compute test (variant): sampled idx values = [{}, {}, {}]",
        vals2[0], vals2[100], vals2[N - 1]
    );
    if vals2[0] == 0.0 && vals2[100] == 100.0 && vals2[N - 1] == 4095.0 {
        info!("Compute test (variant) PASSED");
        Ok(())
    } else {
        error!("Compute test (variant) FAILED");
        Err("variant 2 failed".into())
    }
}
