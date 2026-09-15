@group(0) @binding(0) var<storage, read_write> data: array<f32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.y * 64u + gid.x;
    if (idx >= arrayLength(&data)) { return; }
    data[idx] = f32(idx);
}