struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    var xy = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOut;
    out.pos = vec4<f32>(xy[vi], 0.0, 1.0);
    out.uv = vec2<f32>((xy[vi].x + 1.0) * 0.5, 1.0 - (xy[vi].y + 1.0) * 0.5);
    return out;
}

struct RenderParams {
    W: u32,
    H: u32,
    mode: u32,
    u_scale: f32,
};

@group(0) @binding(0) var<uniform> rparams: RenderParams;
@group(0) @binding(1) var<storage, read> macro_buf: array<f32>;

fn colormap(t: f32) -> vec3<f32> {
    let t4 = clamp(t, 0.0, 1.0) * 4.0;
    let i = floor(t4);
    let f = t4 - i;
    var a: vec3<f32>;
    var b: vec3<f32>;
    if (i < 0.5) {
        a = vec3<f32>(0.0, 0.0, 1.0);
        b = vec3<f32>(0.0, 1.0, 1.0);
    } else if (i < 1.5) {
        a = vec3<f32>(0.0, 1.0, 1.0);
        b = vec3<f32>(0.0, 1.0, 0.0);
    } else if (i < 2.5) {
        a = vec3<f32>(0.0, 1.0, 0.0);
        b = vec3<f32>(1.0, 1.0, 0.0);
    } else {
        a = vec3<f32>(1.0, 1.0, 0.0);
        b = vec3<f32>(1.0, 0.0, 0.0);
    }
    return mix(a, b, f);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let fx = clamp(in.uv.x * f32(rparams.W), 0.0, f32(rparams.W - 1u));
    let fy = clamp(in.uv.y * f32(rparams.H), 0.0, f32(rparams.H - 1u));
    let x = u32(fx);
    let y = u32(fy);
    let base = (y * rparams.W + x) * 4u;
    let solid = macro_buf[base + 3u];
    if (solid > 0.5) {
        return vec4<f32>(0.05, 0.05, 0.05, 1.0);
    }
    let rho = macro_buf[base + 0u];
    let ux = macro_buf[base + 1u];
    let uy = macro_buf[base + 2u];
    if (rparams.mode == 1u) {
        let t = clamp((rho - 1.0) * 20.0 + 0.5, 0.0, 1.0);
        return vec4<f32>(colormap(t), 1.0);
    }
    if (rparams.mode == 2u) {
        let x0 = select(x - 1u, 0u, x == 0u);
        let x1 = select(x + 1u, rparams.W - 1u, x >= rparams.W - 1u);
        let y0 = select(y - 1u, 0u, y == 0u);
        let y1 = select(y + 1u, rparams.H - 1u, y >= rparams.H - 1u);
        let b_x0 = (y * rparams.W + x0) * 4u;
        let b_x1 = (y * rparams.W + x1) * 4u;
        let b_y0 = (y0 * rparams.W + x) * 4u;
        let b_y1 = (y1 * rparams.W + x) * 4u;
        let duy_dx = (macro_buf[b_x1 + 2u] - macro_buf[b_x0 + 2u]) / f32(x1 - x0 + select(0u, 1u, x1 == x0));
        let dvx_dy = (macro_buf[b_y1 + 1u] - macro_buf[b_y0 + 1u]) / f32(y1 - y0 + select(0u, 1u, y1 == y0));
        let vort = duy_dx - dvx_dy;
        let t = clamp(vort * 20.0 + 0.5, 0.0, 1.0);
        return vec4<f32>(colormap(t), 1.0);
    }
    let speed = sqrt(ux * ux + uy * uy);
    let t = speed / max(rparams.u_scale, 0.001);
    return vec4<f32>(colormap(t), 1.0);
}
