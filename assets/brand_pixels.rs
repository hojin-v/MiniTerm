// Geometry and colors match miniterm.svg; supersample for small tray icons.
fn segment_distance(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((x - a.0 - t * dx).powi(2) + (y - a.1 - t * dy).powi(2)).sqrt()
}
pub fn rgba(size: usize) -> Vec<u8> {
    let mut pixels = vec![0; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0u32; 4];
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = (x as f32 + (sx as f32 + 0.5) / 4.0) * 24.0 / size as f32;
                    let py = (y as f32 + (sy as f32 + 0.5) / 4.0) * 24.0 / size as f32;
                    let qx = ((px - 12.0).abs() - 4.5).max(0.0);
                    let qy = ((py - 12.0).abs() - 4.5).max(0.0);
                    if qx * qx + qy * qy > 36.0 {
                        continue;
                    }
                    let color = if segment_distance(px, py, (5.5, 17.0), (5.5, 7.0)) <= 1.0
                        || segment_distance(px, py, (5.5, 7.0), (12.0, 12.5)) <= 1.0
                        || segment_distance(px, py, (12.0, 12.5), (18.5, 7.0)) <= 1.0
                        || segment_distance(px, py, (18.5, 7.0), (18.5, 17.0)) <= 1.0
                        || segment_distance(px, py, (18.5, 17.0), (14.5, 17.0)) <= 1.0
                    {
                        [106, 218, 187]
                    } else {
                        [24, 33, 47]
                    };
                    for i in 0..3 {
                        sum[i] += color[i];
                    }
                    sum[3] += 1;
                }
            }
            let i = (y * size + x) * 4;
            if sum[3] != 0 {
                for c in 0..3 {
                    pixels[i + c] = (sum[c] / sum[3]) as u8;
                }
                pixels[i + 3] = (sum[3] * 255 / 16) as u8;
            }
        }
    }
    pixels
}
