use ab_glyph::{FontVec, PxScale};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use std::fs::File;
use std::io::{Cursor, Write};

fn load_font() -> FontVec {
    let candidates = [
        concat!(env!("CARGO_MANIFEST_DIR"), r"\fonts\AoboshiOne-Regular.ttf"),
        r"C:\Users\koya\AppData\Local\Microsoft\Windows\Fonts\AoboshiOne-Regular.ttf",
        r"C:\Windows\Fonts\AoboshiOne-Regular.ttf",
        r"C:\Windows\Fonts\meiryob.ttc",
        r"C:\Windows\Fonts\YuGothB.ttc",
        r"C:\Windows\Fonts\msgothic.ttc",
        r"C:\Windows\Fonts\arialbd.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(font) = FontVec::try_from_vec_and_index(data.clone(), 0) {
                return font;
            }
            if let Ok(font) = FontVec::try_from_vec(data) {
                return font;
            }
        }
    }
    panic!("No suitable font found");
}

fn lerp_color(a: [u8; 4], b: [u8; 4], t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    Rgba([
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
        255,
    ])
}

fn draw_chevron_pattern(img: &mut RgbaImage, size: u32) {
    let width = size as i32;
    let height = size as i32;

    let spacing = 6;
    let zigzag_period = 20;

    for y in 0..height {
        for x in 0..width {
            let half = zigzag_period / 2;
            let zigzag = ((x % zigzag_period) - half).abs();
            let pattern_y = (y + zigzag).rem_euclid(spacing);

            let bg = *img.get_pixel(x as u32, y as u32);

            if pattern_y == 0 {
                img.put_pixel(x as u32, y as u32, Rgba([
                    bg[0].saturating_add(10),
                    bg[1].saturating_add(10),
                    bg[2].saturating_add(10),
                    255,
                ]));
            } else if pattern_y == 1 {
                img.put_pixel(x as u32, y as u32, Rgba([
                    bg[0].saturating_sub(6),
                    bg[1].saturating_sub(6),
                    bg[2].saturating_sub(6),
                    255,
                ]));
            }
        }
    }
}

fn measure_text_bbox(font: &FontVec, text: &str, scale: f32, canvas: u32) -> Option<(u32, u32, u32, u32)> {
    let white = Rgba([255u8, 255, 255, 255]);
    let mut tmp = RgbaImage::from_pixel(canvas, canvas, Rgba([0, 0, 0, 0]));
    draw_text_mut(&mut tmp, white, 0, 0, PxScale::from(scale), font, text);

    let mut min_x = canvas;
    let mut min_y = canvas;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for y in 0..canvas {
        for x in 0..canvas {
            if tmp.get_pixel(x, y)[3] > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if max_x >= min_x && max_y >= min_y {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

fn generate_image(font: &FontVec, text: &str, size: u32) -> RgbaImage {
    let bg_rgb = [242u8, 220, 198];
    let grad_start = [120u8, 90, 220, 255];
    let grad_end = [20u8, 170, 130, 255];
    let padding: f32 = 0.16;

    let bg = Rgba([bg_rgb[0], bg_rgb[1], bg_rgb[2], 255]);
    let mut img = RgbaImage::from_pixel(size, size, bg);

    draw_chevron_pattern(&mut img, size);

    let ref_scale = 200.0_f32;
    let ref_canvas = 512u32;
    let bbox = match measure_text_bbox(font, text, ref_scale, ref_canvas) {
        Some(b) => b,
        None => return img,
    };
    let ref_w = (bbox.2 - bbox.0 + 1) as f32;
    let ref_h = (bbox.3 - bbox.1 + 1) as f32;

    let target = size as f32 * (1.0 - padding * 2.0);
    let ratio = (target / ref_w).min(target / ref_h);
    let final_scale = ref_scale * ratio;

    let white = Rgba([255u8, 255, 255, 255]);
    let tmp_size = size * 2;
    let mut text_layer = RgbaImage::from_pixel(tmp_size, tmp_size, Rgba([0, 0, 0, 0]));
    draw_text_mut(&mut text_layer, white, 0, 0, PxScale::from(final_scale), font, text);

    let mut min_x = tmp_size;
    let mut min_y = tmp_size;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for y in 0..tmp_size {
        for x in 0..tmp_size {
            if text_layer.get_pixel(x, y)[3] > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    let glyph_w = max_x - min_x + 1;
    let glyph_h = max_y - min_y + 1;
    let offset_x = (size - glyph_w) as i32 / 2 - min_x as i32;
    let offset_y = (size - glyph_h) as i32 / 2 - min_y as i32;

    for y in 0..size {
        for x in 0..size {
            let src_x = x as i32 - offset_x;
            let src_y = y as i32 - offset_y;
            if src_x < 0 || src_y < 0 || src_x >= tmp_size as i32 || src_y >= tmp_size as i32 {
                continue;
            }
            let tp = text_layer.get_pixel(src_x as u32, src_y as u32);
            if tp[3] > 0 {
                let t = ((x as f32 + (size as f32 - y as f32)) / (2.0 * size as f32)).clamp(0.0, 1.0);
                let grad = lerp_color(grad_start, grad_end, t);
                let alpha = tp[3] as f32 / 255.0;
                let bg_px = img.get_pixel(x, y);
                let blended = Rgba([
                    (grad[0] as f32 * alpha + bg_px[0] as f32 * (1.0 - alpha)) as u8,
                    (grad[1] as f32 * alpha + bg_px[1] as f32 * (1.0 - alpha)) as u8,
                    (grad[2] as f32 * alpha + bg_px[2] as f32 * (1.0 - alpha)) as u8,
                    255,
                ]);
                img.put_pixel(x, y, blended);
            }
        }
    }

    img
}

fn main() {
    let font = load_font();
    let base_img = generate_image(&font, "I", 256);

    // Save PNG
    let png_path = "icon.png";
    base_img.save(png_path).expect("Failed to save PNG");
    println!("Saved {png_path}");

    // Save ICO
    let ico_path = "icon.ico";
    let sizes: &[u32] = &[16, 32, 48, 256];
    write_ico(ico_path, &base_img, sizes).expect("Failed to write ICO");
    println!("Saved {ico_path}");
}

/// Write an ICO file with multiple sizes.
/// Small sizes use 32-bit BGRA BMP DIB, 256x256 uses RGBA PNG.
fn write_ico(
    path: &str,
    base_img: &RgbaImage,
    sizes: &[u32],
) -> std::io::Result<()> {
    struct IcoEntry {
        width: u8,
        height: u8,
        planes: u16,
        bpp: u16,
        data: Vec<u8>,
    }

    let mut entries = Vec::new();

    for &s in sizes {
        let resized = if s == 256 {
            base_img.clone()
        } else {
            image::imageops::resize(base_img, s, s, FilterType::Lanczos3)
        };

        if s >= 256 {
            let mut buf = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(resized)
                .write_to(&mut buf, ImageFormat::Png)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            entries.push(IcoEntry {
                width: 0,
                height: 0,
                planes: 1,
                bpp: 32,
                data: buf.into_inner(),
            });
        } else {
            let mut data = Vec::new();

            // BITMAPINFOHEADER (40 bytes)
            data.extend_from_slice(&40u32.to_le_bytes());
            data.extend_from_slice(&(s as i32).to_le_bytes());
            data.extend_from_slice(&(2 * s as i32).to_le_bytes()); // height doubled
            data.extend_from_slice(&1u16.to_le_bytes());
            data.extend_from_slice(&32u16.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes()); // compression
            data.extend_from_slice(&0u32.to_le_bytes()); // image size
            data.extend_from_slice(&0i32.to_le_bytes()); // x ppm
            data.extend_from_slice(&0i32.to_le_bytes()); // y ppm
            data.extend_from_slice(&0u32.to_le_bytes()); // colors used
            data.extend_from_slice(&0u32.to_le_bytes()); // colors important

            // XOR pixel data: BGRA, bottom-up
            for row in (0..s).rev() {
                for col in 0..s {
                    let px = resized.get_pixel(col, row);
                    data.push(px[2]); // B
                    data.push(px[1]); // G
                    data.push(px[0]); // R
                    data.push(px[3]); // A
                }
            }

            // AND mask: all zeros, rows padded to 4-byte boundary
            let mask_row_bytes = ((s + 31) / 32) * 4;
            data.extend(std::iter::repeat(0u8).take((mask_row_bytes * s) as usize));

            entries.push(IcoEntry {
                width: s as u8,
                height: s as u8,
                planes: 1,
                bpp: 32,
                data,
            });
        }
    }

    let mut file = File::create(path)?;
    let count = entries.len() as u16;

    // ICONDIR header
    file.write_all(&0u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&count.to_le_bytes())?;

    let dir_end = 6u32 + 16 * count as u32;
    let mut offset = dir_end;

    // ICONDIRENTRY for each image
    for entry in &entries {
        file.write_all(&[entry.width, entry.height, 0, 0])?;
        file.write_all(&entry.planes.to_le_bytes())?;
        file.write_all(&entry.bpp.to_le_bytes())?;
        file.write_all(&(entry.data.len() as u32).to_le_bytes())?;
        file.write_all(&offset.to_le_bytes())?;
        offset += entry.data.len() as u32;
    }

    for entry in &entries {
        file.write_all(&entry.data)?;
    }

    Ok(())
}
