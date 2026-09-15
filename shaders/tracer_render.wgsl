struct TracerRenderParams {
    W: u32,
    H: u32,
    pad0: u32,
    pad1: u32,
};

@group(0) @binding(0) var<uniform> rp: TracerRenderParams;
@group(0) @binding(1) var<storage, read> particles: array<vec4<f32>>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) age_fade: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let p = particles[ii];
    var cx: f32;
    var cy: f32;
    if (vi == 0u) { cx = -1.0; cy = -1.0; }
    else if (vi == 1u) { cx = 1.0; cy = -1.0; }
    else if (vi == 2u) { cx = -1.0; cy = 1.0; }
    else if (vi == 3u) { cx = -1.0; cy = 1.0; }
    else if (vi == 4u) { cx = 1.0; cy = -1.0; }
    else { cx = 1.0; cy = 1.0; }
    let half_px = 1.5;
    let px = p.x + cx * half_px;
    let py = p.y + cy * half_px;
    let nx = (px / f32(rp.W)) * 2.0 - 1.0;
    let ny = 1.0 - (py / f32(rp.H)) * 2.0;
    var out: VsOut;
    out.pos = vec4<f32>(nx, ny, 0.0, 1.0);
    out.age_fade = clamp(1.0 - p.z / 15000.0, 0.15, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, in.age_fade * 0.8);
}
