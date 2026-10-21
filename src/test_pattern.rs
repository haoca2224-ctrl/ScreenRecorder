//! Deterministic moving target for end-to-end capture validation.
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::time::Instant;

struct Pattern {
    start: Instant,
}
impl eframe::App for Pattern {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let t = self.start.elapsed().as_secs_f32();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Screen Recorder · Capture test");
            ui.label("Moving bars, elapsed clock and readable text");
            ui.label(
                egui::RichText::new(format!("{t:08.2} s"))
                    .size(48.0)
                    .monospace(),
            );
            let (r, _) = ui
                .allocate_exact_size(Vec2::new(ui.available_width(), 380.0), egui::Sense::hover());
            let p = ui.painter();
            let colors = [
                Color32::RED,
                Color32::GREEN,
                Color32::BLUE,
                Color32::YELLOW,
                Color32::WHITE,
                Color32::from_rgb(20, 30, 45),
            ];
            for (i, c) in colors.into_iter().enumerate() {
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(r.left() + r.width() * i as f32 / 6.0, r.top()),
                        Vec2::new(r.width() / 6.0, r.height()),
                    ),
                    0.0,
                    c,
                );
            }
            let x = r.left() + ((t * 180.0) % r.width());
            p.circle_filled(Pos2::new(x, r.center().y), 30.0, Color32::BLACK);
        });
        ctx.request_repaint();
    }
}
pub fn run() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 600.0])
            .with_position([80.0, 80.0])
            .with_title("Recorder Test Pattern"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Recorder Test Pattern",
        options,
        Box::new(|_| {
            Ok(Box::new(Pattern {
                start: Instant::now(),
            }))
        }),
    );
}
