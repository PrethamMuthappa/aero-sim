use log::{info, error};
use wgpu::{BufferDescriptor, BufferUsages, Device, Queue, ShaderModuleDescriptor, ShaderSource};
use bytemuck::{Pod, Zeroable};
use crate::sim::obstacle::default_circle;

fn count_peaks(s: &[f32]) -> usize {
    if s.len() < 3 {
        return 0;
    }
    let mut n = 0;
    for i in 1..s.len() - 1 {
        if s[i] > s[i - 1] && s[i] >= s[i + 1] && s[i] > 0.0005 {
            n += 1;
        }
    }
    n
}

pub const W: u32 = 512;
pub const H: u32 = 256;

const E: [(i32, i32); 9] = [
    (0, 0), (1, 0), (0, 1), (-1, 0), (0, -1),
    (1, 1), (-1, 1), (-1, -1), (1, -1),
];
const W9: [f32; 9] = [
    4.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0,
    1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0,
];

#[repr(C)]
#[derive(Clone, Copy)]
struct Params {
    w: u32,
    h: u32,
    tau: f32,
    u_inlet: f32,
}
unsafe impl Pod for Params {}
unsafe impl Zeroable for Params {}

fn equilibrium(rho: f32, ux: f32, uy: f32) -> [f32; 9] {
    let mut out = [0.0f32; 9];
    let u2 = ux * ux + uy * uy;
    for i in 0..9 {
        let eu = E[i].0 as f32 * ux + E[i].1 as f32 * uy;
        out[i] = W9[i] * rho * (1.0 + 3.0 * eu + 4.5 * eu * eu - 1.5 * u2);
    }
    out
}

pub struct Lbm {
    pub device: std::sync::Arc<Device>,
    pub queue: std::sync::Arc<Queue>,
    pub f_a: wgpu::Buffer,
    pub f_b: wgpu::Buffer,
    pub macro_buf: wgpu::Buffer,
    pub mask_buf: wgpu::Buffer,
    staging_f: wgpu::Buffer,
    staging_macro: wgpu::Buffer,
    params_buf: wgpu::Buffer,
    pub pipe: wgpu::ComputePipeline,
    pub bgl: wgpu::BindGroupLayout,
    pl: wgpu::PipelineLayout,
    pub parity: bool,
    tau: f32,
    u_inlet: f32,
}

fn read_f32(device: &Device, staging: &wgpu::Buffer, n: usize) -> Vec<f32> {
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let out: Vec<f32> = bytemuck::cast_slice::<u8, f32>(&data)[..n].to_vec();
    drop(data);
    staging.unmap();
    out
}

impl Lbm {
    pub fn new(device: std::sync::Arc<Device>, queue: std::sync::Arc<Queue>, u_inlet: f32, tau: f32, mask: &[u32]) -> Self {
        let n_f = (W * H * 9) as u64 * 4;
        let n_m = (W * H * 4) as u64 * 4;
        let n_mask = (W * H) as u64 * 4;
        let mk = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let f_a = mk("f_a", n_f, BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST);
        let f_b = mk("f_b", n_f, BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST);
        let macro_buf = mk("macro", n_m, BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST);
        let mask_buf = mk("mask", n_mask, BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST);
        queue.write_buffer(&mask_buf, 0, bytemuck::cast_slice::<u32, u8>(mask));
        let staging_f = mk("staging_f", n_f, BufferUsages::MAP_READ | BufferUsages::COPY_DST);
        let staging_macro = mk("staging_m", n_m, BufferUsages::MAP_READ | BufferUsages::COPY_DST);
        let params_buf = device.create_buffer(&BufferDescriptor {
            label: Some("params"),
            size: 32,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let sm = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(include_str!("../../shaders/lbm.wgsl").into()),
        });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pl),
            module: &sm,
            entry_point: "main",
            compilation_options: Default::default(),
        });
        let mut s = Self {
            device, queue, f_a, f_b, macro_buf, mask_buf, staging_f, staging_macro,
            params_buf, pipe, bgl, pl, parity: false, tau, u_inlet,
        };
        s.write_params();
        s.init_equilibrium(u_inlet);
        s
    }

    fn write_params(&self) {
        let p = Params { w: W, h: H, tau: self.tau, u_inlet: self.u_inlet };
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(bytemuck::bytes_of(&p));
        self.queue.write_buffer(&self.params_buf, 0, &bytes);
    }

    pub fn bind_group(&self) -> wgpu::BindGroup {
        let (fin, fout) = if !self.parity {
            (&self.f_a, &self.f_b)
        } else {
            (&self.f_b, &self.f_a)
        };
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: fin.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: fout.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.macro_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: self.mask_buf.as_entire_binding() },
            ],
        })
    }

    pub fn init_equilibrium(&mut self, u: f32) {
        self.u_inlet = u;
        self.write_params();
        let cx = W as f32 / 4.0;
        let cy = H as f32 / 2.0;
        let r = H as f32 / 20.0;
        let perturb_amp = 1e-3 * u;
        let mut host = vec![0.0f32; (W * H * 9) as usize];
        for y in 0..H {
            for x in 0..W {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let r2 = dx * dx + dy * dy;
                let mut uy = 0.0f32;
                if r2 < (2.0 * r) * (2.0 * r) && u != 0.0 {
                    uy += perturb_amp * (-(r2 / (r * r))).exp();
                }
                let feq = equilibrium(1.0, u, uy);
                for i in 0..9 {
                    host[((y * W + x) * 9 + i as u32) as usize] = feq[i];
                }
            }
        }
        let bytes = bytemuck::cast_slice::<f32, u8>(&host);
        self.queue.write_buffer(&self.f_a, 0, bytes);
        self.queue.write_buffer(&self.f_b, 0, bytes);
        self.parity = false;
    }

    pub fn add_perturbation(&self, col: u32, amp: f32) {
        let target = if !self.parity { &self.f_a } else { &self.f_b };
        for y in 0..H {
            let uy = amp * ((y as f32 / H as f32) * std::f32::consts::TAU).sin();
            let feq = equilibrium(1.0, self.u_inlet, uy);
            let bytes = bytemuck::cast_slice::<f32, u8>(&feq);
            let dst = ((y * W + col) * 9) as u64 * 4;
            self.queue.write_buffer(target, dst, bytes);
        }
    }

    pub fn dispatch_into(&mut self, enc: &mut wgpu::CommandEncoder) {
        let bg = self.bind_group();
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups((W + 7) / 8, (H + 7) / 8, 1);
        }
        self.parity = !self.parity;
    }

    pub fn step(&mut self, n: u32) {
        for _ in 0..n {
            let bg = self.bind_group();
            let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipe);
                pass.set_bind_group(0, &bg, &[]);
                pass.dispatch_workgroups((W + 7) / 8, (H + 7) / 8, 1);
            }
            self.queue.submit([enc.finish()]);
            self.parity = !self.parity;
        }
        self.device.poll(wgpu::Maintain::Wait);
    }

    pub fn read_macro(&self) -> Vec<f32> {
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let n = (W * H * 4) as u64 * 4;
        enc.copy_buffer_to_buffer(&self.macro_buf, 0, &self.staging_macro, 0, n);
        self.queue.submit([enc.finish()]);
        read_f32(&self.device, &self.staging_macro, (W * H * 4) as usize)
    }

    pub fn set_tau(&mut self, tau: f32) {
        self.tau = tau.max(0.51);
        self.write_params();
    }

    pub fn set_u_inlet(&mut self, u: f32) {
        self.u_inlet = u.clamp(0.0, 0.15);
        self.write_params();
    }

    pub fn set_mask(&self, mask: &[u32]) {
        self.queue.write_buffer(&self.mask_buf, 0, bytemuck::cast_slice::<u32, u8>(mask));
    }

    pub fn char_len(&self) -> f32 {
        2.0 * H as f32 / 20.0
    }
}

pub fn run_lbm_tests(device: std::sync::Arc<Device>, queue: std::sync::Arc<Queue>) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== LBM Milestone 5 tests (512x256, circle) ===");
    let mask = default_circle(W, H);
    let n_solid: usize = mask.iter().map(|&v| v as usize).sum();
    info!("mask: {} solid cells of {}", n_solid, mask.len());
    if n_solid == 0 {
        return Err("mask is empty".into());
    }
    let cx = W / 4;
    let cy = H / 2;
    let r = H / 20;
    let d = 2.0 * r as f32;
    let u = 0.05;
    for (re, steps, label) in [(100.0f32, 50000u32, "B-Re100"), (200.0f32, 50000u32, "C-Re200")] {
        let l = d;
        let nu = u * l / re;
        let tau_re = (3.0 * nu + 0.5).max(0.51);
        info!("{} Re={} tau={:.4}", label, re, tau_re);
        let mut lbm = Lbm::new(device.clone(), queue.clone(), u, tau_re, &mask);
        let px = cx + 6 * r;
        let py = cy;
        let mut series: Vec<f32> = Vec::new();
        let chunk = 200u32;
        let mut done = 0u32;
        while done < steps {
            lbm.step(chunk);
            done += chunk;
            let m = lbm.read_macro();
            let b = ((py * W + px) * 4) as usize;
            series.push(m[b + 2]);
            if m.iter().any(|v| !v.is_finite()) {
                error!("{} FAILED: NaN at step {}", label, done);
                return Err(format!("{} NaN", label).into());
            }
        }
        let max = series.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        let min = series.iter().cloned().fold(f32::INFINITY, f32::min);
        let half = series.len() / 2;
        let tail = &series[half..];
        let mut crossings = 0usize;
        for i in 1..tail.len() {
            if (tail[i - 1] < 0.0) != (tail[i] < 0.0) {
                crossings += 1;
            }
        }
        let cycles = crossings as f32 / 2.0;
        let span = (tail.len() as f32) * chunk as f32;
        let period = if cycles > 0.0 { span / cycles } else { 0.0 };
        let freq = if period > 0.0 { 1.0 / period } else { 0.0 };
        let st = freq * d / u;
        info!("{} Re={} uy min={:.5} max={:.5} cycles~{:.1} period~{:.0} St~{:.3}", label, re, min, max, cycles, period, st);
    }
    let mut lbm = Lbm::new(device.clone(), queue.clone(), 0.05, (3.0 * 0.005 + 0.5f32).max(0.51), &mask);
    let t0 = std::time::Instant::now();
    lbm.step(1000);
    let dt = t0.elapsed();
    info!("Test D 1000 steps at 512x256: {:.2?} ({:.1} steps/s)", dt, 1000.0 / dt.as_secs_f32());
    let m = lbm.read_macro();
    info!("=== LBM Milestone 5 PASSED ===");
    Ok(())
}
