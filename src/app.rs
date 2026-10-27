use crate::{
    audio, editor_launcher,
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

#[path = "recording_qa.rs"]
mod qa;

const INK: Color32 = Color32::from_rgb(234, 239, 247);
const MUTED: Color32 = Color32::from_rgb(157, 174, 195);
const PANEL: Color32 = Color32::from_rgb(23, 32, 46);
const ACCENT: Color32 = Color32::from_rgb(111, 198, 237);
const RED: Color32 = Color32::from_rgb(218, 62, 78);

#[derive(PartialEq, Debug)]
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
    shortcut_at: [Option<Instant>; 2],
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

    screenshot_result: Option<Receiver<Result<PathBuf, String>>>,
    notice: Option<String>,
    system_devices: Vec<audio::Device>,
    mic_devices: Vec<audio::Device>,
    meter: Option<audio::Meter>,
    settings_path: PathBuf,
    editor: editor_launcher::EditorLauncher,
    library: crate::library_ui::LibraryUi,
    qa_capture_at: Instant,
    sources: crate::source_picker::Sources,
    inventory_ready: bool,
    show_result: bool,
    auto_edit_pending: bool,
    recording_audio: [bool; 2],
    qa_stage: u8,
    qa_clock: Instant,
    qa_trace: Vec<String>,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_theme(egui::Theme::Dark);
        if std::env::args().any(|a| a == "--inspect-ui") {
            if let Ok(value) = std::env::var("SCREENRECORDER_UI_SCALE") {
                if let Ok(scale) = value.parse::<f32>() {
                    ctx.set_zoom_factor(scale.clamp(1., 1.5));
                }
            }
        }
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
        let settings_path = std::env::var_os("SCREENRECORDER_SETTINGS_PATH")
            .map(PathBuf::from)
            .unwrap_or(settings_path);
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
        let library = crate::library_ui::LibraryUi::new(prefs.output.clone());
        Self {
            library,
            qa_capture_at: Instant::now(),
            sources: crate::source_picker::Sources::new(),
            inventory_ready: false,
            show_result: false,
            auto_edit_pending: false,
            recording_audio: [false; 2],
            qa_stage: 0,
            qa_clock: Instant::now(),
            qa_trace: vec![],
            prefs,
            monitors: vec![],
            windows: vec![],
            monitor: 0,
            window: 0,
            source_filter: String::new(),
            mode: 1,
            region: None,
            picker: None,
            controller: None,
            hotkeys: native::hotkeys(),
            shortcut_at: [None, None],
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
            tab: if std::env::var_os("SCREENRECORDER_LIBRARY_VIEW").is_some() {
                1
            } else {
                0
            },
            preview_stop: None,
            preview_key: String::new(),
            preview_candidate: (String::new(), Instant::now()),

            screenshot_result: None,
            notice: None,
            system_devices: vec![],
            mic_devices: vec![],
            meter,
            settings_path,
            editor: editor_launcher::EditorLauncher::default(),
        }
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
        if !self.inventory_ready || self.picker.is_some() {
            return;
        }
        if self.prefs.microphone
            && !audio::device_available(&self.mic_devices, &self.prefs.mic_device)
        {
            self.error = Some(
                self.prefs
                    .language
                    .text(
                        "Micro đã chọn không khả dụng. Chọn thiết bị khác hoặc tắt Micro.",
                        "Selected microphone unavailable. Choose another device or turn Microphone off.",
                    )
                    .into(),
            );
            return;
        }
        if self.prefs.system_audio
            && !audio::device_available(&self.system_devices, &self.prefs.system_device)
        {
            self.error = Some(
                self.prefs
                    .language
                    .text(
                        "Thiết bị tiếng máy đã chọn không khả dụng. Chọn lại loa/tai nghe.",
                        "Selected system audio device unavailable. Choose another speaker or headset.",
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
                self.auto_edit_pending = self.prefs.edit_after_recording;
                self.show_result = false;
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
            self.auto_edit_pending = false;
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
                if !std::env::args().any(|a| a == "--ui-smoke") {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                let _ = c.commands.send(Command::Resume);
                self.phase = Phase::Starting;
            }
        }
    }
    fn shortcut(&mut self, id: u32, ctx: &egui::Context) {
        let Some(last) = self.shortcut_at.get_mut(id.saturating_sub(1) as usize) else {
            return;
        };
        if last.is_some_and(|t| t.elapsed() < Duration::from_millis(500)) {
            return;
        }
        *last = Some(Instant::now());
        match id {
            1 if self.phase == Phase::Ready => self.begin(),
            1 if matches!(
                self.phase,
                Phase::Countdown | Phase::Recording | Phase::Paused
            ) =>
            {
                self.stop()
            }
            2 => self.pause_resume(ctx),
            _ => {}
        }
    }
    fn tick(&mut self, ctx: &egui::Context) {
        native::exclude_main_window(true);
        self.refresh_sources(ctx);
        self.studio_tick(ctx);
        while let Ok(key) = self.hotkeys.try_recv() {
            match key {
                Ok(id) => self.shortcut(id, ctx),
                Err(e) => self.error = Some(e),
            }
        }
        // Local fallback also works when another application owns the global shortcut.
        let keys = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } if modifiers.ctrl && modifiers.shift => match key {
                        egui::Key::F9 => Some(1),
                        egui::Key::F10 => Some(2),
                        _ => None,
                    },
                    _ => None,
                })
                .collect::<Vec<_>>()
        });
        for key in keys {
            self.shortcut(key, ctx);
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
                    if !std::env::args().any(|a| a == "--ui-smoke") {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    self.stop_preview();
                    let mut prefs = self.prefs.clone();
                    // Available tracks can be enabled from the floating toolbar.
                    prefs.system_audio =
                        audio::device_available(&self.system_devices, &prefs.system_device);
                    prefs.microphone =
                        audio::device_available(&self.mic_devices, &prefs.mic_device);
                    self.recording_audio = [prefs.system_audio, prefs.microphone];
                    self.controller = Some(recorder::start_live(
                        RecordingOptions { source, prefs },
                        Some(self.live.clone()),
                    ));
                    self.phase = Phase::Starting;
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    self.auto_edit_pending = false;
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
        let mut opened_editor = false;
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
                    self.last_file = Some(path.clone());
                    self.show_result = true;
                    self.tab = 0;
                    self.overlay.take();
                    self.library.refresh();
                    if self.auto_edit_pending && !self.close_requested {
                        self.editor.open(path);
                        opened_editor = true;
                    }
                    self.auto_edit_pending = false;
                    self.phase = Phase::Ready;
                    ended = true;
                }
                Event::SavedWithWarning { path, warning } => {
                    self.last_file = Some(path);
                    self.show_result = true;
                    self.tab = 0;
                    self.error = Some(warning);
                    self.auto_edit_pending = false;
                    self.overlay.take();
                    self.library.refresh();
                    self.phase = Phase::Ready;
                    ended = true;
                }
                Event::Failed(e) => {
                    self.error = Some(e);
                    self.auto_edit_pending = false;
                    self.overlay.take();
                    self.phase = Phase::Ready;
                    ended = true;
                }
            }
        }
        if ended {
            self.controller = None;
            self.live.lock().unwrap().tool = studio::Tool::Pointer;

            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            if !opened_editor {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
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
    fn refresh_sources(&mut self, ctx: &egui::Context) {
        self.sources.tick(ctx);
        let inventories: Vec<_> = self.sources.inventory.try_iter().collect();
        for mut inventory in inventories {
            if std::env::args().any(|a| a == "--ui-smoke") {
                let pid = std::env::var("SCREENRECORDER_QA_PATTERN_PID")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok());
                inventory.windows.retain(|w| {
                    w.name == "Recorder Test Pattern" && pid.is_none_or(|p| w.process_id == p)
                });
            }
            let display = self.monitors.get(self.monitor).map(|m| m.handle);
            let window = self
                .windows
                .get(self.window)
                .map(|w| (w.handle, w.process_id));
            self.monitor = if !self.inventory_ready {
                0
            } else {
                display
                    .and_then(|id| inventory.displays.iter().position(|m| m.handle == id))
                    .unwrap_or(usize::MAX)
            };
            self.window = if !self.inventory_ready {
                0
            } else {
                window
                    .and_then(|id| {
                        inventory
                            .windows
                            .iter()
                            .position(|w| (w.handle, w.process_id) == id)
                    })
                    .unwrap_or(usize::MAX)
            };
            self.monitors = inventory.displays;
            self.windows = inventory.windows;
            self.system_devices = inventory.speakers;
            self.mic_devices = inventory.microphones;
            self.inventory_ready = true;
            self.live.lock().unwrap().audio_available = [
                audio::device_available(&self.system_devices, &self.prefs.system_device),
                audio::device_available(&self.mic_devices, &self.prefs.mic_device),
            ];
            if !matches!(self.phase, Phase::Ready | Phase::Countdown) {
                let mut live = self.live.lock().unwrap();
                for (available, track) in live.audio_available.iter_mut().zip(self.recording_audio)
                {
                    *available &= track;
                }
            }
            // Never silently switch to another source when a window/display disappears.
            if self.phase == Phase::Countdown
                && (self.source().is_err()
                    || (self.prefs.microphone
                        && !audio::device_available(&self.mic_devices, &self.prefs.mic_device))
                    || (self.prefs.system_audio
                        && !audio::device_available(
                            &self.system_devices,
                            &self.prefs.system_device,
                        )))
            {
                self.stop();
                self.error = Some(
                    self.prefs
                        .language
                        .text(
                            "Nguồn quay hoặc thiết bị âm thanh đã mất kết nối. Hãy chọn lại.",
                            "Capture source or audio device disconnected. Select it again.",
                        )
                        .into(),
                );
            }
        }
    }
    fn choose_region(&mut self, ctx: &egui::Context) {
        if let Some(m) = self.monitors.get(self.monitor) {
            self.overlay.take();
            if !std::env::args().any(|a| a == "--ui-smoke") {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            self.picker = Some(native::choose_area(m.area, self.prefs.language));
        }
    }
    fn source_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.label(RichText::new(lang.text("1  Nguồn quay", "1  Capture source")).strong());
            let before = (self.mode, self.monitor, self.window);
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (1, lang.text("Màn hình", "Screen")),
                    (2, lang.text("Cửa sổ", "Window")),
                    (0, lang.text("Vùng chọn", "Region")),
                ] {
                    ui.add_sized(
                        [96., 36.],
                        egui::Button::new(label).selected(self.mode == mode),
                    )
                    .clicked()
                    .then(|| self.mode = mode);
                }
            });
            if !self.inventory_ready {
                ui.spinner();
                ui.label(lang.text("Đang tìm nguồn…", "Finding sources…"));
            }
            if self.mode == 2 {
                ui.add(
                    egui::TextEdit::singleline(&mut self.source_filter)
                        .hint_text(lang.text("Tìm cửa sổ…", "Find a window…"))
                        .desired_width(f32::INFINITY),
                );
            }
            let filter = self.source_filter.to_lowercase();
            let rows: Vec<_> = if self.mode == 2 {
                self.windows
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| {
                        filter.is_empty()
                            || format!("{} {}", w.name, w.app)
                                .to_lowercase()
                                .contains(&filter)
                    })
                    .map(|(i, w)| {
                        (
                            i,
                            format!("{} · {}", w.app, w.name),
                            w.area,
                            Source::Window { handle: w.handle },
                        )
                    })
                    .collect()
            } else {
                self.monitors
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        (
                            i,
                            m.name.clone(),
                            m.area,
                            Source::Monitor {
                                handle: m.handle,
                                bounds: m.area,
                                region: m.area,
                            },
                        )
                    })
                    .collect()
            };
            egui::ScrollArea::vertical()
                .id_salt("source_tiles")
                .max_height(230.)
                .show_rows(ui, 76., rows.len(), |ui, range| {
                    for n in range {
                        let (index, title, area, source) = &rows[n];
                        let selected = if self.mode == 2 {
                            self.window == *index
                        } else {
                            self.monitor == *index
                        };
                        let texture = self.sources.texture(source.clone());
                        let response =
                            source_tile(ui, title, *area, selected, texture.as_ref(), lang);
                        if response.clicked() {
                            if self.mode == 2 {
                                self.window = *index;
                            } else {
                                self.monitor = *index;
                            }
                        }
                    }
                });
            if rows.is_empty() && self.inventory_ready {
                ui.label(lang.text(
                    "Không có nguồn phù hợp. Danh sách tự cập nhật.",
                    "No matching source. This list updates automatically.",
                ));
            }
            if before != (self.mode, self.monitor, self.window) {
                self.overlay.take();
                self.stop_preview();

                let mut live = self.live.lock().unwrap();
                live.clear();
                live.latest = None;
                if before.1 != self.monitor {
                    self.region = None;
                }
            }
            if self.mode == 0 {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(lang.text("Chọn vùng…", "Select region…"))
                        .clicked()
                    {
                        self.choose_region(ctx);
                    }
                    if ui
                        .button(lang.text("Toàn màn hình này", "Use this display"))
                        .clicked()
                    {
                        if let Some(m) = self.monitors.get(self.monitor) {
                            self.set_region(m.area);
                        }
                    }
                });
                if let Some(mut a) = self.region {
                    ui.label(
                        RichText::new(format!(
                            "{} × {} px · X {}, Y {}",
                            a.width, a.height, a.x, a.y
                        ))
                        .color(ACCENT),
                    );
                    egui::CollapsingHeader::new(
                        lang.text("Nhập tọa độ bằng bàn phím", "Enter region with keyboard"),
                    )
                    .show(ui, |ui| {
                        let old = a;
                        ui.horizontal_wrapped(|ui| {
                            for (label, value) in [("X", &mut a.x), ("Y", &mut a.y)] {
                                ui.label(label);
                                ui.add(egui::DragValue::new(value));
                            }
                            ui.label(lang.text("Rộng", "Width"));
                            ui.add(egui::DragValue::new(&mut a.width).range(16..=16384));
                            ui.label(lang.text("Cao", "Height"));
                            ui.add(egui::DragValue::new(&mut a.height).range(16..=16384));
                        });
                        if old != a {
                            a.width &= !1;
                            a.height &= !1;
                            self.set_region(a);
                        }
                    });
                } else {
                    ui.label(lang.text(
                        "Kéo chọn vùng hoặc dùng toàn màn hình rồi nhập tọa độ.",
                        "Drag a region, or use this display then enter coordinates.",
                    ));
                }
            }
            if let Err(e) = self.source() {
                ui.label(RichText::new(e.to_string()).color(Color32::from_rgb(255, 190, 197)));
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        self.source().is_ok(),
                        egui::Button::new(
                            lang.text("Toolbar & chú thích", "Toolbar & annotations"),
                        ),
                    )
                    .clicked()
                {
                    self.show_overlay();
                }
                ui.checkbox(&mut self.prefs.cursor, lang.text("Con trỏ", "Cursor"));
            });
        });
    }
    fn audio_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.;
            ui.spacing_mut().interact_size.y = 24.;
            ui.spacing_mut().button_padding.y = 4.;
            ui.label(RichText::new(lang.text("2  Âm thanh", "2  Audio")).strong());
            let levels = self.live.lock().unwrap().levels;
            let mut changed = false;
            for mic in [true, false] {
                let (on, id, devices, title) = if mic {
                    (
                        &mut self.prefs.microphone,
                        &mut self.prefs.mic_device,
                        &self.mic_devices,
                        lang.text("Micro", "Microphone"),
                    )
                } else {
                    (
                        &mut self.prefs.system_audio,
                        &mut self.prefs.system_device,
                        &self.system_devices,
                        lang.text("Tiếng máy", "System audio"),
                    )
                };
                ui.push_id(mic, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(on, title);
                        ui.label(
                            RichText::new(if *on {
                                lang.text("Bật", "On")
                            } else {
                                lang.text("Tắt", "Off")
                            })
                            .small()
                            .color(if *on { ACCENT } else { MUTED }),
                        );
                    });
                    let available = audio::device_available(devices, id);
                    let name = if id.is_empty() {
                        lang.text("Mặc định Windows", "Windows default").to_owned()
                    } else {
                        devices
                            .iter()
                            .find(|d| d.id == *id)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| {
                                lang.text("Thiết bị đã mất kết nối", "Device disconnected")
                                    .into()
                            })
                    };
                    egui::ComboBox::from_id_salt("audio_device")
                        .width(ui.available_width() - 8.)
                        .selected_text(egui::RichText::new(name).size(13.))
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
                    if !available && self.inventory_ready {
                        ui.label(
                            RichText::new(lang.text(
                                "Mất kết nối · chọn thiết bị khác hoặc tắt",
                                "Disconnected · choose another device or turn off",
                            ))
                            .small()
                            .color(Color32::from_rgb(255, 190, 197)),
                        );
                    }
                    let level = if *on && available {
                        levels[usize::from(mic)]
                    } else {
                        0.
                    };
                    ui.add(
                        egui::ProgressBar::new(level.clamp(0., 1.))
                            .desired_height(8.)
                            .fill(if level > 0.95 {
                                RED
                            } else {
                                Color32::from_rgb(95, 211, 168)
                            }),
                    )
                    .on_hover_text(
                        lang.text("Mức tín hiệu thực của thiết bị", "Live device signal level"),
                    );
                    if mic {
                        ui.separator();
                    }
                });
            }
            {
                let mut live = self.live.lock().unwrap();
                live.system_on = self.prefs.system_audio;
                live.mic_on = self.prefs.microphone;
            }
            if changed {
                self.meter = Some(audio::meters(self.prefs.clone(), self.live.clone()));
            }
        });
    }
    fn quality_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.;
            ui.spacing_mut().interact_size.y = 24.;
            ui.spacing_mut().button_padding.y = 4.;
            ui.label(
                RichText::new(lang.text("3  Chất lượng & cách quay", "3  Quality & workflow"))
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                for (preset, label) in [
                    (QualityPreset::Light, lang.text("Nhẹ", "Light")),
                    (QualityPreset::Balanced, lang.text("Cân bằng", "Balanced")),
                    (
                        QualityPreset::High,
                        lang.text("Chất lượng cao", "High quality"),
                    ),
                ] {
                    if ui
                        .selectable_label(preset.matches(&self.prefs), label)
                        .clicked()
                    {
                        preset.apply(&mut self.prefs);
                    }
                }
            });
            let resolution = match self.prefs.max_height {
                0 => lang.text("Kích thước gốc", "Original size").to_owned(),
                h => format!("≤ {h}p"),
            };
            let custom = ![
                QualityPreset::Light,
                QualityPreset::Balanced,
                QualityPreset::High,
            ]
            .iter()
            .any(|p| p.matches(&self.prefs));
            ui.label(
                RichText::new(format!(
                    "{}MP4 · {} · {} fps",
                    if custom {
                        lang.text("Tùy chỉnh · ", "Custom · ")
                    } else {
                        ""
                    },
                    resolution,
                    self.prefs.fps
                ))
                .small()
                .color(MUTED),
            );
            egui::CollapsingHeader::new(lang.text("Nâng cao", "Advanced")).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Mbps");
                    ui.add(egui::Slider::new(&mut self.prefs.bitrate, 2..=20));
                });
                egui::ComboBox::from_id_salt("encoder_quick")
                    .selected_text(&self.prefs.encoder)
                    .show_ui(ui, |ui| {
                        for (value, label) in [
                            ("auto", lang.text("Tự động", "Automatic")),
                            ("qsv", "Intel Quick Sync"),
                            ("cpu", "CPU"),
                        ] {
                            ui.selectable_value(&mut self.prefs.encoder, value.into(), label);
                        }
                    });
                ui.checkbox(
                    &mut self.prefs.cursor_highlight,
                    lang.text("Nổi con trỏ", "Cursor highlight"),
                );
                ui.checkbox(
                    &mut self.prefs.click_effect,
                    lang.text("Hiệu ứng nhấp", "Click effect"),
                );
                ui.add(
                    egui::Slider::new(&mut self.prefs.mic_volume, 0.0..=2.0)
                        .text(lang.text("Mức micro", "Mic gain")),
                );
                ui.add(
                    egui::Slider::new(&mut self.prefs.system_volume, 0.0..=2.0)
                        .text(lang.text("Mức tiếng máy", "System gain")),
                );
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(
                    &mut self.prefs.edit_after_recording,
                    false,
                    lang.text("Quay nhanh", "Quick recording"),
                );
                ui.selectable_value(
                    &mut self.prefs.edit_after_recording,
                    true,
                    lang.text("Quay rồi chỉnh sửa", "Record & edit"),
                );
            });
            ui.label(
                RichText::new(if self.prefs.edit_after_recording {
                    lang.text(
                        "Lưu xong sẽ mở video trong editor.",
                        "Opens the video in the editor after saving.",
                    )
                } else {
                    lang.text(
                        "Dừng quay để xem kết quả và chọn bước tiếp theo.",
                        "Stop to see the result and choose what to do next.",
                    )
                })
                .small()
                .color(MUTED),
            );
        });
    }
    fn result_panel(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.last_file.clone() else {
            return;
        };
        let lang = self.prefs.language;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lang.text("Đã lưu video", "Video saved"))
                        .strong()
                        .color(ACCENT),
                );
                if ui
                    .button(lang.text("Quay tiếp", "Record another"))
                    .clicked()
                {
                    self.show_result = false;
                }
            });
            if self.library.recording_result(ui, &path, lang) {
                self.editor.open(path.clone());
                self.tab = 2;
            }
            self.editor.show_error(ui, lang);
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
                    self.auto_edit_pending = false;
                    self.controller = Some(recorder::recover_async(folder));
                    self.phase = Phase::Finishing;
                }
            }
        });
    }
    fn library(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = self.error.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::LIGHT_RED, error);
                if ui
                    .small_button(self.prefs.language.text("Đóng", "Dismiss"))
                    .clicked()
                {
                    self.error = None;
                }
            });
        }
        match self
            .library
            .ui(ui, self.prefs.language, self.phase == Phase::Ready)
        {
            Some(crate::library_ui::Action::Edit(path)) => {
                self.editor.open(path);
                self.tab = 2;
            }
            Some(crate::library_ui::Action::Recover(path)) => {
                self.controller = Some(recorder::recover_async(path));
                self.phase = Phase::Finishing;
            }
            None => {}
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.editor.set_language(self.prefs.language);
        self.tick(ctx);
        self.qa_tick(ctx);
        self.library.tick(&self.prefs.output, ctx);
        // Opt-in framebuffer capture for repeatable visual QA of this window.
        // WGC cannot capture the recorder's normal capture-excluded window.
        if std::env::args().any(|a| a == "--inspect-ui") {
            if let Some(file) = std::env::var_os("SCREENRECORDER_UI_CAPTURE") {
                for event in ctx.input(|i| i.events.clone()) {
                    if let egui::Event::Screenshot { image, .. } = event {
                        let mut file = PathBuf::from(&file);
                        if std::env::args().any(|a| a == "--ui-smoke") {
                            let label = if self.error.is_some() {
                                "Error".into()
                            } else if self.show_result {
                                "Result".into()
                            } else {
                                format!("{:?}", self.phase)
                            };
                            file.set_file_name(format!("{label}.png"));
                        }
                        std::thread::spawn(move || {
                            let bytes = image
                                .pixels
                                .iter()
                                .flat_map(|p| p.to_array())
                                .collect::<Vec<_>>();
                            let _ = image::save_buffer(
                                file,
                                &bytes,
                                image.size[0] as u32,
                                image.size[1] as u32,
                                image::ColorType::Rgba8,
                            );
                        });
                    }
                }
                if self.qa_capture_at.elapsed() > Duration::from_secs(2) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                    self.qa_capture_at = Instant::now();
                }
            }
        }
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
                    if ui.available_width() > 900. {
                        ui.label(RichText::new("Screen Recorder").size(19.).strong());
                        ui.add_space(16.);
                    }
                    let lang = self.prefs.language;
                    ui.selectable_value(&mut self.tab, 0, lang.text("Quay", "Record"));
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
                        if ui.available_width() > 180. {
                            ui.label(
                                RichText::new(concat!("STUDIO  /  ", env!("CARGO_PKG_VERSION")))
                                    .size(11.)
                                    .color(MUTED),
                            );
                        }
                    });
                });
            });
        if self.tab == 0 || self.phase != Phase::Ready {
            egui::TopBottomPanel::bottom("controls")
                .frame(
                    egui::Frame::new()
                        .fill(PANEL)
                        .inner_margin(egui::Margin::symmetric(20, 8)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let t = self.seconds as u64;
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{:02}:{:02}:{:02}",
                                    t / 3600,
                                    t / 60 % 60,
                                    t % 60
                                ))
                                .size(20.)
                                .monospace(),
                            );
                            ui.label(
                                RichText::new(match self.phase {
                                    Phase::Ready => {
                                        if self.error.is_some() {
                                            lang.text(
                                                "Có lỗi · kiểm tra thông báo",
                                                "Error · check the message",
                                            )
                                        } else {
                                            lang.text("Sẵn sàng quay", "Ready to record")
                                        }
                                    }
                                    Phase::Recording => lang.text("Đang ghi hình", "Recording"),
                                    Phase::Paused => lang.text("Đã tạm dừng", "Paused"),
                                    Phase::Countdown => lang.text("Đếm ngược", "Countdown"),
                                    Phase::Starting => lang.text("Đang chuẩn bị…", "Preparing…"),
                                    _ => lang.text("Đang hoàn tất video…", "Finalizing video…"),
                                })
                                .small()
                                .color(MUTED),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            match self.phase {
                                Phase::Ready => {
                                    if ui
                                        .add_sized(
                                            [175., 44.],
                                            egui::Button::new(
                                                RichText::new(lang.text("●  Quay", "●  Record"))
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            )
                                            .fill(RED)
                                            .corner_radius(8),
                                        )
                                        .on_hover_text("Ctrl + Shift + F9")
                                        .clicked()
                                    {
                                        self.begin();
                                    }
                                    if ui
                                        .button(lang.text("Chụp ảnh", "Screenshot"))
                                        .on_hover_text(lang.text(
                                            "Lưu PNG của vùng đang chọn",
                                            "Save the selected region as PNG",
                                        ))
                                        .clicked()
                                    {
                                        self.screenshot();
                                    }
                                }
                                Phase::Countdown => {
                                    if ui.button(lang.text("Hủy", "Cancel")).clicked() {
                                        self.stop();
                                    }
                                    ui.label(format!(
                                        "{} {}…",
                                        lang.text("Bắt đầu sau", "Starting in"),
                                        3 - self.countdown.unwrap().elapsed().as_secs().min(3)
                                    ));
                                }
                                Phase::Recording | Phase::Paused => {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                lang.text("■  Dừng & lưu", "■  Stop & save"),
                                            )
                                            .fill(RED),
                                        )
                                        .clicked()
                                    {
                                        self.stop();
                                    }
                                    if ui
                                        .button(if self.phase == Phase::Paused {
                                            lang.text("Tiếp tục", "Resume")
                                        } else {
                                            lang.text("Tạm dừng", "Pause")
                                        })
                                        .clicked()
                                    {
                                        self.pause_resume(ctx);
                                    }
                                    if ui
                                        .button(lang.text("Toolbar nổi", "Floating toolbar"))
                                        .on_hover_text(lang.text(
                                            "Hiện thanh điều khiển không ghi vào video",
                                            "Show controls excluded from the video",
                                        ))
                                        .clicked()
                                    {
                                        self.show_overlay();
                                    }
                                }
                                _ => {
                                    ui.spinner();
                                }
                            }
                        });
                    });
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {} fps",
                            if self.prefs.edit_after_recording {
                                lang.text("Quay rồi chỉnh sửa", "Record & edit")
                            } else {
                                lang.text("Quay nhanh", "Quick recording")
                            },
                            if self.prefs.max_height == 0 {
                                lang.text("Kích thước gốc", "Original size").into()
                            } else {
                                format!("≤ {}p", self.prefs.max_height)
                            },
                            self.prefs.fps
                        ))
                        .small()
                        .color(ACCENT),
                    );
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
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(lang.text("Mở", "Open")).clicked() {
                                open(&self.prefs.output);
                            }
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
                            ui.add_sized(
                                [ui.available_width().max(20.), 22.],
                                egui::Label::new(
                                    RichText::new(self.prefs.output.display().to_string()).small(),
                                )
                                .truncate(),
                            )
                            .on_hover_text(self.prefs.output.display().to_string());
                        });
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
                if self.tab == 1 {
                    self.library(ui);
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt(("workspace", self.tab, self.show_result))
                    .show(ui, |ui| {
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
                                        if ui.small_button(lang.text("Đóng", "Dismiss")).clicked()
                                        {
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
                                if self.show_result {
                                    self.result_panel(ui);
                                }
                                ui.add_enabled_ui(
                                    self.phase == Phase::Ready && self.picker.is_none(),
                                    |ui| {
                                        let width = ui.available_width();
                                        if width < 680. {
                                            self.source_panel(ui, ctx);
                                            self.audio_panel(ui);
                                            self.quality_panel(ui);
                                            return;
                                        }
                                        let left = (width - 16.) * 0.52;
                                        ui.horizontal_top(|ui| {
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(left, 360.),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    self.source_panel(ui, ctx);
                                                },
                                            );
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(width - left - 16., 360.),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    self.audio_panel(ui);
                                                    self.quality_panel(ui);
                                                },
                                            );
                                        });
                                    },
                                );
                            }
                            1 => self.library(ui),
                            2 => {}
                            _ => {
                                ui.add_enabled_ui(self.phase == Phase::Ready, |ui| {
                                    self.settings(ui)
                                });
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

fn source_tile(
    ui: &mut egui::Ui,
    title: &str,
    area: Area,
    selected: bool,
    texture: Option<&egui::TextureHandle>,
    lang: Language,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 70.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            title,
        )
    });
    ui.painter().rect_filled(
        rect,
        6,
        if selected {
            Color32::from_rgb(40, 86, 110)
        } else {
            Color32::from_rgb(14, 23, 35)
        },
    );
    if response.has_focus() || response.hovered() {
        ui.painter().rect_stroke(
            rect,
            6,
            egui::Stroke::new(2., ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let thumb = egui::Rect::from_min_size(rect.min + egui::vec2(8., 8.), egui::vec2(96., 54.));
    ui.painter()
        .rect_filled(thumb, 4, Color32::from_rgb(9, 15, 23));
    if let Some(t) = texture {
        let scale = (thumb.width() / t.size_vec2().x).min(thumb.height() / t.size_vec2().y);
        ui.painter().image(
            t.id(),
            egui::Rect::from_center_size(thumb.center(), t.size_vec2() * scale),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            thumb.center(),
            egui::Align2::CENTER_CENTER,
            lang.text("Xem trước…", "Preview…"),
            FontId::proportional(11.),
            MUTED,
        );
    }
    let text_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(116., 8.),
        rect.max - egui::vec2(8., 4.),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.add(egui::Label::new(RichText::new(title).strong()).truncate());
    child.label(
        RichText::new(format!(
            "{} × {} · {}",
            area.width,
            area.height,
            if selected {
                lang.text("Đã chọn", "Selected")
            } else {
                lang.text("Chọn nguồn này", "Select source")
            }
        ))
        .small()
        .color(if selected { INK } else { MUTED }),
    );
    response.on_hover_text(title)
}
