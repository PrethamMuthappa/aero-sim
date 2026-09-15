struct Params {
    W: u32,
    H: u32,
    tau: f32,
    u_inlet: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> f_in: array<f32>;
@group(0) @binding(2) var<storage, read_write> f_out: array<f32>;
@group(0) @binding(3) var<storage, read_write> fout_macro: array<f32>;
@group(0) @binding(4) var<storage, read> mask: array<u32>;

fn ex(i: u32) -> i32 {
    if (i == 0u) { return 0; }
    if (i == 1u) { return 1; }
    if (i == 2u) { return 0; }
    if (i == 3u) { return -1; }
    if (i == 4u) { return 0; }
    if (i == 5u) { return 1; }
    if (i == 6u) { return -1; }
    if (i == 7u) { return -1; }
    return 1;
}

fn ey(i: u32) -> i32 {
    if (i == 0u) { return 0; }
    if (i == 1u) { return 0; }
    if (i == 2u) { return 1; }
    if (i == 3u) { return 0; }
    if (i == 4u) { return -1; }
    if (i == 5u) { return 1; }
    if (i == 6u) { return 1; }
    if (i == 7u) { return -1; }
    return -1;
}

fn w9(i: u32) -> f32 {
    if (i == 0u) { return 4.0 / 9.0; }
    if (i <= 4u) { return 1.0 / 9.0; }
    return 1.0 / 36.0;
}

fn opp(i: u32) -> u32 {
    if (i == 0u) { return 0u; }
    if (i == 1u) { return 3u; }
    if (i == 2u) { return 4u; }
    if (i == 3u) { return 1u; }
    if (i == 4u) { return 2u; }
    if (i == 5u) { return 7u; }
    if (i == 6u) { return 8u; }
    if (i == 7u) { return 5u; }
    return 6u;
}

fn idx(x: u32, y: u32, i: u32) -> u32 {
    return (y * params.W + x) * 9u + i;
}

fn midx(x: u32, y: u32, c: u32) -> u32 {
    return (y * params.W + x) * 4u + c;
}

fn is_solid(x: u32, y: u32) -> bool {
    return mask[y * params.W + x] == 1u;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.W || y >= params.H) {
        return;
    }
    let W = params.W;
    let H = params.H;
    let is_solid_cell = is_solid(x, y);
    if (is_solid_cell) {
        fout_macro[midx(x, y, 0u)] = 1.0;
        fout_macro[midx(x, y, 1u)] = 0.0;
        fout_macro[midx(x, y, 2u)] = 0.0;
        fout_macro[midx(x, y, 3u)] = 1.0;
        return;
    }
    var f: array<f32, 9>;
    if (x == W - 1u) {
        for (var i: u32 = 0u; i < 9u; i = i + 1u) {
            f[i] = f_in[idx(x - 1u, y, i)];
        }
    } else {
        for (var i: u32 = 0u; i < 9u; i = i + 1u) {
            f[i] = f_in[idx(x, y, i)];
        }
    }
    var rho: f32 = 0.0;
    var ux: f32 = 0.0;
    var uy: f32 = 0.0;
    for (var i: u32 = 0u; i < 9u; i = i + 1u) {
        rho += f[i];
        ux += f[i] * f32(ex(i));
        uy += f[i] * f32(ey(i));
    }
    ux /= rho;
    uy /= rho;
    let is_inlet = x == 0u;
    let is_wall = y == 0u || y == H - 1u;
    if (is_inlet) {
        rho = 1.0;
        ux = params.u_inlet;
        uy = 0.0;
        for (var i: u32 = 0u; i < 9u; i = i + 1u) {
            let eu = f32(ex(i)) * ux + f32(ey(i)) * uy;
            f[i] = w9(i) * rho * (1.0 + 3.0 * eu + 4.5 * eu * eu - 1.5 * (ux * ux + uy * uy));
        }
    }
    var feq: array<f32, 9>;
    for (var i: u32 = 0u; i < 9u; i = i + 1u) {
        let eu = f32(ex(i)) * ux + f32(ey(i)) * uy;
        feq[i] = w9(i) * rho * (1.0 + 3.0 * eu + 4.5 * eu * eu - 1.5 * (ux * ux + uy * uy));
    }
    let omega = 1.0 / params.tau;
    for (var i: u32 = 0u; i < 9u; i = i + 1u) {
        f[i] = f[i] - omega * (f[i] - feq[i]);
    }
    if (is_wall && !is_inlet) {
        var bounced: array<f32, 9>;
        for (var i: u32 = 0u; i < 9u; i = i + 1u) {
            bounced[i] = f[opp(i)];
        }
        f = bounced;
    }
    for (var i: u32 = 0u; i < 9u; i = i + 1u) {
        let nx = i32(x) + ex(i);
        let ny = i32(y) + ey(i);
        let off_domain = nx < 0 || nx >= i32(W) || ny < 0 || ny >= i32(H);
        let hits_solid = !off_domain && is_solid(u32(nx), u32(ny));
        if (off_domain || hits_solid) {
            f_out[idx(x, y, opp(i))] = f[i];
        } else {
            f_out[idx(u32(nx), u32(ny), i)] = f[i];
        }
    }
    fout_macro[midx(x, y, 0u)] = rho;
    fout_macro[midx(x, y, 1u)] = ux;
    fout_macro[midx(x, y, 2u)] = uy;
    fout_macro[midx(x, y, 3u)] = 0.0;
}
