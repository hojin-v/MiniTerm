mod brand_pixels;
fn main() {
    let sizes = [16usize, 24, 32, 48, 64, 128, 256];
    let mut icon = vec![0, 0, 1, 0, sizes.len() as u8, 0];
    let mut offset = 6 + 16 * sizes.len();
    let mut bitmaps = Vec::new();
    for size in sizes {
        let rgba = brand_pixels::rgba(size);
        let mask_stride = size.div_ceil(32) * 4;
        let image_size = size * size * 4 + size * mask_stride;
        let mut bitmap = Vec::new();
        for n in [40u32, size as u32, (size * 2) as u32] { bitmap.extend(n.to_le_bytes()); }
        bitmap.extend(1u16.to_le_bytes());
        bitmap.extend(32u16.to_le_bytes());
        for n in [0u32, image_size as u32, 0, 0, 0, 0] { bitmap.extend(n.to_le_bytes()); }
        for y in (0..size).rev() {
            for x in 0..size {
                let p = (y * size + x) * 4;
                bitmap.extend([rgba[p + 2], rgba[p + 1], rgba[p], rgba[p + 3]]);
            }
        }
        for y in (0..size).rev() {
            let mut mask = vec![0; mask_stride];
            for x in 0..size {
                if rgba[(y * size + x) * 4 + 3] == 0 { mask[x / 8] |= 0x80 >> (x % 8); }
            }
            bitmap.extend(mask);
        }
        icon.extend([size as u8, size as u8, 0, 0]);
        icon.extend(1u16.to_le_bytes());
        icon.extend(32u16.to_le_bytes());
        icon.extend((bitmap.len() as u32).to_le_bytes());
        icon.extend((offset as u32).to_le_bytes());
        offset += bitmap.len();
        bitmaps.push(bitmap);
    }
    for bitmap in bitmaps { icon.extend(bitmap); }
    std::fs::write(std::env::args().nth(1).expect("output .ico path"), icon).unwrap();
}
