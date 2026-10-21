use crate::{
    audio, editor,
    model::*,
    native, overlay, recorder,
    studio::{self, Status},
};
use eframe::egui::{self, Color32, FontId, RichText, Vec2};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::{
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

const INK: Color32 = Color32::from_rgb(234, 239, 247);
const MUTED: Color32 = Color32::from_rgb(157, 174, 195);
const PANEL: Color32 = Color32::from_rgb(23, 32, 46);
const ACCENT: Color32 = Color32::from_rgb(111, 198, 237);
const RED: Color32 = Color32::from_rgb(218, 62, 78);

#[derive(PartialEq)]
enum Phase {
    Ready,
    Countdown,
    Starting,
    Recording,
    Pausing,
    Paused,
    Finishing,
}

pub struct App {
    prefs: Preferences,
    monitors: Vec<native::Display>,
    windows: Vec<native::AppWindow>,
    monitor: usize,
    window: usize,
    source_filter: String,
    mode: u8,
    region: Option<Area>,
    picker: Option<Receiver<anyhow::Result<Option<Area>>>>,
    controller: Option<recorder::Controller>,
    hotkeys: Receiver<Result<u32, String>>,
    phase: Phase,
    countdown: Option<Instant>,
    error: Option<String>,
    last_file: Option<PathBuf>,
    seconds: f64,
    bytes: u64,
    encoder: String,
    close_requested: bool,
    overlay: Option<overlay::Overlay>,
    live: studio::Shared,
    tab: u8,
    preview_stop: Option<Arc<AtomicBool>>,
    preview_key: String,
    preview_candidate: (String, Instant),
    preview_texture: Option<egui::TextureHandle>,
    preview_updated: Instant,
    screenshot_result: Option<Receiver<Result<PathBuf, String>>>,
    notice: Option<String>,
    system_devices: Vec<audio::Device>,
    mic_devices: Vec<audio::Device>,
    meter: Option<audio::Meter>,
    settings_path: PathBuf,
    editor: editor::EditorState,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_theme(egui::Theme::Dark);
        ctx.enable_accesskit();
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe".into(), egui::FontData::from_owned(data).into());
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "segoe".into());
        }
        ctx.set_fonts(fonts);
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(13, 19, 30);
        style.visuals.window_fill = PANEL;
        style.visuals.override_text_color = Some(INK);
        style.visuals.selection.bg_fill = Color32::from_rgb(40, 86, 110);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 49, 68);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(47, 68, 91);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.spacing.interact_size.y = 30.0;
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.bg_stroke = egui::Stroke::NONE;
        }
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(33, 44, 59);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(45, 65, 85);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(13.0));
        ctx.set_style(style);
        let settings_path = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("ScreenRecorder")
            .join("settings.json");
        let mut prefs: Preferences = std::fs::read(&settings_path)
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default();
        prefs.fps = 30;
        prefs.bitrate = prefs.bitrate.clamp(2, 20);
        prefs.system_volume = prefs.system_volume.clamp(0.0, 2.0);
        prefs.mic_volume = prefs.mic_volume.clamp(0.0, 2.0);
        let live = Arc::new(Mutex::new(studio::Live::default()));
        let meter = Some(audio::meters(prefs.clone(), live.clone()));
        let mut app = Self {
            prefs,
            monitors: native::displays(),
            windows: native::app_windows(),
            monitor: 0,
            window: 0,
            source_filter: String::new(),
            mode: 0,
            region: None,
            picker: None,
            controller: None,
            hotkeys: native::hotkeys(),
            phase: Phase::Ready,
            countdown: None,
            error: None,
            last_file: None,
            seconds: 0.0,
            bytes: 0,
            encoder: String::new(),
            close_requested: false,
            overlay: None,
            live,
            tab: 0,
            preview_stop: None,
            preview_key: String::new(),
            preview_candidate: (String::new(), Instant::now()),
            preview_texture: None,
            preview_updated: Instant::now(),
            screenshot_result: None,
            notice: None,
            system_devices: audio::devices(false),
            mic_devices: audio::devices(true),
            meter,
            settings_path,
            editor: editor::EditorState::default(),
        };
        let args = std::env::args().collect::<Vec<_>>();
        if let Some(index) = args.iter().position(|value| value == "--open-editor") {
            if let Some(path) = args
                .get(index + 1)
                .map(PathBuf::from)
                .filter(|path| path.exists())
            {
                app.editor.open(path);
                app.tab = 2;
            }
        }
        app
    }
    fn source(&self) -> anyhow::Result<Source> {
        if self.mode == 2 {
            let w = self.windows.get(self.window).ok_or_else(|| {
                anyhow::anyhow!(self.prefs.language.text(
                    "Hãy chọn cửa sổ muốn quay.",
                    "Choose an application window to record."
                ))
            })?;
            return Ok(Source::Window { handle: w.handle });
        }
        let m = self.monitors.get(self.monitor).ok_or_else(|| {
            anyhow::anyhow!(self
                .prefs
                .language
                .text("Không tìm thấy màn hình.", "No display was found."))
        })?;
        let region = if self.mode == 0 {
            self.region.ok_or_else(|| {
                anyhow::anyhow!(self.prefs.language.text(
                    "Hãy kéo chọn vùng trước khi bắt đầu.",
                    "Select a region before recording."
                ))
            })?
        } else {
            m.area
        };
        if !region.fits(m.area) {
            anyhow::bail!(self.prefs.language.text(
                "Vùng chọn phải nằm trong màn hình, tối thiểu 16 × 16 pixel.",
                "The region must stay inside one display and be at least 16 × 16 pixels."
            ));
        }
        Ok(Source::Monitor {
            handle: m.handle,
            bounds: m.area,
            region,
        })
    }
    fn save_settings(&self) {
        if let Some(parent) = self.settings_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_vec_pretty(&self.prefs) {
            let _ = std::fs::write(&self.settings_path, data);
        }
    }
    fn begin(&mut self) {
        if self.prefs.microphone && self.mic_devices.is_empty() {
            self.error = Some(
                self.prefs
                    .language
                    .text(
                        "Không tìm thấy micro. Hãy kết nối thiết bị hoặc tắt Micro.",
                        "No microphone was found. Connect one or disable Microphone.",
                    )
                    .into(),
            );
            return;
        }
        if self.prefs.system_audio && self.system_devices.is_empty() {
            self.error = Some(
                self.prefs
                    .language
                    .text(
                        "Không tìm thấy thiết bị phát âm thanh. Hãy kiểm tra loa/tai nghe.",
                        "No playback device was found. Check your speakers or headphones.",
                    )
                    .into(),
            );
            return;
        }
        match self.source() {
            Err(e) => self.error = Some(e.to_string()),
            Ok(_) => {
                if self.prefs.output.as_os_str().is_empty() {
                    self.error = Some(
                        self.prefs
                            .language
                            .text("Hãy chọn thư mục lưu.", "Choose an output folder.")
                            .into(),
                    );
                    return;
                }
                self.error = None;
                self.show_overlay();
                self.seconds = 0.0;
                self.bytes = 0;
                self.phase = Phase::Countdown;
                self.countdown = Some(Instant::now());
                self.save_settings();
            }
        }
    }
    fn stop(&mut self) {
        if self.phase == Phase::Countdown {
            self.phase = Phase::Ready;
            self.countdown = None;
            return;
        }
        if let Some(c) = &self.controller {
            let _ = c.commands.send(Command::Stop);
            self.phase = Phase::Finishing;
        }
    }
    fn pause_resume(&mut self, ctx: &egui::Context) {
        if let Some(c) = &self.controller {
            if self.phase == Phase::Recording {
                let _ = c.commands.send(Command::Pause);
                self.phase = Phase::Pausing;
            } else if self.phase == Phase::Paused {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                let _ = c.commands.send(Command::Resume);
                self.phase = Phase::Starting;
            }
        }
    }
    fn tick(&mut self, ctx: &egui::Context) {
        native::exclude_main_window(true);
        self.studio_tick(ctx);
        while let Ok(key) = self.hotkeys.try_recv() {
            match key {
                Ok(1) if self.phase == Phase::Ready => self.begin(),
                Ok(1) => self.stop(),
                Ok(2) => self.pause_resume(ctx),
                Err(e) => self.error = Some(e),
                _ => {}
            }
        }
        let selected = self.picker.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = selected {
            self.picker = None;
            match result {
                Ok(Some(r)) => {
                    self.set_region(r);
                    self.show_overlay();
                }
                Err(e) => self.error = Some(e.to_string()),
                _ => {}
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.phase == Phase::Countdown
            && self
                .countdown
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(3))
        {
            match self.source() {
                Ok(source) => {
                    native::exclude_main_window(true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    self.stop_preview();
                    let mut prefs = self.prefs.clone();
                    // Available tracks can be enabled from the floating toolbar.
                    prefs.system_audio = !self.system_devices.is_empty();
                    prefs.microphone = !self.mic_devices.is_empty();
                    self.controller = Some(recorder::start_live(
                        RecordingOptions { source, prefs },
                        Some(self.live.clone()),
                    ));
                    self.phase = Phase::Starting;
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    self.phase = Phase::Ready;
                }
            }
        }
        let events = self
            .controller
            .as_ref()
            .map(|c| c.events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut ended = false;
        for event in events {
            match event {
                Event::Starting => self.phase = Phase::Starting,
                Event::Recording { encoder } => {
                    self.encoder = encoder;
                    self.phase = Phase::Recording;
                }
                Event::Paused => {
                    self.phase = Phase::Paused;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                }
                Event::Finishing => {
                    if self.phase != Phase::Pausing {
                        self.phase = Phase::Finishing;
                    }
                }
                Event::Progress { seconds, bytes } => {
                    self.seconds = seconds;
                    self.bytes = bytes;
                }
                Event::Saved(path) => {
                    self.last_file = Some(path);
                    self.phase = Phase::Ready;
                    ended = true;
                }
                Event::Failed(e) => {
                    self.error = Some(e);
                    self.phase = Phase::Ready;
                    ended = true;
                }
            }
        }
        if ended {
            self.controller = None;
            self.live.lock().unwrap().tool = studio::Tool::Pointer;

            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            if self.close_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn set_region(&mut self, a: Area) {
        self.region = Some(a);
        let mut l = self.live.lock().unwrap();
        if l.area != a {
            l.area = a;
            l.clear();
        }
    }
    fn show_overlay(&mut self) {
        if self.overlay.is_some() {
            return;
        }
        let Ok(source) = self.source() else {
            return;
        };
        let (area, bounds) = match source {
            Source::Monitor { region, bounds, .. } => (region, bounds),
            Source::Window { handle } => {
                let Some(a) = native::window_area(handle) else {
                    return;
                };
                let bounds = self
                    .monitors
                    .iter()
                    .find(|m| a.x >= m.area.x && a.x < m.area.x + m.area.width as i32)
                    .map(|m| m.area)
                    .unwrap_or(a);
                (a, bounds)
            }
        };
        {
            let mut l = self.live.lock().unwrap();
            l.language = self.prefs.language;
            l.area = area;
            l.allow_resize = self.mode == 0;
            l.system_on = self.prefs.system_audio;
            l.mic_on = self.prefs.microphone;
        }
        self.overlay = Some(overlay::spawn(self.live.clone(), bounds));
    }
    fn stop_preview(&mut self) {
        if let Some(s) = self.preview_stop.take() {
            s.store(true, Ordering::Relaxed);
        }
        self.preview_key.clear();
    }
    fn studio_tick(&mut self, ctx: &egui::Context) {
        if self.mode == 2 && self.overlay.is_some() {
            if let Some(w) = self.windows.get(self.window) {
                if let Some(area) = native::window_area(w.handle) {
                    let mut l = self.live.lock().unwrap();
                    if self.phase == Phase::Ready
                        && (l.area.width != area.width || l.area.height != area.height)
                    {
                        l.clear();
                    }
                    l.area = area;
                }
            }
        }
        let actions = self
            .overlay
            .as_ref()
            .map(|o| o.actions.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for action in actions {
            match action {
                studio::Action::StartStop => {
                    if self.phase == Phase::Ready {
                        self.begin()
                    } else if matches!(
                        self.phase,
                        Phase::Countdown | Phase::Recording | Phase::Paused
                    ) {
                        self.stop()
                    }
                }
                studio::Action::Pause => self.pause_resume(ctx),
                studio::Action::Screenshot => self.screenshot(),
                studio::Action::ShowMain => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                studio::Action::Dismiss => {
                    if self.phase == Phase::Ready {
                        self.overlay.take();
                        let mut live = self.live.lock().unwrap();
                        live.tool = studio::Tool::Pointer;
                        live.draft = None;
                        live.changed();
                    }
                }
            }
        }
        {
            let mut l = self.live.lock().unwrap();
            if self.phase == Phase::Ready {
                if self.mode == 0 && self.overlay.is_some() && l.area.width >= 16 {
                    self.region = Some(l.area);
                }
                // Floating mute controls and the main panel share the same state.
                if self.overlay.is_some() {
                    self.prefs.system_audio = l.system_on;
                    self.prefs.microphone = l.mic_on;
                } else {
                    l.system_on = self.prefs.system_audio;
                    l.mic_on = self.prefs.microphone;
                }
            }
            if self.phase != Phase::Ready {
                self.prefs.system_audio = l.system_on;
                self.prefs.microphone = l.mic_on;
            }
            l.system_gain = self.prefs.system_volume;
            l.mic_gain = self.prefs.mic_volume;
            l.language = self.prefs.language;
            l.cursor_highlight = self.prefs.cursor_highlight;
            l.click_effect = self.prefs.click_effect;
            l.seconds = self.seconds;
            l.bytes = self.bytes;
            l.countdown = 3 - self
                .countdown
                .map(|t| t.elapsed().as_secs().min(3))
                .unwrap_or(0);
            l.status = match self.phase {
                Phase::Ready => Status::Ready,
                Phase::Countdown => Status::Countdown,
                Phase::Starting => Status::Starting,
                Phase::Recording => Status::Recording,
                Phase::Paused => Status::Paused,
                _ => Status::Busy,
            };
            if let Some(e) = l.fault.take() {
                self.error = Some(e);
            }
        }
        if self.phase == Phase::Ready && self.picker.is_none() {
            if let Ok(source) = self.source() {
                let key = format!("{source:?}");
                if self.preview_candidate.0 != key {
                    self.preview_candidate = (key.clone(), Instant::now());
                    self.stop_preview();
                    self.preview_texture = None;
                    self.live.lock().unwrap().latest = None;
                }
                if self.preview_key != key
                    && self.preview_candidate.1.elapsed() > Duration::from_millis(350)
                {
                    self.stop_preview();
                    self.preview_key = key;
                    let stop = Arc::new(AtomicBool::new(false));
                    self.preview_stop = Some(stop.clone());
                    let live = self.live.clone();
                    std::thread::spawn(move || {
                        if let Ok(session) = crate::capture::ScreenSession::start(&source, false, 5)
                        {
                            while !stop.load(Ordering::Relaxed) {
                                let image = session.latest.lock().unwrap().image.clone();
                                if let Some(image) = image {
                                    if !stop.load(Ordering::Relaxed) {
                                        let mut l = live.lock().unwrap();
                                        if !stop.load(Ordering::Relaxed) {
                                            l.latest = Some(image);
                                        }
                                    }
                                }
                                std::thread::sleep(Duration::from_millis(150));
                            }
                        }
                    });
                }
            } else {
                self.stop_preview();
                self.preview_texture = None;
            }
        }
        if let Some(result) = self
            .screenshot_result
            .as_ref()
            .and_then(|r| r.try_recv().ok())
        {
            self.screenshot_result = None;
            match result {
                Ok(p) => {
                    self.notice = Some(format!(
                        "{}: {}",
                        self.prefs.language.text("Đã chụp", "Screenshot saved"),
                        p.file_name().unwrap_or_default().to_string_lossy()
                    ))
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn screenshot(&mut self) {
        if self.screenshot_result.is_some() {
            return;
        }
        let image = self.live.lock().unwrap().latest.clone();
        let Some(image) = image else {
            self.error = Some(
                self.prefs
                    .language
                    .text(
                        "Chọn nguồn và chờ hình xem trước trước khi chụp.",
                        "Select a source and wait for the preview before taking a screenshot.",
                    )
                    .into(),
            );
            return;
        };
        let live = self.live.clone();
        let output = self.prefs.output.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.screenshot_result = Some(rx);
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<PathBuf> {
                std::fs::create_dir_all(&output)?;
                let path = output.join(format!(
                    "Anh_{}.png",
                    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_%3f")
                ));
                let pixels = studio::composite(&image, &live);
                studio::save_png(
                    &crate::capture::Image::from_bgra(image.width, image.height, pixels),
                    &path,
                )?;
                Ok(path)
            })()
            .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }
    fn preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.preview_updated.elapsed() > Duration::from_millis(250) {
            let image = self.live.lock().unwrap().latest.clone();
            if let Some(img) = image {
                let pixels = studio::composite(&img, &self.live);
                let mut rgba = pixels;
                for p in rgba.chunks_exact_mut(4) {
                    p.swap(0, 2);
                    p[3] = 255;
                }
                let color = egui::ColorImage::from_rgba_unmultiplied(
                    [img.width as usize, img.height as usize],
                    &rgba,
                );
                if let Some(tex) = &mut self.preview_texture {
                    tex.set(color, egui::TextureOptions::LINEAR)
                } else {
                    self.preview_texture = Some(ctx.load_texture(
                        "source_preview",
                        color,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
            self.preview_updated = Instant::now();
        }
        let (r, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 152.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(r, 8, Color32::from_rgb(11, 15, 22));
        if let Some(tex) = &self.preview_texture {
            let size = tex.size_vec2();
            let factor = (r.width() / size.x).min(r.height() / size.y);
            let image = egui::Rect::from_center_size(r.center(), size * factor);
            ui.painter().image(
                tex.id(),
                image,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                Color32::WHITE,
            );
        } else {
            ui.painter().rect_stroke(
                r.shrink2(Vec2::new(135., 40.)),
                8,
                egui::Stroke::new(1.5, ACCENT),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                self.prefs
                    .language
                    .text("Chọn vùng để xem trước", "Select a source to preview"),
                FontId::proportional(14.),
                MUTED,
            );
        }
    }
    fn source_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.label(
                RichText::new(lang.text("NGUỒN GHI HÌNH", "CAPTURE SOURCE"))
                    .small()
                    .strong()
                    .color(MUTED),
            );
            let before = self.mode;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, 0, lang.text("Vùng chọn", "Region"));
                ui.selectable_value(&mut self.mode, 2, lang.text("Ứng dụng", "Application"));
                ui.selectable_value(&mut self.mode, 1, lang.text("Toàn màn hình", "Fullscreen"));
            });
            if before != self.mode {
                self.overlay.take();
                self.stop_preview();
                self.live.lock().unwrap().clear();
                self.live.lock().unwrap().latest = None;
                self.preview_texture = None;
            }
            self.preview(ui, ctx);
            if self.mode < 2 {
                let before = self.monitor;
                egui::ComboBox::from_id_salt("monitor")
                    .width(ui.available_width() - 8.)
                    .selected_text(
                        self.monitors
                            .get(self.monitor)
                            .map(|m| format!("{} · {} × {}", m.name, m.area.width, m.area.height))
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (i, m) in self.monitors.iter().enumerate() {
                            ui.selectable_value(&mut self.monitor, i, &m.name);
                        }
                    });
                if before != self.monitor {
                    self.overlay.take();
                    self.region = None;
                }
                if self.mode == 0 {
                    ui.horizontal(|ui| {
                        if ui
                            .button(lang.text("+ Chọn vùng", "+ Select region"))
                            .clicked()
                        {
                            if let Some(m) = self.monitors.get(self.monitor) {
                                self.overlay.take();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                                self.picker =
                                    Some(native::choose_area(m.area, self.prefs.language));
                            }
                        }
                        if ui
                            .add_enabled(
                                self.region.is_some(),
                                egui::Button::new(
                                    lang.text("Hiện khung & toolbar", "Show frame & toolbar"),
                                ),
                            )
                            .clicked()
                        {
                            self.show_overlay();
                        }
                    });
                    if let Some(mut r) = self.region {
                        let old = r;
                        ui.horizontal(|ui| {
                            ui.label(lang.text("Rộng", "Width"));
                            ui.add(
                                egui::DragValue::new(&mut r.width)
                                    .range(32..=16384)
                                    .speed(2),
                            );
                            ui.label(lang.text("Cao", "Height"));
                            ui.add(
                                egui::DragValue::new(&mut r.height)
                                    .range(32..=16384)
                                    .speed(2),
                            );
                            ui.label(
                                RichText::new(format!("@ {}, {}", r.x, r.y))
                                    .small()
                                    .color(MUTED),
                            );
                        });
                        if r != old {
                            if let Some(m) = self.monitors.get(self.monitor) {
                                r.width = r
                                    .width
                                    .min((m.area.x + m.area.width as i32 - r.x).max(32) as u32)
                                    & !1;
                                r.height = r
                                    .height
                                    .min((m.area.y + m.area.height as i32 - r.y).max(32) as u32)
                                    & !1;
                            }
                            self.set_region(r);
                        }
                    } else {
                        ui.label(
                            RichText::new(lang.text(
                                "Kéo chọn vùng trên màn hình, nhấn Enter để dùng.",
                                "Drag a region on screen, then press Enter.",
                            ))
                            .small()
                            .color(MUTED),
                        );
                    }
                } else {
                    if ui
                        .button(lang.text("Hiện toolbar nổi", "Show floating toolbar"))
                        .clicked()
                    {
                        self.show_overlay();
                    }
                    ui.label(
                        RichText::new(lang.text(
                            "Ghi toàn bộ màn hình đã chọn.",
                            "Record the selected display.",
                        ))
                        .small()
                        .color(MUTED),
                    );
                }
            } else {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.source_filter)
                            .hint_text(
                                self.prefs
                                    .language
                                    .text("Tìm ứng dụng hoặc cửa sổ…", "Search apps or windows…"),
                            )
                            .desired_width(ui.available_width() - 92.0),
                    );
                    if ui
                        .button(self.prefs.language.text("Làm mới", "Refresh"))
                        .on_hover_text(self.prefs.language.text(
                            "Quét lại các ứng dụng đang mở",
                            "Scan open applications again",
                        ))
                        .clicked()
                    {
                        self.windows = native::app_windows();
                        self.window = 0;
                    }
                });
                let needle = self.source_filter.trim().to_lowercase();
                let matches = self
                    .windows
                    .iter()
                    .enumerate()
                    .filter(|(_, window)| {
                        needle.is_empty()
                            || window.name.to_lowercase().contains(&needle)
                            || window.app.to_lowercase().contains(&needle)
                    })
                    .map(|(index, window)| {
                        (
                            index,
                            window.app.clone(),
                            window.name.clone(),
                            window.area,
                            window.process_id,
                        )
                    })
                    .collect::<Vec<_>>();
                ui.label(
                    RichText::new(format!(
                        "{} ({})",
                        self.prefs
                            .language
                            .text("ỨNG DỤNG ĐANG MỞ", "OPEN APPLICATIONS"),
                        matches.len()
                    ))
                    .size(11.0)
                    .strong()
                    .color(MUTED),
                );
                egui::ScrollArea::vertical()
                    .id_salt("active_app_sources")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for (index, app, title, area, pid) in matches {
                            let selected = self.window == index;
                            let row = ui
                                .selectable_label(
                                    selected,
                                    RichText::new(format!("{app}   ·   {title}")).size(13.0),
                                )
                                .on_hover_text(format!(
                                    "{} × {} · PID {}\n{}",
                                    area.width, area.height, pid, title
                                ));
                            if row.clicked() {
                                self.window = index;
                                self.overlay.take();
                                self.stop_preview();
                                self.preview_texture = None;
                            }
                            if selected {
                                ui.label(
                                    RichText::new(format!(
                                        "{} × {}  ·  PID {}",
                                        area.width, area.height, pid
                                    ))
                                    .small()
                                    .color(ACCENT),
                                );
                            }
                        }
                    });
                if self.windows.is_empty() {
                    ui.label(
                        RichText::new(self.prefs.language.text(
                            "Không tìm thấy cửa sổ có thể ghi.",
                            "No recordable windows found.",
                        ))
                        .small()
                        .color(MUTED),
                    );
                }
                if ui
                    .button(
                        self.prefs
                            .language
                            .text("Hiện toolbar nổi", "Show floating toolbar"),
                    )
                    .clicked()
                {
                    self.show_overlay();
                }
                ui.label(
                    RichText::new(self.prefs.language.text(
                        "Giữ nguyên kích thước cửa sổ khi đang ghi.",
                        "Keep the source window at the same size while recording.",
                    ))
                    .small()
                    .color(MUTED),
                );
            }
        });
    }
    fn audio_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing.y = 5.0;
            ui.spacing_mut().button_padding.y = 4.0;
            ui.label(
                RichText::new(lang.text("ÂM THANH", "AUDIO"))
                    .small()
                    .strong()
                    .color(MUTED),
            );
            let mut changed = false;
            let mut toggled = false;
            let levels = self.live.lock().unwrap().levels;
            for mic in [false, true] {
                let idx = usize::from(mic);
                let (on, gain, id, devices, title) = if mic {
                    (
                        &mut self.prefs.microphone,
                        &mut self.prefs.mic_volume,
                        &mut self.prefs.mic_device,
                        &self.mic_devices,
                        "Micro",
                    )
                } else {
                    (
                        &mut self.prefs.system_audio,
                        &mut self.prefs.system_volume,
                        &mut self.prefs.system_device,
                        &self.system_devices,
                        lang.text("Âm thanh máy", "System audio"),
                    )
                };
                ui.push_id(idx, |ui| {
                    ui.horizontal(|ui| {
                        toggled |= ui.checkbox(on, title).changed();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(if *on {
                                    lang.text("Đang bật", "On")
                                } else {
                                    lang.text("Đã tắt", "Off")
                                })
                                .small()
                                .color(if *on {
                                    ACCENT
                                } else {
                                    MUTED
                                }),
                            );
                        });
                    });
                    egui::ComboBox::from_id_salt("device")
                        .width(ui.available_width() - 8.)
                        .selected_text(
                            devices
                                .iter()
                                .find(|d| d.id == *id)
                                .map(|d| d.name.as_str())
                                .unwrap_or(lang.text("Mặc định Windows", "Windows default")),
                        )
                        .show_ui(ui, |ui| {
                            changed |= ui
                                .selectable_value(
                                    id,
                                    String::new(),
                                    lang.text("Mặc định Windows", "Windows default"),
                                )
                                .changed();
                            for d in devices {
                                changed |= ui.selectable_value(id, d.id.clone(), &d.name).changed();
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.add_enabled(*on, egui::Slider::new(gain, 0.0..=2.0).show_value(false));
                        ui.label(
                            RichText::new(format!("{:.0}%", *gain * 100.))
                                .small()
                                .color(MUTED),
                        );
                    });
                    let level = if *on { levels[idx] * *gain } else { 0. };
                    ui.add(
                        egui::ProgressBar::new(level.clamp(0., 1.))
                            .desired_height(4.)
                            .fill(if level > 0.95 {
                                RED
                            } else {
                                Color32::from_rgb(95, 211, 168)
                            }),
                    );
                });
                if !mic {
                    ui.add_space(3.);
                }
            }
            if toggled || self.overlay.is_none() {
                let mut l = self.live.lock().unwrap();
                l.system_on = self.prefs.system_audio;
                l.mic_on = self.prefs.microphone;
            }
            if changed {
                self.meter = Some(audio::meters(self.prefs.clone(), self.live.clone()));
            }
        });
    }
    fn quality_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing.y = 5.0;
            ui.spacing_mut().button_padding.y = 4.0;
            ui.label(
                RichText::new(lang.text("CHẤT LƯỢNG", "QUALITY"))
                    .small()
                    .strong()
                    .color(MUTED),
            );
            ui.horizontal(|ui| {
                for (title, rate, limit) in [
                    (lang.text("Nhẹ", "Light"), 4, 720),
                    (lang.text("Cân bằng", "Balanced"), 8, 0),
                    (lang.text("Rõ chữ", "Sharp text"), 12, 0),
                ] {
                    if ui
                        .selectable_label(
                            self.prefs.bitrate == rate && self.prefs.max_height == limit,
                            title,
                        )
                        .clicked()
                    {
                        self.prefs.bitrate = rate;
                        self.prefs.max_height = limit;
                    }
                }
            });
            ui.label(
                RichText::new(lang.text(
                    "MP4 · 30 fps · Tăng tốc phần cứng tự động",
                    "MP4 · 30 fps · Automatic hardware acceleration",
                ))
                .small()
                .color(MUTED),
            );
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.prefs.cursor_highlight,
                    lang.text("Nổi con trỏ", "Cursor highlight"),
                );
                ui.checkbox(
                    &mut self.prefs.click_effect,
                    lang.text("Hiệu ứng click", "Click effect"),
                );
            });
        });
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.language;
        ui.heading(lang.text("Cài đặt ứng dụng", "Application settings"));
        ui.add_space(12.);
        card(ui, |ui| {
            ui.label(lang.text("Ngôn ngữ", "Language"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.prefs.language, Language::Vi, "Tiếng Việt");
                ui.selectable_value(&mut self.prefs.language, Language::En, "English");
            });
            ui.label(
                RichText::new(lang.text(
                    "Lựa chọn được lưu cho lần mở tiếp theo và áp dụng cho toolbar nổi.",
                    "The choice is saved and also applies to the floating toolbar.",
                ))
                .small()
                .color(MUTED),
            );
        });
        card(ui, |ui| {
            ui.label("Video");
            ui.checkbox(
                &mut self.prefs.cursor,
                lang.text("Ghi con trỏ chuột", "Capture mouse cursor"),
            );
            ui.horizontal(|ui| {
                ui.label("Bitrate (Mbps)");
                ui.add(egui::Slider::new(&mut self.prefs.bitrate, 2..=20));
            });
            egui::ComboBox::from_label(lang.text("Độ phân giải", "Resolution"))
                .selected_text(match self.prefs.max_height {
                    720 => lang.text("Tối đa 720p", "Up to 720p"),
                    1080 => lang.text("Tối đa 1080p", "Up to 1080p"),
                    _ => lang.text("Kích thước gốc", "Original size"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.prefs.max_height,
                        0,
                        lang.text("Kích thước gốc", "Original size"),
                    );
                    ui.selectable_value(
                        &mut self.prefs.max_height,
                        720,
                        lang.text("Tối đa 720p", "Up to 720p"),
                    );
                    ui.selectable_value(
                        &mut self.prefs.max_height,
                        1080,
                        lang.text("Tối đa 1080p", "Up to 1080p"),
                    );
                });
            egui::ComboBox::from_label(lang.text("Bộ mã hóa", "Encoder"))
                .selected_text(&self.prefs.encoder)
                .show_ui(ui, |ui| {
                    for (v, label) in [
                        ("auto", lang.text("Tự động", "Automatic")),
                        ("qsv", "Intel Quick Sync"),
                        ("cpu", "CPU"),
                    ] {
                        ui.selectable_value(&mut self.prefs.encoder, v.into(), label);
                    }
                });
        });
        card(ui, |ui| {
            ui.label(lang.text("Phím tắt toàn hệ thống", "Global shortcuts"));
            ui.label(lang.text(
                "Ctrl + Shift + F9     Bắt đầu / dừng ghi",
                "Ctrl + Shift + F9     Start / stop recording",
            ));
            ui.label(lang.text(
                "Ctrl + Shift + F10   Tạm dừng / tiếp tục",
                "Ctrl + Shift + F10   Pause / resume",
            ));
            ui.label(lang.text(
                "Esc   Thoát vẽ     ·     Ctrl + Z / Y   Hoàn tác / làm lại",
                "Esc   Exit drawing     ·     Ctrl + Z / Y   Undo / redo",
            ));
        });
        card(ui, |ui| {
            ui.label(lang.text("Khôi phục phiên ghi", "Session recovery"));
            if ui
                .button(lang.text(
                    "Phục hồi bản ghi bị gián đoạn…",
                    "Recover an interrupted recording…",
                ))
                .clicked()
            {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Chọn phiên Quay_… trong .sessions")
                    .set_directory(self.prefs.output.join(".sessions"))
                    .pick_folder()
                {
                    self.error = None;
                    self.controller = Some(recorder::recover_async(folder));
                    self.phase = Phase::Finishing;
                }
            }
        });
    }
    fn library(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.language;
        ui.horizontal(|ui| {
            ui.heading(lang.text("Video & ảnh đã lưu", "Saved videos & images"));
            if ui.button(lang.text("Mở thư mục", "Open folder")).clicked() {
                open(&self.prefs.output);
            }
        });
        ui.add_space(10.);
        let mut files = std::fs::read_dir(&self.prefs.output)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                matches!(
                    p.extension().and_then(|s| s.to_str()),
                    Some("mp4" | "mkv" | "webm" | "mov" | "png")
                )
            })
            .collect::<Vec<_>>();
        files.sort();
        files.reverse();
        if files.is_empty() {
            ui.label(lang.text(
                "Bản ghi và ảnh chụp của bạn sẽ xuất hiện tại đây.",
                "Your recordings and screenshots will appear here.",
            ));
        }
        for path in files.iter().take(80) {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                        ui.label(
                            RichText::new(format!(
                                "{:.1} MB",
                                path.metadata().map(|m| m.len()).unwrap_or(0) as f64 / 1_000_000.
                            ))
                            .small()
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(lang.text("Mở", "Open")).clicked() {
                            open(path);
                        }
                        if matches!(
                            path.extension().and_then(|s| s.to_str()),
                            Some("mp4" | "mkv" | "webm" | "mov")
                        ) && ui.button(lang.text("Chỉnh sửa", "Edit clip")).clicked()
                        {
                            self.editor.open(path.clone());
                            self.tab = 2;
                        }
                    });
                });
            });
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
        let lang = self.prefs.language;
        if ctx.input(|i| i.viewport().close_requested()) && self.phase != Phase::Ready {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_requested = true;
            self.stop();
            if self.phase == Phase::Ready {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        egui::TopBottomPanel::top("navigation")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("●").size(23.).color(RED));
                    ui.label(RichText::new("Screen Recorder").size(19.).strong());
                    ui.add_space(25.);
                    let lang = self.prefs.language;
                    ui.selectable_value(&mut self.tab, 0, lang.text("Ghi hình", "Record"));
                    ui.selectable_value(&mut self.tab, 1, lang.text("Thư viện", "Library"));
                    ui.selectable_value(&mut self.tab, 2, lang.text("Chỉnh sửa", "Editor"));
                    ui.selectable_value(&mut self.tab, 3, lang.text("Cài đặt", "Settings"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("language_top")
                            .width(58.0)
                            .selected_text(if lang == Language::Vi { "VI" } else { "EN" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.prefs.language, Language::Vi, "VI");
                                ui.selectable_value(&mut self.prefs.language, Language::En, "EN");
                            });
                        ui.label(RichText::new("STUDIO  /  0.3.1").size(11.).color(MUTED));
                    });
                });
            });
        if self.tab != 2 || self.phase != Phase::Ready {
            egui::TopBottomPanel::bottom("controls").frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(20,12))).show(ctx,|ui|{
            ui.horizontal(|ui|{
                let t=self.seconds as u64;
                ui.vertical(|ui|{ui.label(RichText::new(format!("{:02}:{:02}:{:02}",t/3600,t/60%60,t%60)).size(25.).monospace());ui.label(RichText::new(match self.phase{Phase::Ready=>lang.text("Sẵn sàng ghi hình","Ready to record"),Phase::Recording=>lang.text("Đang ghi hình","Recording"),Phase::Paused=>lang.text("Đã tạm dừng","Paused"),Phase::Countdown=>lang.text("Đếm ngược","Countdown"),Phase::Starting=>lang.text("Đang chuẩn bị…","Preparing…"),_=>lang.text("Đang hoàn tất video…","Finalizing video…")}).small().color(MUTED));});
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    match self.phase{
                        Phase::Ready=>{if ui.add_sized([175.,44.],egui::Button::new(RichText::new(lang.text("●  Bắt đầu ghi","●  Start recording")).strong().color(Color32::WHITE)).fill(RED).corner_radius(8)).clicked(){self.begin();}
 if ui.button(lang.text("Chụp ảnh","Screenshot")).on_hover_text(lang.text("Lưu PNG của vùng đang chọn","Save the selected region as PNG")).clicked(){self.screenshot();}},
                        Phase::Countdown=>{if ui.button(lang.text("Hủy","Cancel")).clicked(){self.stop();}ui.label(format!("{} {}…",lang.text("Bắt đầu sau","Starting in"),3-self.countdown.unwrap().elapsed().as_secs().min(3)));},
                        Phase::Recording|Phase::Paused=>{if ui.add(egui::Button::new(lang.text("■  Dừng & lưu","■  Stop & save")).fill(RED)).clicked(){self.stop();}
 if ui.button(if self.phase==Phase::Paused{lang.text("Tiếp tục","Resume")}else{lang.text("Tạm dừng","Pause")}).clicked(){self.pause_resume(ctx);}
 if ui.button(lang.text("Toolbar nổi","Floating toolbar")).on_hover_text(lang.text("Hiện thanh điều khiển không ghi vào video","Show controls excluded from the video")).clicked(){self.show_overlay();}},
                        _=>{ui.spinner();},
                    }
                });
            });
            ui.label(RichText::new(lang.text("Ctrl + Shift + F9  Ghi / dừng     ·     Ctrl + Shift + F10  Tạm dừng / tiếp tục","Ctrl + Shift + F9  Record / stop     ·     Ctrl + Shift + F10  Pause / resume")).size(11.).color(MUTED));
        });
        }
        if self.tab == 0 {
            egui::TopBottomPanel::bottom("output_folder")
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(14, 18, 25))
                        .inner_margin(egui::Margin::symmetric(18, 6)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(lang.text("LƯU TẠI", "SAVE TO"))
                                .size(11.)
                                .color(MUTED),
                        );
                        ui.add_sized(
                            [ui.available_width() - 130., 22.],
                            egui::Label::new(
                                RichText::new(self.prefs.output.display().to_string()).small(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(self.prefs.output.display().to_string());
                        if ui
                            .add_enabled(
                                self.phase == Phase::Ready,
                                egui::Button::new(lang.text("Đổi…", "Change…")).small(),
                            )
                            .clicked()
                        {
                            if let Some(p) = rfd::FileDialog::new()
                                .set_directory(&self.prefs.output)
                                .pick_folder()
                            {
                                self.prefs.output = p;
                            }
                        }
                        if ui.small_button(lang.text("Mở", "Open")).clicked() {
                            open(&self.prefs.output);
                        }
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(14, 18, 25))
                    .inner_margin(16.),
            )
            .show(ctx, |ui| {
                if self.tab == 2 {
                    ui.add_enabled_ui(self.phase == Phase::Ready, |ui| {
                        self.editor
                            .ui(ctx, ui, &self.prefs.output, self.prefs.language);
                    });
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(error) = self.error.clone() {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(65, 29, 39))
                            .corner_radius(6)
                            .inner_margin(8.)
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(error)
                                            .color(Color32::from_rgb(255, 190, 197)),
                                    );
                                    if ui.small_button(lang.text("Đóng", "Dismiss")).clicked() {
                                        self.error = None;
                                    }
                                });
                            });
                        ui.add_space(6.);
                    }
                    if let Some(notice) = self.notice.clone() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(notice).small().color(ACCENT));
                            if ui.small_button("×").clicked() {
                                self.notice = None;
                            }
                        });
                    }
                    match self.tab {
                        0 => {
                            ui.add_enabled_ui(
                                self.phase == Phase::Ready && self.picker.is_none(),
                                |ui| {
                                    let width = ui.available_width();
                                    let left = (width - 8.) * 0.56;
                                    ui.horizontal_top(|ui| {
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(left, 360.),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                self.source_panel(ui, ctx);
                                            },
                                        );
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(width - left - 8., 360.),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                self.audio_panel(ui);
                                                self.quality_panel(ui);
                                            },
                                        );
                                    });
                                },
                            );
                            if let Some(path) = &self.last_file {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(lang.text("Đã lưu video", "Video saved"))
                                            .small()
                                            .color(ACCENT),
                                    );
                                    if ui
                                        .small_button(lang.text("Xem video", "Open video"))
                                        .clicked()
                                    {
                                        open(path);
                                    }
                                    if ui
                                        .small_button(lang.text("Chỉnh sửa", "Edit clip"))
                                        .clicked()
                                    {
                                        self.editor.open(path.clone());
                                        self.tab = 2;
                                    }
                                });
                            }
                        }
                        1 => self.library(ui),
                        2 => {}
                        _ => {
                            ui.add_enabled_ui(self.phase == Phase::Ready, |ui| self.settings(ui));
                        }
                    }
                });
            });
        ctx.request_repaint_after(Duration::from_millis(80));
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_preview();
        self.overlay.take();
        self.save_settings();
    }
}
fn open(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
}
fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1., Color32::from_rgb(39, 49, 64)))
        .corner_radius(10)
        .inner_margin(12.)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui);
        });
    ui.add_space(6.);
}
