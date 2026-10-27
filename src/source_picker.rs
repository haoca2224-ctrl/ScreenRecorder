//! Source discovery and bounded thumbnails run independently of egui paint.
use crate::{audio, capture, model::Source, native};
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

pub struct Inventory {
    pub displays: Vec<native::Display>,
    pub windows: Vec<native::AppWindow>,
    pub speakers: Vec<audio::Device>,
    pub microphones: Vec<audio::Device>,
}
pub struct Sources {
    pub inventory: mpsc::Receiver<Inventory>,
    stop: Arc<AtomicBool>,
    jobs: mpsc::SyncSender<(String, Source)>,
    results: mpsc::Receiver<(String, Option<egui::ColorImage>)>,
    pending: HashSet<String>,
    cache: HashMap<String, (Instant, Option<egui::TextureHandle>)>,
}
impl Sources {
    pub fn new() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (tx, inventory) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                let _ = tx.try_send(Inventory {
                    displays: native::displays(),
                    windows: native::app_windows(),
                    speakers: audio::devices(false),
                    microphones: audio::devices(true),
                });
                for _ in 0..20 {
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        let (jobs, rx) = mpsc::sync_channel::<(String, Source)>(8);
        let (done, results) = mpsc::channel();
        let flag = stop.clone();
        std::thread::spawn(move || {
            while let Ok((key, source)) = rx.recv() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let image = thumbnail(&source);
                if done.send((key, image)).is_err() {
                    break;
                }
            }
        });
        Self {
            inventory,
            stop,
            jobs,
            results,
            pending: HashSet::new(),
            cache: HashMap::new(),
        }
    }
    pub fn tick(&mut self, ctx: &egui::Context) {
        for (key, image) in self.results.try_iter() {
            self.pending.remove(&key);
            if self.cache.len() >= 64 {
                self.cache.clear();
            }
            self.cache.insert(
                key.clone(),
                (
                    Instant::now(),
                    image.map(|i| ctx.load_texture(key, i, egui::TextureOptions::LINEAR)),
                ),
            );
        }
    }
    pub fn texture(&mut self, source: Source) -> Option<egui::TextureHandle> {
        let key = format!("{source:?}");
        let stale = self
            .cache
            .get(&key)
            .is_none_or(|(t, _)| t.elapsed() > Duration::from_secs(8));
        if stale
            && !self.pending.contains(&key)
            && self.jobs.try_send((key.clone(), source)).is_ok()
        {
            self.pending.insert(key.clone());
        }
        self.cache.get(&key).and_then(|(_, t)| t.clone())
    }
}
impl Drop for Sources {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn thumbnail(source: &Source) -> Option<egui::ColorImage> {
    let session =
        capture::ScreenSession::start_with_timeout(source, false, 2, Duration::from_millis(1200))
            .ok()?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(900) {
        let image = session.latest.lock().ok()?.image.clone();
        if let Some(image) = image {
            return Some(resize(&image));
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    None
}
pub fn resize(image: &capture::Image) -> egui::ColorImage {
    // Sample directly into the bounded preview; never copy a full 4K RGBA image on the UI thread.
    let factor = (320.0 / image.width as f32)
        .min(180.0 / image.height as f32)
        .min(1.0);
    let w = (image.width as f32 * factor).max(1.0) as usize;
    let h = (image.height as f32 * factor).max(1.0) as usize;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * image.height as usize / h) * image.width as usize
                + x * image.width as usize / w)
                * 4;
            let p = &image.pixels[i..i + 4];
            rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba)
}

#[cfg(test)]
mod tests {
    #[test]
    fn thumbnails_are_bounded_and_keep_bgra_color() {
        let image = crate::capture::Image::from_bgra(640, 360, [3, 20, 240, 255].repeat(640 * 360));
        let small = super::resize(&image);
        assert_eq!(small.size, [320, 180]);
        assert_eq!(small.pixels[0].to_array(), [240, 20, 3, 255]);
    }
}
