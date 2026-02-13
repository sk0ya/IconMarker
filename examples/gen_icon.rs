use ab_glyph::{FontVec, PxScale};
use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use std::fs::File;

fn load_font() -> FontVec {
    let candidates = [
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

fn draw_chevron_pattern(img: &mut RgbaImage, base: [u8; 3], size: u32) {
    let line_color = Rgba([
        base[0].saturating_add(8),
        base[1].saturating_add(5),
        base[2].saturating_add(2),
        255,
    ]);
    let spacing = (size as f32 / 32.0).max(4.0) as i32;
    let half = spacing / 2;

    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let row = y / half;
            let offset = if row % 2 == 0 { 0 } else { half };
            let lx = (x + offset) % spacing;
            let ly = y % half;
            let on_line = (lx - ly).abs() <= 1 || (lx + ly - half).abs() <= 1;
            if on_line {
                img.put_pixel(x as u32, y as u32, line_color);
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
    let bg_rgb = [245u8, 222, 199];
    let grad_start = [120u8, 90, 220, 255];
    let grad_end = [20u8, 170, 130, 255];
    let padding: f32 = 0.16;

    let bg = Rgba([bg_rgb[0], bg_rgb[1], bg_rgb[2], 255]);
    let mut img = RgbaImage::from_pixel(size, size, bg);

    draw_chevron_pattern(&mut img, bg_rgb, size);

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
    let mut text_layer = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    draw_text_mut(&mut text_layer, white, 0, 0, PxScale::from(final_scale), font, text);

    let mut min_x = size;
    let mut min_y = size;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for y in 0..size {
        for x in 0..size {
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
            if src_x < 0 || src_y < 0 || src_x >= size as i32 || src_y >= size as i32 {
                continue;
            }
            let tp = text_layer.get_pixel(src_x as u32, src_y as u32);
            if tp[3] > 0 {
                let t = ((x as f32 + y as f32) / (2.0 * size as f32)).clamp(0.0, 1.0);
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
    let mut icon_dir = IconDir::new(ResourceType::Icon);

    for &s in sizes {
        let resized = if s == 256 {
            base_img.clone()
        } else {
            image::imageops::resize(&base_img, s, s, FilterType::Lanczos3)
        };
        let icon_image = IconImage::from_rgba_data(s, s, resized.into_raw());
        let entry = IconDirEntry::encode_as_png(&icon_image).expect("Failed to encode icon");
        icon_dir.add_entry(entry);
    }

    let file = File::create(ico_path).expect("Failed to create ICO file");
    icon_dir.write(file).expect("Failed to write ICO");
    println!("Saved {ico_path}");
}
