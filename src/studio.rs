//! Shared annotation model. The excluded desktop overlay and encoder use this
//! same transparent raster, so controls can never be burned into the export.
use crate::{
    capture::Image,
    model::{Area, Language},
};
use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use std::sync::{Arc, Mutex};

pub type Shared = Arc<Mutex<Live>>;
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Pointer,
    Pen,
    Highlight,
    Arrow,
    Rectangle,
    Ellipse,
    Text,
    Number,
}
impl Tool {
    pub const ALL: [(Self, &'static str); 8] = [
        (Self::Pointer, "Thao tác"),
        (Self::Pen, "Bút"),
        (Self::Highlight, "Đánh dấu"),
        (Self::Arrow, "Mũi tên"),
        (Self::Rectangle, "Khung"),
        (Self::Ellipse, "Tròn"),
        (Self::Text, "Chữ"),
        (Self::Number, "Số"),
    ];
}
#[derive(Clone)]
pub struct Mark {
    pub tool: Tool,
    pub points: Vec<(f32, f32)>,
    pub color: [u8; 3],
    pub width: f32,
    pub text: String,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Ready,
    Countdown,
    Starting,
    Recording,
    Paused,
    Busy,
}
#[derive(Clone, Copy)]
pub enum Action {
    StartStop,
    Pause,
    Screenshot,
    ShowMain,
    Dismiss,
}
pub struct Live {
    pub language: Language,
    pub area: Area,
    pub allow_resize: bool,
    pub status: Status,
    pub seconds: f64,
    pub countdown: u64,
    pub bytes: u64,
    pub tool: Tool,
    pub color: [u8; 3],
    pub width: f32,
    pub text: String,
    pub marks: Vec<Mark>,
    pub redo: Vec<Mark>,
    pub draft: Option<Mark>,
    pub revision: u64,
    pub raster: Arc<Vec<u8>>,
    raster_revision: u64,
    raster_size: (u32, u32),
    effect_scratch: Vec<u8>,
    pub system_on: bool,
    pub mic_on: bool,
    pub system_gain: f32,
    pub mic_gain: f32,
    pub levels: [f32; 2],
    pub audio_available: [bool; 2],
    pub cursor_highlight: bool,
    pub click_effect: bool,
    pub pointer: Option<(f32, f32)>,
    pub click_at: Option<((f32, f32), std::time::Instant)>,
    pub latest: Option<Image>,
    /// When true, Desktop Duplication records this annotation surface directly.
    /// The toolbar stays excluded while the frame window becomes capturable.
    pub desktop_capture: bool,
    pub fault: Option<String>,
}
impl Default for Live {
    fn default() -> Self {
        Self {
            language: Language::Vi,
            area: Area::default(),
            allow_resize: true,
            status: Status::Ready,
            seconds: 0.0,
            countdown: 3,
            bytes: 0,
            tool: Tool::Pointer,
            color: [255, 193, 64],
            width: 4.0,
            text: "Ghi chú".into(),
            marks: vec![],
            redo: vec![],
            draft: None,
            revision: 1,
            raster: Arc::new(vec![]),
            raster_revision: 0,
            raster_size: (0, 0),
            effect_scratch: Vec::new(),
            system_on: true,
            mic_on: false,
            system_gain: 1.0,
            mic_gain: 1.0,
            levels: [0.0; 2],
            audio_available: [true; 2],
            cursor_highlight: false,
            click_effect: false,
            pointer: None,
            click_at: None,
            latest: None,
            desktop_capture: false,
            fault: None,
        }
    }
}
impl Live {
    pub fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn undo(&mut self) {
        if let Some(m) = self.marks.pop() {
            self.redo.push(m);
            self.changed();
        }
    }
    pub fn redo(&mut self) {
        if let Some(m) = self.redo.pop() {
            self.marks.push(m);
            self.changed();
        }
    }
    pub fn clear(&mut self) {
        self.marks.clear();
        self.redo.clear();
        self.draft = None;
        self.changed();
    }
    pub fn finish_mark(&mut self) {
        if let Some(m) = self.draft.take() {
            self.marks.push(m);
            self.redo.clear();
            self.changed();
        }
    }
    pub fn raster(&mut self, w: u32, h: u32) -> Arc<Vec<u8>> {
        if self.raster_revision != self.revision || self.raster_size != (w, h) {
            let mut canvas = Canvas::new(w, h);
            for mark in self.marks.iter().chain(self.draft.iter()) {
                canvas.mark(mark);
            }
            self.raster = Arc::new(canvas.pixels);
            self.raster_revision = self.revision;
            self.raster_size = (w, h);
        }
        self.raster.clone()
    }
}

pub struct Canvas {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            pixels: vec![0; w as usize * h as usize * 4],
            width: w,
            height: h,
        }
    }
    fn dot(&mut self, x: i32, y: i32, color: [u8; 3], alpha: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        // A mark paints its own coverage; avoid accumulating highlighter opacity
        // at every sample along a single stroke.
        if alpha >= self.pixels[i + 3] {
            self.pixels[i..i + 4].copy_from_slice(&[color[2], color[1], color[0], alpha]);
        }
    }
    pub fn disc(&mut self, p: (f32, f32), r: f32, color: [u8; 3], alpha: u8) {
        for y in
            ((p.1 - r - 1.0) as i32).max(0)..=((p.1 + r + 1.0) as i32).min(self.height as i32 - 1)
        {
            for x in ((p.0 - r - 1.0) as i32).max(0)
                ..=((p.0 + r + 1.0) as i32).min(self.width as i32 - 1)
            {
                let coverage = (r + 0.5
                    - ((x as f32 - p.0).powi(2) + (y as f32 - p.1).powi(2)).sqrt())
                .clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.dot(x, y, color, (alpha as f32 * coverage) as u8);
                }
            }
        }
    }
    fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: [u8; 3], alpha: u8) {
        let n = ((b.0 - a.0).hypot(b.1 - a.1).ceil() as usize).max(1);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.disc(
                (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t),
                width / 2.0,
                color,
                alpha,
            );
        }
    }
    fn text(&mut self, text: &str, p: (f32, f32), size: f32, color: [u8; 3]) {
        static FONT: std::sync::OnceLock<Option<FontArc>> = std::sync::OnceLock::new();
        let Some(font) = FONT.get_or_init(|| {
            std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf")
                .ok()
                .and_then(|b| FontArc::try_from_vec(b).ok())
        }) else {
            return;
        };
        let scaled = font.as_scaled(PxScale::from(size));
        let mut x = p.0;
        for ch in text.chars() {
            let id = font.glyph_id(ch);
            let glyph = id.with_scale_and_position(size, point(x, p.1 + scaled.ascent()));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, v| {
                    self.dot(
                        gx as i32 + bounds.min.x as i32,
                        gy as i32 + bounds.min.y as i32,
                        color,
                        (v * 255.0) as u8,
                    )
                });
            }
            x += scaled.h_advance(id);
        }
    }
    pub fn mark(&mut self, m: &Mark) {
        let Some(&a) = m.points.first() else { return };
        let b = *m.points.last().unwrap();
        let (w, c) = (m.width, m.color);
        match m.tool {
            Tool::Pointer => {}
            Tool::Pen | Tool::Highlight => {
                let (width, alpha) = if m.tool == Tool::Highlight {
                    (w * 5.0, 80)
                } else {
                    (w, 255)
                };
                self.disc(a, width / 2.0, c, alpha);
                for pair in m.points.windows(2) {
                    self.line(pair[0], pair[1], width, c, alpha);
                }
            }
            Tool::Arrow => {
                self.line(a, b, w, c, 255);
                let ang = (b.1 - a.1).atan2(b.0 - a.0);
                let len = (w * 4.0).max(16.0);
                for d in [-0.55, 0.55] {
                    self.line(
                        b,
                        (b.0 - len * (ang + d).cos(), b.1 - len * (ang + d).sin()),
                        w,
                        c,
                        255,
                    );
                }
            }
            Tool::Rectangle => {
                let p = [a, (b.0, a.1), b, (a.0, b.1), a];
                for pair in p.windows(2) {
                    self.line(pair[0], pair[1], w, c, 255);
                }
            }
            Tool::Ellipse => {
                let center = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let r = ((a.0 - b.0).abs() / 2.0, (a.1 - b.1).abs() / 2.0);
                let n = ((r.0 + r.1) * 3.2).ceil().max(24.0) as usize;
                let mut prev = (center.0 + r.0, center.1);
                for i in 1..=n {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    let p = (center.0 + r.0 * t.cos(), center.1 + r.1 * t.sin());
                    self.line(prev, p, w, c, 255);
                    prev = p;
                }
            }
            Tool::Text => self.text(&m.text, a, (w * 5.0).max(20.0), c),
            Tool::Number => {
                self.disc(a, 17.0, c, 255);
                self.text(&m.text, (a.0 - 7.0, a.1 - 14.0), 22.0, [20, 25, 33]);
            }
        }
    }
}
pub fn blend(dst: &mut [u8], overlay: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(overlay.chunks_exact(4)) {
        let a = s[3] as u32;
        if a == 0 {
            continue;
        }
        let da = d[3] as u32;
        let out = a * 255 + da * (255 - a);
        if out > 0 {
            for j in 0..3 {
                d[j] =
                    ((s[j] as u32 * a * 255 + d[j] as u32 * da * (255 - a) + out / 2) / out) as u8;
            }
        }
        d[3] = ((out + 127) / 255) as u8;
    }
}
pub fn composite(image: &Image, shared: &Shared) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(image.pixels.len());
    composite_into(image, shared, &mut pixels);
    pixels
}

pub fn composite_into(image: &Image, shared: &Shared, pixels: &mut Vec<u8>) {
    let mut live = shared.lock().unwrap();
    let raster = live.raster(image.width, image.height);
    pixels.clear();
    pixels.extend_from_slice(&image.pixels);
    blend(pixels, &raster);
    effects_reuse(pixels, image.width, image.height, &mut live);
}
pub fn effects_reuse(pixels: &mut [u8], w: u32, h: u32, live: &mut Live) {
    if !live.cursor_highlight && !live.click_effect {
        return;
    }
    let len = w as usize * h as usize * 4;
    live.effect_scratch.clear();
    live.effect_scratch.resize(len, 0);
    let mut c = Canvas {
        pixels: std::mem::take(&mut live.effect_scratch),
        width: w,
        height: h,
    };
    if live.cursor_highlight {
        if let Some(p) = live.pointer {
            c.disc(p, 18.0, [255, 210, 60], 65);
        }
    }
    if live.click_effect {
        if let Some((p, t)) = live.click_at {
            let age = t.elapsed().as_secs_f32();
            if age < 0.55 {
                let r = 10.0 + age * 45.0;
                let alpha = ((1.0 - age / 0.55) * 220.0) as u8;
                let mark = Mark {
                    tool: Tool::Ellipse,
                    points: vec![(p.0 - r, p.1 - r), (p.0 + r, p.1 + r)],
                    color: [90, 190, 255],
                    width: 3.0,
                    text: String::new(),
                };
                c.mark(&mark);
                for px in c.pixels.chunks_exact_mut(4) {
                    px[3] = (px[3] as u16 * alpha as u16 / 255) as u8;
                }
            }
        }
    }
    blend(pixels, &c.pixels);
    live.effect_scratch = c.pixels;
}
pub fn save_png(image: &Image, path: &std::path::Path) -> anyhow::Result<()> {
    let mut rgba = image.pixels.to_vec();
    for p in rgba.chunks_exact_mut(4) {
        p.swap(0, 2);
        p[3] = 255;
    }
    image::save_buffer_with_format(
        path,
        &rgba,
        image.width,
        image.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn annotations_blend_but_controls_are_not_in_raster() {
        let mut l = Live::default();
        l.marks.push(Mark {
            tool: Tool::Rectangle,
            points: vec![(10., 10.), (30., 30.)],
            color: [255, 0, 0],
            width: 3.,
            text: String::new(),
        });
        let raster = l.raster(64, 64);
        let mut dst = vec![255; 64 * 64 * 4];
        blend(&mut dst, &raster);
        assert_eq!(
            &dst[(10 * 64 + 20) * 4..(10 * 64 + 20) * 4 + 3],
            &[0, 0, 255]
        );
        assert_eq!(&dst[0..4], &[255; 4]);
        assert_eq!(&dst[(20 * 64 + 20) * 4..(20 * 64 + 20) * 4 + 4], &[255; 4]);
    }
    #[test]
    fn undo_and_resize_invalidate_cache() {
        let mut l = Live {
            draft: Some(Mark {
                tool: Tool::Pen,
                points: vec![(8., 8.)],
                color: [255, 0, 0],
                width: 4.,
                text: String::new(),
            }),
            ..Default::default()
        };
        l.finish_mark();
        assert!(l.raster(32, 32).iter().any(|b| *b != 0));
        l.undo();
        assert!(l.raster(32, 32).iter().all(|b| *b == 0));
        l.redo();
        assert_eq!(l.raster(64, 32).len(), 64 * 32 * 4);
    }
}
