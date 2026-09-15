use std::path::{Path, PathBuf};

pub struct ObstacleSource {
    pub mask: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub name: String,
}

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

pub fn from_circle(w: u32, h: u32) -> ObstacleSource {
    ObstacleSource {
        mask: default_circle(w, h),
        width: w,
        height: h,
        name: "circle".to_string(),
    }
}

pub fn from_png(path: &Path, target_w: u32, target_h: u32) -> Result<ObstacleSource, String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (iw, ih) = (rgba.width(), rgba.height());
    if iw == 0 || ih == 0 {
        return Err("empty image".to_string());
    }
    let has_alpha = rgba.pixels().any(|p| p[3] < 250);
    let gray = image::imageops::grayscale(&rgba);
    let vals: Vec<bool> = rgba
        .pixels()
        .zip(gray.pixels())
        .map(|(c, g)| {
            if has_alpha {
                c[3] > 128
            } else {
                g[0] < 128
            }
        })
        .collect();
    let scale = f32::min(target_w as f32 / iw as f32, target_h as f32 / ih as f32) * 0.75;
    let new_w = ((iw as f32 * scale) as u32).max(1);
    let new_h = ((ih as f32 * scale) as u32).max(1);
    let mut solid_img = image::GrayImage::new(iw, ih);
    for (i, &s) in vals.iter().enumerate() {
        let x = (i as u32) % iw;
        let y = (i as u32) / iw;
        solid_img.put_pixel(x, y, image::Luma([if s { 255u8 } else { 0u8 }]));
    }
    let resized = image::imageops::resize(&solid_img, new_w, new_h, image::imageops::FilterType::Lanczos3);
    let ox = target_w.saturating_sub(new_w) / 2;
    let oy = target_h.saturating_sub(new_h) / 2;
    let mut mask = vec![0u32; (target_w * target_h) as usize];
    for y in 0..new_h {
        for x in 0..new_w {
            if resized.get_pixel(x, y)[0] > 128 {
                let gx = ox + x;
                let gy = oy + y;
                if gx < target_w && gy < target_h {
                    mask[(gy * target_w + gx) as usize] = 1;
                }
            }
        }
    }
    let mut solid_count: usize = mask.iter().map(|&v| v as usize).sum();
    if solid_count > mask.len() / 2 {
        for v in mask.iter_mut() {
            *v = 1 - *v;
        }
        solid_count = mask.len() - solid_count;
    }
    if solid_count == 0 {
        return Err("no solid pixels after threshold".to_string());
    }
    if solid_count > mask.len() * 9 / 10 {
        return Err("obstacle covers >90% of domain".to_string());
    }
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    Ok(ObstacleSource {
        mask,
        width: target_w,
        height: target_h,
        name,
    })
}

pub fn load_or_circle(path: &Path, w: u32, h: u32) -> ObstacleSource {
    if path.to_string_lossy() == "__circle__" {
        return from_circle(w, h);
    }
    match from_png(&PathBuf::from(path), w, h) {
        Ok(o) => o,
        Err(_) => from_circle(w, h),
    }
}
