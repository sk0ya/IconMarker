#![windows_subsystem = "windows"]

use ab_glyph::{FontVec, PxScale};
use eframe::egui;
use egui::color_picker::{color_edit_button_srgba, Alpha};
use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use rfd::FileDialog;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

fn load_font() -> FontVec {
    // Aoboshi One を優先的に探す
    let candidates = [
        // バンドルフォント (exe と同じディレクトリの fonts/)
        concat!(env!("CARGO_MANIFEST_DIR"), r"\fonts\AoboshiOne-Regular.ttf"),
        // システムフォント (ユーザーインストール)
        r"C:\Users\koya\AppData\Local\Microsoft\Windows\Fonts\AoboshiOne-Regular.ttf",
        r"C:\Windows\Fonts\AoboshiOne-Regular.ttf",
        // フォールバック
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
    panic!("No suitable font found in system fonts");
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    Rgba([
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        255,
    ])
}

fn draw_chevron_pattern(img: &mut RgbaImage, _base: Color32, size: u32) {
    let width = size as i32;
    let height = size as i32;

    let spacing = 6;          // 線の間隔
    let zigzag_period = 20;   // ジグザグ1周期の幅（px）

    for y in 0..height {
        for x in 0..width {
            // 三角波（繰り返しジグザグ）で複数のV字折り返しを作る
            let half = zigzag_period / 2;
            let zigzag = ((x % zigzag_period) - half).abs();
            let pattern_y = (y + zigzag).rem_euclid(spacing);

            let bg = *img.get_pixel(x as u32, y as u32);

            if pattern_y == 0 {
                // 微かなハイライト線
                img.put_pixel(x as u32, y as u32, Rgba([
                    bg[0].saturating_add(10),
                    bg[1].saturating_add(10),
                    bg[2].saturating_add(10),
                    255,
                ]));
            } else if pattern_y == 1 {
                // ハイライト直下の微かなシャドウで立体感を出す
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


fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([620.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "IconMarker",
        options,
        Box::new(|_cc| Ok(Box::new(IconMarkerApp::new()))),
    )
}

struct IconMarkerApp {
    text: String,
    bg_color: Color32,
    grad_start: Color32,
    grad_end: Color32,
    padding: f32, // 0.0..=0.4 — fraction of canvas used as margin
    chevron_on: bool,
    texture: Option<TextureHandle>,
    needs_update: bool,
    status_msg: String,
    font: Arc<FontVec>,
}

impl IconMarkerApp {
    fn new() -> Self {
        Self {
            text: "G".to_string(),
            bg_color: Color32::from_rgb(242, 220, 198),
            grad_start: Color32::from_rgb(120, 90, 220),
            grad_end: Color32::from_rgb(20, 170, 130),
            padding: 0.16,
            chevron_on: true,
            texture: None,
            needs_update: true,
            status_msg: String::new(),
            font: Arc::new(load_font()),
        }
    }

    /// Measure the actual pixel bounding box of text rendered at a given scale.
    /// Returns (min_x, min_y, max_x, max_y) of non-transparent pixels, or None.
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

    fn generate_image(&self, size: u32) -> RgbaImage {
        let bg = Rgba([self.bg_color.r(), self.bg_color.g(), self.bg_color.b(), 255]);
        let mut img = RgbaImage::from_pixel(size, size, bg);

        if self.chevron_on {
            draw_chevron_pattern(&mut img, self.bg_color, size);
        }

        if self.text.is_empty() {
            return img;
        }

        let font = &*self.font;

        // Step 1: Render at reference scale and measure actual glyph bounding box
        let ref_scale = 200.0_f32;
        let ref_canvas = 512u32;
        let bbox = match Self::measure_text_bbox(font, &self.text, ref_scale, ref_canvas) {
            Some(b) => b,
            None => return img,
        };
        let ref_w = (bbox.2 - bbox.0 + 1) as f32;
        let ref_h = (bbox.3 - bbox.1 + 1) as f32;

        // Step 2: Calculate scale to fill canvas with padding
        let target = size as f32 * (1.0 - self.padding * 2.0);
        let ratio = (target / ref_w).min(target / ref_h);
        let final_scale = ref_scale * ratio;

        // Step 3: Render at final scale on an oversized canvas to avoid clipping
        let white = Rgba([255u8, 255, 255, 255]);
        let tmp_size = size * 2;
        let mut text_layer = RgbaImage::from_pixel(tmp_size, tmp_size, Rgba([0, 0, 0, 0]));
        draw_text_mut(&mut text_layer, white, 0, 0, PxScale::from(final_scale), font, &self.text);

        // Find actual bounds at final scale
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

        // Step 4: Calculate offset to center the actual glyph pixels
        let glyph_w = max_x - min_x + 1;
        let glyph_h = max_y - min_y + 1;
        let offset_x = (size - glyph_w) as i32 / 2 - min_x as i32;
        let offset_y = (size - glyph_h) as i32 / 2 - min_y as i32;

        // Step 5: Composite text with gradient onto background, applying offset
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
                    let grad = lerp_color(self.grad_start, self.grad_end, t);
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

    fn save_png(&mut self) {
        if let Some(path) = FileDialog::new()
            .set_title("Save PNG")
            .add_filter("PNG", &["png"])
            .set_file_name("icon.png")
            .save_file()
        {
            let path = ensure_extension(path, "png");
            let img = self.generate_image(256);
            match img.save(&path) {
                Ok(_) => self.status_msg = format!("PNG saved: {}", path.display()),
                Err(e) => self.status_msg = format!("Error: {e}"),
            }
        }
    }

    fn save_ico(&mut self) {
        if let Some(path) = FileDialog::new()
            .set_title("Save ICO")
            .add_filter("ICO", &["ico"])
            .set_file_name("icon.ico")
            .save_file()
        {
            let path = ensure_extension(path, "ico");
            let base_img = self.generate_image(256);
            let sizes: &[u32] = &[16, 32, 48, 256];
            let mut icon_dir = IconDir::new(ResourceType::Icon);

            for &s in sizes {
                let resized = if s == 256 {
                    base_img.clone()
                } else {
                    image::imageops::resize(&base_img, s, s, FilterType::Lanczos3)
                };
                let icon_image = IconImage::from_rgba_data(s, s, resized.into_raw());
                let entry_result = if s >= 256 {
                    IconDirEntry::encode_as_png(&icon_image)
                } else {
                    IconDirEntry::encode_as_bmp(&icon_image)
                };
                match entry_result {
                    Ok(entry) => icon_dir.add_entry(entry),
                    Err(e) => {
                        self.status_msg = format!("Error encoding {s}x{s}: {e}");
                        return;
                    }
                }
            }

            match File::create(&path) {
                Ok(file) => match icon_dir.write(file) {
                    Ok(_) => self.status_msg = format!("ICO saved: {}", path.display()),
                    Err(e) => self.status_msg = format!("Error writing ICO: {e}"),
                },
                Err(e) => self.status_msg = format!("Error creating file: {e}"),
            }
        }
    }

    fn update_preview(&mut self, ctx: &egui::Context) {
        let img = self.generate_image(256);
        let color_image = ColorImage::from_rgba_unmultiplied([256, 256], img.as_raw());

        match &mut self.texture {
            Some(tex) => tex.set(color_image, TextureOptions::NEAREST),
            None => {
                self.texture = Some(ctx.load_texture(
                    "icon-preview",
                    color_image,
                    TextureOptions::NEAREST,
                ));
            }
        }
        self.needs_update = false;
    }
}

impl eframe::App for IconMarkerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("IconMarker");
            ui.separator();

            ui.horizontal(|ui| {
                // Left: controls
                ui.vertical(|ui| {
                    ui.set_width(300.0);

                    ui.label("Text:");
                    if ui.text_edit_singleline(&mut self.text).changed() {
                        self.needs_update = true;
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Background:");
                        if color_edit_button_srgba(ui, &mut self.bg_color, Alpha::Opaque).changed()
                        {
                            self.needs_update = true;
                        }
                    });

                    ui.add_space(4.0);
                    if ui.checkbox(&mut self.chevron_on, "Chevron pattern").changed() {
                        self.needs_update = true;
                    }

                    ui.add_space(8.0);
                    ui.label("Text gradient:");
                    ui.horizontal(|ui| {
                        ui.label("  Start:");
                        if color_edit_button_srgba(ui, &mut self.grad_start, Alpha::Opaque)
                            .changed()
                        {
                            self.needs_update = true;
                        }
                        ui.label("  End:");
                        if color_edit_button_srgba(ui, &mut self.grad_end, Alpha::Opaque).changed()
                        {
                            self.needs_update = true;
                        }
                    });

                    ui.add_space(8.0);
                    ui.label("Padding:");
                    if ui
                        .add(egui::Slider::new(&mut self.padding, 0.0..=0.4).fixed_decimals(2))
                        .changed()
                    {
                        self.needs_update = true;
                    }

                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save PNG").clicked() {
                            self.save_png();
                        }
                        if ui.button("Save ICO").clicked() {
                            self.save_ico();
                        }
                    });

                    if !self.status_msg.is_empty() {
                        ui.add_space(8.0);
                        ui.label(&self.status_msg);
                    }
                });

                ui.separator();

                // Right: preview
                ui.vertical(|ui| {
                    ui.label("Preview (256x256):");
                    if let Some(tex) = &self.texture {
                        ui.image((tex.id(), egui::vec2(256.0, 256.0)));
                    }
                });
            });
        });

        if self.needs_update {
            self.update_preview(ctx);
        }
    }
}

fn ensure_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        path
    } else {
        path.with_extension(ext)
    }
}
