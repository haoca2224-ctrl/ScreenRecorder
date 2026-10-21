use crate::model::Source;
use anyhow::{bail, Result};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::*,
    window::Window,
};

#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<PooledPixels>,
}

impl Image {
    pub fn from_bgra(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels: Arc::new(PooledPixels::unpooled(pixels)),
        }
    }
}

pub struct PooledPixels {
    bytes: Option<Vec<u8>>,
    pool: Option<Arc<BufferPool>>,
}

impl PooledPixels {
    fn pooled(bytes: Vec<u8>, pool: Arc<BufferPool>) -> Self {
        Self {
            bytes: Some(bytes),
            pool: Some(pool),
        }
    }

    fn unpooled(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Some(bytes),
            pool: None,
        }
    }
}

impl Deref for PooledPixels {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.bytes.as_deref().unwrap_or_default()
    }
}

impl Drop for PooledPixels {
    fn drop(&mut self) {
        if let (Some(pool), Some(bytes)) = (&self.pool, self.bytes.take()) {
            pool.give_back(bytes);
        }
    }
}

#[derive(Default)]
struct BufferPool {
    free: Mutex<Vec<Vec<u8>>>,
}

impl BufferPool {
    fn take(&self, len: usize) -> Vec<u8> {
        let mut free = self.free.lock().unwrap();
        let mut bytes = free
            .iter()
            .position(|buffer| buffer.capacity() >= len)
            .map(|index| free.swap_remove(index))
            .unwrap_or_else(|| Vec::with_capacity(len));
        drop(free);
        bytes.resize(len, 0);
        bytes
    }

    fn give_back(&self, mut bytes: Vec<u8>) {
        bytes.clear();
        let mut free = self.free.lock().unwrap();
        if free.len() < 3 {
            free.push(bytes);
        }
    }
}
#[derive(Default)]
pub struct Latest {
    pub image: Option<Image>,
    pub error: Option<String>,
}
#[derive(Clone)]
pub struct Flags {
    latest: Arc<Mutex<Latest>>,
    pool: Arc<BufferPool>,
    crop: Option<(u32, u32, u32, u32)>,
    interval: Duration,
}
pub struct Handler {
    flags: Flags,
    last: Option<Instant>,
}
type CaptureError = Box<dyn std::error::Error + Send + Sync>;

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Flags;
    type Error = CaptureError;
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            flags: ctx.flags,
            last: None,
        })
    }
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.last.is_some_and(|t| t.elapsed() < self.flags.interval) {
            return Ok(());
        }
        self.last = Some(Instant::now());
        let result = (|| -> Result<Image, CaptureError> {
            let (x, y, w, h) =
                self.flags
                    .crop
                    .unwrap_or((0, 0, frame.width() & !1, frame.height() & !1));
            if w < 16 || h < 16 || x + w > frame.width() || y + h > frame.height() {
                return Err("Kích thước nguồn quay đã thay đổi. Dừng và chọn lại vùng.".into());
            }
            let mut buffer = frame.buffer_crop(x, y, x + w, y + h)?;
            let source = buffer.as_nopadding_buffer()?;
            let mut pixels = self.flags.pool.take(source.len());
            pixels.copy_from_slice(source);
            let pixels = Arc::new(PooledPixels::pooled(pixels, self.flags.pool.clone()));
            Ok(Image {
                width: w,
                height: h,
                pixels,
            })
        })();
        let mut latest = self.flags.latest.lock().unwrap();
        match result {
            Ok(image) => {
                if latest
                    .image
                    .as_ref()
                    .is_some_and(|old| old.width != image.width || old.height != image.height)
                {
                    latest.error = Some("Cửa sổ đã đổi kích thước. Bản ghi được giữ lại; hãy chọn nguồn và ghi tiếp một bản mới.".into());
                    control.stop();
                } else {
                    latest.image = Some(image);
                }
            }
            Err(e) => {
                latest.error = Some(e.to_string());
                control.stop();
            }
        }
        Ok(())
    }
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.flags.latest.lock().unwrap().error = Some("Nguồn ghi hình đã đóng.".into());
        Ok(())
    }
}

pub struct ScreenSession {
    pub latest: Arc<Mutex<Latest>>,
    control: Option<CaptureControl<Handler, CaptureError>>,
}
impl ScreenSession {
    pub fn start(source: &Source, cursor: bool, fps: u32) -> Result<Self> {
        let latest = Arc::new(Mutex::new(Latest::default()));
        let mut flags = Flags {
            latest: latest.clone(),
            pool: Arc::new(BufferPool::default()),
            crop: None,
            interval: Duration::from_secs_f64(0.8 / fps as f64),
        };
        let cursor = if cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        };
        let control = match *source {
            Source::Monitor {
                handle,
                bounds,
                region,
            } => {
                if !region.fits(bounds) {
                    bail!("Vùng chọn nằm ngoài màn hình hoặc nhỏ hơn 16 × 16.");
                }
                flags.crop = Some((
                    (region.x - bounds.x) as u32,
                    (region.y - bounds.y) as u32,
                    region.width & !1,
                    region.height & !1,
                ));
                Handler::start_free_threaded(Settings::new(
                    Monitor::from_raw_hmonitor(handle as *mut _),
                    cursor,
                    DrawBorderSettings::Default,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    flags,
                ))
            }
            Source::Window { handle } => Handler::start_free_threaded(Settings::new(
                Window::from_raw_hwnd(handle as *mut _),
                cursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                flags,
            )),
        }
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        let session = Self {
            latest,
            control: Some(control),
        };
        let deadline = Instant::now() + Duration::from_secs(8);
        while session.latest.lock().unwrap().image.is_none() {
            if let Some(e) = session.latest.lock().unwrap().error.clone() {
                bail!(e);
            }
            if Instant::now() >= deadline {
                bail!("Không nhận được hình. Hãy mở cửa sổ nguồn, bỏ thu nhỏ và thử lại.");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(session)
    }
}
impl Drop for ScreenSession {
    fn drop(&mut self) {
        if let Some(c) = self.control.take() {
            let _ = c.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_storage_returns_to_the_bounded_pool() {
        let pool = Arc::new(BufferPool::default());
        let mut bytes = pool.take(1920 * 1080 * 4);
        let pointer = bytes.as_ptr();
        bytes[0] = 42;
        drop(Arc::new(PooledPixels::pooled(bytes, pool.clone())));

        let reused = pool.take(1920 * 1080 * 4);
        assert_eq!(reused.as_ptr(), pointer);
        assert_eq!(reused.len(), 1920 * 1080 * 4);
    }

    #[test]
    fn pool_keeps_at_most_three_frames() {
        let pool = Arc::new(BufferPool::default());
        for _ in 0..5 {
            pool.give_back(Vec::with_capacity(16));
        }
        assert_eq!(pool.free.lock().unwrap().len(), 3);
    }
}
