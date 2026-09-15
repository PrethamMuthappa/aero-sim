pub fn circle_mask(w: u32, h: u32, cx: f32, cy: f32, r: f32) -> Vec<u32> {
    let mut mask = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r * r {
                mask[(y * w + x) as usize] = 1;
            }
        }
    }
    mask
}

pub fn default_circle(w: u32, h: u32) -> Vec<u32> {
    let cx = w as f32 / 4.0;
    let cy = h as f32 / 2.0;
    let r = h as f32 / 20.0;
    circle_mask(w, h, cx, cy, r)
}
