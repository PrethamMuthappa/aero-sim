struct TracerParams {
    W: u32,
    H: u32,
    frame: u32,
    dt: f32,
    max_age: f32,
    u_inlet: f32,
    pad0: u32,
    pad1: u32,
};

@group(0) @binding(0) var<uniform> tp: TracerParams;
@group(0) @binding(1) var<storage, read> macro_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> particles: array<vec4<f32>>;

fn hash(n: u32) -> f32 {
    var x = n;
    x = (x ^ 61u) ^ (x >> 16u);
    x = x + (x << 3u);
    x = x ^ (x >> 4u);
    x = x * 2654435761u;
    x = x ^ (x >> 15u);
    return f32(x) / 4294967295.0;
}

fn vel_at(px: f32, py: f32) -> vec2<f32> {
    let fx = clamp(px, 0.0, f32(tp.W) - 1.001);
    let fy = clamp(py, 0.0, f32(tp.H) - 1.001);
    let x0 = u32(fx);
    let y0 = u32(fy);
    let x1 = min(x0 + 1u, tp.W - 1u);
    let y1 = min(y0 + 1u, tp.H - 1u);
    let tx = fx - f32(x0);
    let ty = fy - f32(y0);
    let b00 = (y0 * tp.W + x0) * 4u;
    let b10 = (y0 * tp.W + x1) * 4u;
    let b01 = (y1 * tp.W + x0) * 4u;
    let b11 = (y1 * tp.W + x1) * 4u;
    let ux = mix(mix(macro_buf[b00 + 1u], macro_buf[b10 + 1u], tx), mix(macro_buf[b01 + 1u], macro_buf[b11 + 1u], tx), ty);
    let uy = mix(mix(macro_buf[b00 + 2u], macro_buf[b10 + 2u], tx), mix(macro_buf[b01 + 2u], macro_buf[b11 + 2u], tx), ty);
    return vec2<f32>(ux, uy);
}

fn is_solid_at(px: f32, py: f32) -> bool {
    let x = u32(clamp(px, 0.0, f32(tp.W - 1u)));
    let y = u32(clamp(py, 0.0, f32(tp.H - 1u)));
    return macro_buf[(y * tp.W + x) * 4u + 3u] > 0.5;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&particles)) {
        return;
    }
    var p = particles[i];
    var age = p.z + 1.0;
    let seed = bitcast<u32>(p.w);
    let v = vel_at(p.x, p.y);
    var nx = p.x + v.x * tp.dt;
    var ny = p.y + v.y * tp.dt;
    var dead = age > tp.max_age;
    dead = dead || nx < 0.5 || nx > f32(tp.W) - 0.5 || ny < 0.5 || ny > f32(tp.H) - 0.5;
    dead = dead || is_solid_at(nx, ny);
    if (dead) {
        nx = 1.0;
        ny = 1.0 + hash(seed * 7919u + tp.frame * 104729u) * f32(tp.H - 2u);
        age = 0.0;
    }
    particles[i] = vec4<f32>(nx, ny, age, p.w);
}
