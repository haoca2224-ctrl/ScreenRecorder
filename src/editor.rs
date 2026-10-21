use crate::{model::Language, recorder};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use serde::Deserialize;
use std::{
    io::{BufRead, BufReader},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MUTED: Color32 = Color32::from_rgb(157, 174, 195);
const PANEL: Color32 = Color32::from_rgb(23, 32, 46);
const ACCENT: Color32 = Color32::from_rgb(111, 198, 237);
const RED: Color32 = Color32::from_rgb(218, 62, 78);

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextMotion {
    None,
    Fade,
    SlideUp,
    Pop,
    Typewriter,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextStylePreset {
    Clean,
    Bold,
    Outline,
    Neon,
    Label,
    Shadow,
}

impl TextStylePreset {
    const ALL: [Self; 6] = [
        Self::Clean,
        Self::Bold,
        Self::Outline,
        Self::Neon,
        Self::Label,
        Self::Shadow,
    ];

    fn label(self, lang: Language) -> &'static str {
        match self {
            Self::Clean => lang.text("Tối giản", "Clean"),
            Self::Bold => lang.text("Đậm", "Bold"),
            Self::Outline => lang.text("Viền", "Outline"),
            Self::Neon => "Neon",
            Self::Label => lang.text("Nhãn", "Label"),
            Self::Shadow => lang.text("Bóng", "Shadow"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextInspectorTab {
    Basic,
    Animation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnimationPhase {
    In,
    Out,
}

impl TextMotion {
    const ALL: [Self; 5] = [
        Self::None,
        Self::Fade,
        Self::SlideUp,
        Self::Pop,
        Self::Typewriter,
    ];
    fn label(self, lang: Language) -> &'static str {
        match self {
            Self::None => lang.text("Tĩnh", "Static"),
            Self::Fade => "Fade",
            Self::SlideUp => "Slide Up",
            Self::Pop => "Pop",
            Self::Typewriter => lang.text("Gõ chữ", "Typewriter"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VideoMotion {
    Clean,
    Fade,
    SlowZoom,
}

impl VideoMotion {
    const ALL: [Self; 3] = [Self::Clean, Self::Fade, Self::SlowZoom];
    fn label(self, lang: Language) -> &'static str {
        match self {
            Self::Clean => lang.text("Không hiệu ứng", "No effect"),
            Self::Fade => "Soft Fade",
            Self::SlowZoom => lang.text("Zoom chậm", "Slow Zoom"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputSize {
    Original,
    Hd,
    FullHd,
    Square,
    Vertical,
}

impl OutputSize {
    const ALL: [Self; 5] = [
        Self::Original,
        Self::Hd,
        Self::FullHd,
        Self::Square,
        Self::Vertical,
    ];
    fn label(self, lang: Language) -> &'static str {
        match self {
            Self::Original => lang.text("Kích thước gốc", "Original"),
            Self::Hd => "HD · 1280×720",
            Self::FullHd => "Full HD · 1920×1080",
            Self::Square => "Square · 1080×1080",
            Self::Vertical => "Vertical · 1080×1920",
        }
    }
    fn dimensions(self, source: (u32, u32), crop: Crop) -> (u32, u32) {
        match self {
            Self::Original => (
                ((source.0 as f32 * crop.w).round() as u32).max(16) & !1,
                ((source.1 as f32 * crop.h).round() as u32).max(16) & !1,
            ),
            Self::Hd => (1280, 720),
            Self::FullHd => (1920, 1080),
            Self::Square => (1080, 1080),
            Self::Vertical => (1080, 1920),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Mp4,
    Mkv,
    Webm,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorPanel {
    Media,
    Text,
    Effects,
}

impl OutputFormat {
    const ALL: [Self; 3] = [Self::Mp4, Self::Mkv, Self::Webm];
    fn label(self) -> &'static str {
        match self {
            Self::Mp4 => "MP4 · H.264",
            Self::Mkv => "MKV · H.264",
            Self::Webm => "WebM · VP9",
        }
    }
    fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Crop {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        }
    }
}

#[derive(Clone, PartialEq)]
struct TextLayer {
    text: String,
    x: f32,
    y: f32,
    size: f32,
    start: f64,
    end: f64,
    style: TextStylePreset,
    motion_in: TextMotion,
    motion_out: TextMotion,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ClipSegment {
    source_start: f64,
    source_end: f64,
    speed: f32,
}

impl ClipSegment {
    fn source_duration(self) -> f64 {
        (self.source_end - self.source_start).max(0.0)
    }

    fn duration(self) -> f64 {
        self.source_duration() / self.speed.max(0.05) as f64
    }
}

#[derive(Clone, PartialEq)]
struct EditorSnapshot {
    crop: Crop,
    layers: Vec<TextLayer>,
    segments: Vec<ClipSegment>,
    video_motion: VideoMotion,
}

impl Default for TextLayer {
    fn default() -> Self {
        Self {
            text: "Your text".into(),
            x: 0.08,
            y: 0.82,
            size: 48.0,
            start: 0.0,
            end: 5.0,
            style: TextStylePreset::Clean,
            motion_in: TextMotion::Fade,
            motion_out: TextMotion::None,
        }
    }
}

struct PreviewFrame {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
}

#[derive(Clone, Copy)]
struct MediaInfo {
    width: u32,
    height: u32,
    duration: f64,
    has_audio: bool,
}

enum ExportEvent {
    Progress(f32),
    Done(PathBuf),
    Failed(String),
}

pub struct EditorState {
    input: Option<PathBuf>,
    info: Option<MediaInfo>,
    probe_rx: Option<Receiver<Result<MediaInfo, String>>>,
    waveform_rx: Option<Receiver<Vec<f32>>>,
    waveform: Vec<f32>,
    preview_rx: Option<Receiver<Result<PreviewFrame, String>>>,
    preview_texture: Option<egui::TextureHandle>,
    preview_requested: f64,
    preview_time: f64,
    preview_at: Instant,
    trim_start: f64,
    trim_end: f64,
    crop: Crop,
    crop_drag: Option<(f32, f32)>,
    text_drag_origin: Option<(f32, f32)>,
    text_scale_origin: Option<f32>,
    snap_guides: (Option<f32>, Option<f32>),
    snap_enabled: bool,
    layers: Vec<TextLayer>,
    selected_layer: usize,
    video_motion: VideoMotion,
    output_size: OutputSize,
    output_format: OutputFormat,
    export_rx: Option<Receiver<ExportEvent>>,
    export_progress: f32,
    export_error: Option<String>,
    last_output: Option<PathBuf>,
    panel: EditorPanel,
    text_tab: TextInspectorTab,
    animation_phase: AnimationPhase,
    segments: Vec<ClipSegment>,
    selected_segment: usize,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    browser_open: bool,
    inspector_open: bool,
    timeline_open: bool,
    playing: bool,
    play_tick: Instant,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            input: None,
            info: None,
            probe_rx: None,
            waveform_rx: None,
            waveform: Vec::new(),
            preview_rx: None,
            preview_texture: None,
            preview_requested: -1.0,
            preview_time: 0.0,
            preview_at: Instant::now() - Duration::from_secs(1),
            trim_start: 0.0,
            trim_end: 0.0,
            crop: Crop::default(),
            crop_drag: None,
            text_drag_origin: None,
            text_scale_origin: None,
            snap_guides: (None, None),
            snap_enabled: true,
            layers: Vec::new(),
            selected_layer: 0,
            video_motion: VideoMotion::Clean,
            output_size: OutputSize::FullHd,
            output_format: OutputFormat::Mp4,
            export_rx: None,
            export_progress: 0.0,
            export_error: None,
            last_output: None,
            panel: EditorPanel::Media,
            text_tab: TextInspectorTab::Basic,
            animation_phase: AnimationPhase::In,
            segments: Vec::new(),
            selected_segment: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            browser_open: true,
            inspector_open: true,
            timeline_open: true,
            playing: false,
            play_tick: Instant::now(),
        }
    }
}

impl EditorState {
    pub fn open(&mut self, path: PathBuf) {
        self.input = Some(path.clone());
        self.info = None;
        self.preview_texture = None;
        self.preview_rx = None;
        self.preview_requested = -1.0;
        self.preview_time = 0.0;
        self.trim_start = 0.0;
        self.trim_end = 0.0;
        self.crop = Crop::default();
        self.layers.clear();
        self.segments.clear();
        self.selected_segment = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.waveform.clear();
        self.export_error = None;
        self.playing = false;
        let (tx, rx) = mpsc::channel();
        let probe_path = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(probe(&probe_path).map_err(|e| format!("{e:#}")));
        });
        self.probe_rx = Some(rx);
        let (wave_tx, wave_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = wave_tx.send(waveform(&path).unwrap_or_default());
        });
        self.waveform_rx = Some(wave_rx);
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            crop: self.crop,
            layers: self.layers.clone(),
            segments: self.segments.clone(),
            video_motion: self.video_motion,
        }
    }

    fn record_previous(&mut self, previous: EditorSnapshot) {
        if previous == self.snapshot() {
            return;
        }
        if self.undo_stack.last() != Some(&previous) {
            self.undo_stack.push(previous);
            if self.undo_stack.len() > 64 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
    }

    fn checkpoint(&mut self) {
        let snapshot = self.snapshot();
        if self.undo_stack.last() != Some(&snapshot) {
            self.undo_stack.push(snapshot);
            if self.undo_stack.len() > 64 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
    }

    fn apply_snapshot(&mut self, snapshot: EditorSnapshot) {
        self.crop = snapshot.crop;
        self.layers = snapshot.layers;
        self.segments = snapshot.segments;
        self.video_motion = snapshot.video_motion;
        self.selected_layer = self.selected_layer.min(self.layers.len().saturating_sub(1));
        self.selected_segment = self
            .selected_segment
            .min(self.segments.len().saturating_sub(1));
        self.sync_project_duration();
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.apply_snapshot(previous);
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.apply_snapshot(next);
        }
    }

    fn project_duration(&self) -> f64 {
        self.segments.iter().map(|segment| segment.duration()).sum()
    }

    fn segment_at(&self, time: f64) -> Option<(usize, f64, f64)> {
        let mut project_start = 0.0;
        for (index, segment) in self.segments.iter().copied().enumerate() {
            let project_end = project_start + segment.duration();
            if time <= project_end + 0.000_1 || index + 1 == self.segments.len() {
                let local = (time - project_start).clamp(0.0, segment.duration());
                let source = segment.source_start + local * segment.speed as f64;
                return Some((index, project_start, source.min(segment.source_end)));
            }
            project_start = project_end;
        }
        None
    }

    fn project_to_source(&self, time: f64) -> f64 {
        self.segment_at(time)
            .map(|(_, _, source)| source)
            .unwrap_or(0.0)
    }

    fn sync_project_duration(&mut self) {
        self.trim_start = 0.0;
        self.trim_end = self.project_duration();
        self.preview_time = self.preview_time.clamp(0.0, self.trim_end.max(0.0));
        for layer in &mut self.layers {
            layer.start = layer.start.clamp(0.0, self.trim_end);
            layer.end = layer.end.clamp(layer.start, self.trim_end);
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if self.playing {
            let delta = now.saturating_duration_since(self.play_tick).as_secs_f64();
            self.preview_time += delta;
            if self.preview_time >= self.trim_end {
                self.preview_time = self.trim_end;
                self.playing = false;
            }
            ctx.request_repaint();
        }
        self.play_tick = now;
        if let Some(result) = self.probe_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.probe_rx = None;
            match result {
                Ok(info) => {
                    self.info = Some(info);
                    self.trim_end = info.duration;
                    self.preview_time = 0.0;
                    self.segments = vec![ClipSegment {
                        source_start: 0.0,
                        source_end: info.duration,
                        speed: 1.0,
                    }];
                }
                Err(error) => self.export_error = Some(error),
            }
        }
        if let Some(waveform) = self.waveform_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.waveform = waveform;
            self.waveform_rx = None;
        }
        if let Some(result) = self.preview_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.preview_rx = None;
            match result {
                Ok(frame) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [frame.width, frame.height],
                        &frame.rgba,
                    );
                    if let Some(texture) = &mut self.preview_texture {
                        texture.set(image, egui::TextureOptions::LINEAR);
                    } else {
                        self.preview_texture = Some(ctx.load_texture(
                            "editor_preview",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                Err(error) => self.export_error = Some(error),
            }
        }
        if let Some(event) = self.export_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            match event {
                ExportEvent::Progress(progress) => self.export_progress = progress,
                ExportEvent::Done(path) => {
                    self.last_output = Some(path);
                    self.export_progress = 1.0;
                    self.export_rx = None;
                }
                ExportEvent::Failed(error) => {
                    self.export_error = Some(error);
                    self.export_rx = None;
                }
            }
        }
        if self.info.is_some()
            && self.preview_rx.is_none()
            && (self.preview_time - self.preview_requested).abs() > 0.04
            && self.preview_at.elapsed() > Duration::from_millis(180)
        {
            self.request_preview();
        }
    }

    fn request_preview(&mut self) {
        let Some(path) = self.input.clone() else {
            return;
        };
        let project_at = self.preview_time;
        let at = self.project_to_source(project_at);
        self.preview_requested = project_at;
        self.preview_at = Instant::now();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(preview(&path, at).map_err(|e| format!("{e:#}")));
        });
        self.preview_rx = Some(rx);
    }

    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        output_folder: &Path,
        lang: Language,
    ) {
        self.tick(ctx);
        let undo = ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::Z));
        let redo = ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::CTRL, egui::Key::Y)
                || input.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
        });
        if undo {
            self.undo();
        } else if redo {
            self.redo();
        }
        egui::Frame::new()
            .fill(Color32::from_rgb(18, 25, 36))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(lang.text("Studio chỉnh sửa", "Editing studio"));
                    ui.label(RichText::new("0.6").small().strong().color(ACCENT));
                    if let Some(path) = &self.input {
                        ui.separator();
                        ui.add_sized(
                            [260.0, 24.0],
                            egui::Label::new(
                                RichText::new(
                                    path.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .small()
                                .color(MUTED),
                            )
                            .truncate(),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let running = self.export_rx.is_some();
                        if ui
                            .add_enabled(
                                !running && self.info.is_some() && self.trim_end > self.trim_start,
                                egui::Button::new(
                                    RichText::new(lang.text("Xuất video", "Export"))
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(31, 164, 180)),
                            )
                            .on_hover_text(lang.text(
                                "Xuất theo cài đặt ở bảng bên phải",
                                "Export using the settings on the right",
                            ))
                            .clicked()
                        {
                            self.start_export(output_folder);
                        }
                        if ui
                            .button(lang.text("Nhập media…", "Import media…"))
                            .on_hover_text(
                                lang.text(
                                    "Mở MP4, MKV, WebM hoặc MOV",
                                    "Open MP4, MKV, WebM or MOV",
                                ),
                            )
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Video", &["mp4", "mkv", "webm", "mov"])
                                .pick_file()
                            {
                                self.open(path);
                            }
                        }
                        ui.separator();
                        if ui
                            .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("↷"))
                            .on_hover_text(lang.text(
                                "Làm lại thao tác Editor (Ctrl+Y)",
                                "Redo editor action (Ctrl+Y)",
                            ))
                            .clicked()
                        {
                            self.redo();
                        }
                        if ui
                            .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("↶"))
                            .on_hover_text(lang.text(
                                "Hoàn tác thao tác Editor (Ctrl+Z)",
                                "Undo editor action (Ctrl+Z)",
                            ))
                            .clicked()
                        {
                            self.undo();
                        }
                        ui.separator();
                        ui.toggle_value(&mut self.timeline_open, "▤")
                            .on_hover_text(lang.text("Hiện / ẩn timeline", "Show / hide timeline"));
                        ui.toggle_value(&mut self.inspector_open, "▥")
                            .on_hover_text(
                                lang.text("Hiện / ẩn thuộc tính", "Show / hide inspector"),
                            );
                        ui.toggle_value(&mut self.browser_open, "▧")
                            .on_hover_text(lang.text("Hiện / ẩn thư viện", "Show / hide browser"));
                    });
                });
            });
        ui.add_space(6.0);
        if self.input.is_none() {
            empty_editor(ui, lang);
            return;
        }
        if self.info.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(lang.text("Đang đọc thông tin video…", "Reading video metadata…"));
            });
            return;
        }
        let width = ui.available_width();
        let height = ui.available_height();
        let compact = width < 1120.0 || height < 610.0;
        let timeline_height = if self.timeline_open {
            if compact {
                (height * 0.30).clamp(118.0, 148.0)
            } else {
                (height * 0.34).clamp(150.0, 190.0)
            }
        } else {
            0.0
        };
        let workspace_height =
            (height - timeline_height - if self.timeline_open { 8.0 } else { 0.0 }).max(220.0);
        let browser_width = if self.browser_open {
            if compact {
                186.0
            } else {
                224.0
            }
        } else {
            0.0
        };
        let inspector_width = if self.inspector_open {
            if compact {
                230.0
            } else {
                274.0
            }
        } else {
            0.0
        };
        let gaps = ((self.browser_open as u8 + self.inspector_open as u8) as f32) * 8.0;
        let player_width = (width - browser_width - inspector_width - gaps).max(260.0);
        ui.horizontal_top(|ui| {
            if self.browser_open {
                ui.allocate_ui_with_layout(
                    Vec2::new(browser_width, workspace_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.browser_ui(ui, lang),
                );
            }
            ui.allocate_ui_with_layout(
                Vec2::new(player_width, workspace_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| panel(ui, |ui| self.preview_ui(ui, lang)),
            );
            if self.inspector_open {
                ui.allocate_ui_with_layout(
                    Vec2::new(inspector_width, workspace_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.inspector_ui(ui, output_folder, lang),
                );
            }
        });
        if self.timeline_open {
            ui.allocate_ui_with_layout(
                Vec2::new(width, timeline_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| panel(ui, |ui| self.timeline_ui(ui, lang)),
            );
        }
        if let Some(error) = self.export_error.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(error).color(Color32::from_rgb(255, 170, 180)));
                if ui.small_button("×").clicked() {
                    self.export_error = None;
                }
            });
        }
        if let Some(path) = self.last_output.clone() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(lang.text("Đã xuất:", "Exported:")).color(ACCENT));
                ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                if ui
                    .small_button(lang.text("Mở video", "Open video"))
                    .clicked()
                {
                    let _ = Command::new("explorer.exe").arg(&path).spawn();
                }
            });
        }
    }

    fn browser_ui(&mut self, ui: &mut egui::Ui, lang: Language) {
        panel(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.panel,
                    EditorPanel::Media,
                    lang.text("Media", "Media"),
                )
                .on_hover_text(lang.text("Nguồn và crop", "Source and crop"));
                ui.selectable_value(&mut self.panel, EditorPanel::Text, lang.text("Chữ", "Text"))
                    .on_hover_text(lang.text("Tiêu đề và chú thích", "Titles and captions"));
                ui.selectable_value(
                    &mut self.panel,
                    EditorPanel::Effects,
                    lang.text("Hiệu ứng", "Effects"),
                )
                .on_hover_text(lang.text("Preset chuyển động", "Motion presets"));
            });
        });
        egui::ScrollArea::vertical()
            .id_salt("editor_browser_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                panel(ui, |ui| match self.panel {
                    EditorPanel::Media => {
                        ui.label(
                            RichText::new(lang.text("MEDIA ĐÃ NHẬP", "IMPORTED MEDIA"))
                                .small()
                                .strong()
                                .color(MUTED),
                        );
                        if let (Some(path), Some(info)) = (&self.input, self.info) {
                            egui::Frame::new()
                                .fill(Color32::from_rgb(31, 43, 59))
                                .corner_radius(7)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(
                                                path.file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy(),
                                            )
                                            .strong(),
                                        )
                                        .truncate(),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{}×{}  ·  {}",
                                            info.width,
                                            info.height,
                                            format_time(info.duration)
                                        ))
                                        .small()
                                        .color(MUTED),
                                    );
                                });
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(lang.text("KHUNG HÌNH", "CROP RATIO"))
                                .small()
                                .strong()
                                .color(MUTED),
                        );
                        for (label, ratio) in [
                            (lang.text("Gốc", "Original"), 0.0),
                            ("16:9", 16.0 / 9.0),
                            ("9:16", 9.0 / 16.0),
                            ("1:1", 1.0),
                            ("4:3", 4.0 / 3.0),
                        ] {
                            if ui
                                .add_sized([ui.available_width(), 30.0], egui::Button::new(label))
                                .clicked()
                            {
                                self.set_crop_aspect(ratio);
                            }
                        }
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(lang.text(
                                "Có thể kéo trực tiếp trên Player để chọn vùng tự do.",
                                "Drag directly on the Player for a free crop.",
                            ))
                            .small()
                            .color(MUTED),
                        );
                    }
                    EditorPanel::Text => {
                        ui.label(
                            RichText::new(lang.text("MẪU CHỮ", "TEXT PRESETS"))
                                .small()
                                .strong()
                                .color(MUTED),
                        );
                        for row in TextStylePreset::ALL.chunks(2) {
                            ui.columns(2, |columns| {
                                for (column, style) in row.iter().copied().enumerate() {
                                    let label = format!("Aa\n{}", style.label(lang));
                                    if columns[column]
                                        .add_sized(
                                            [columns[column].available_width(), 58.0],
                                            egui::Button::new(RichText::new(label).strong()),
                                        )
                                        .on_hover_text(lang.text(
                                            "Thêm preset chữ vào vị trí playhead",
                                            "Add this text preset at the playhead",
                                        ))
                                        .clicked()
                                    {
                                        self.add_text(style);
                                    }
                                }
                            });
                        }
                        ui.label(
                    RichText::new(lang.text(
                        "Chọn chữ trên Player để kéo, dùng chấm góc để đổi cỡ.",
                        "Select text in the Player to move it; drag the corner dot to scale.",
                    ))
                    .small()
                    .color(MUTED),
                );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!(
                                "{} ({})",
                                lang.text("LỚP TRÊN TIMELINE", "TIMELINE LAYERS"),
                                self.layers.len()
                            ))
                            .small()
                            .strong()
                            .color(MUTED),
                        );
                        for (index, layer) in self.layers.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    self.selected_layer == index,
                                    format!("T{}  {}", index + 1, layer.text),
                                )
                                .clicked()
                            {
                                self.selected_layer = index;
                            }
                        }
                    }
                    EditorPanel::Effects => {
                        ui.label(
                            RichText::new(lang.text("PRESET VIDEO", "VIDEO PRESETS"))
                                .small()
                                .strong()
                                .color(MUTED),
                        );
                        for motion in VideoMotion::ALL {
                            let selected = self.video_motion == motion;
                            if ui
                                .add_sized(
                                    [ui.available_width(), 54.0],
                                    egui::Button::new(
                                        RichText::new(motion.label(lang))
                                            .strong()
                                            .color(if selected { ACCENT } else { Color32::WHITE }),
                                    )
                                    .fill(if selected {
                                        Color32::from_rgb(35, 70, 83)
                                    } else {
                                        Color32::from_rgb(31, 43, 59)
                                    }),
                                )
                                .clicked()
                            {
                                self.checkpoint();
                                self.video_motion = motion;
                            }
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(lang.text(
                                "Preset áp dụng cho toàn bộ đoạn đã cắt.",
                                "The preset applies to the complete trimmed clip.",
                            ))
                            .small()
                            .color(MUTED),
                        );
                    }
                })
            });
    }

    fn add_text(&mut self, style: TextStylePreset) {
        self.checkpoint();
        let layer = TextLayer {
            style,
            start: self.preview_time.clamp(self.trim_start, self.trim_end),
            end: self
                .trim_end
                .min(self.preview_time + 5.0)
                .max(self.preview_time + 0.1),
            ..Default::default()
        };
        self.layers.push(layer);
        self.selected_layer = self.layers.len() - 1;
        self.panel = EditorPanel::Text;
        self.text_tab = TextInspectorTab::Basic;
    }

    fn set_crop_aspect(&mut self, ratio: f32) {
        let Some(info) = self.info else {
            return;
        };
        self.checkpoint();
        if ratio <= 0.0 {
            self.crop = Crop::default();
            return;
        }
        let source = info.width as f32 / info.height.max(1) as f32;
        self.crop = if source > ratio {
            let width = ratio / source;
            Crop {
                x: (1.0 - width) / 2.0,
                y: 0.0,
                w: width,
                h: 1.0,
            }
        } else {
            let height = source / ratio;
            Crop {
                x: 0.0,
                y: (1.0 - height) / 2.0,
                w: 1.0,
                h: height,
            }
        };
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, lang: Language) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(lang.text("PLAYER", "PLAYER"))
                    .small()
                    .strong()
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.snap_enabled, lang.text("Bắt dính", "Snap"))
                    .on_hover_text(lang.text(
                        "Bắt chữ vào tâm và vùng an toàn 5%",
                        "Snap text to center and the 5% safe zone",
                    ));
                if ui
                    .small_button(lang.text("Đặt lại crop", "Reset crop"))
                    .clicked()
                {
                    self.checkpoint();
                    self.crop = Crop::default();
                }
                if let Some(info) = self.info {
                    ui.label(
                        RichText::new(format!("{}×{}", info.width, info.height))
                            .small()
                            .color(MUTED),
                    );
                }
            });
        });
        let available = Vec2::new(
            ui.available_width(),
            (ui.available_height() - 48.0).max(120.0),
        );
        let (outer, _) = ui.allocate_exact_size(available, Sense::hover());
        ui.painter()
            .rect_filled(outer, 8, Color32::from_rgb(9, 13, 20));
        let Some(texture) = &self.preview_texture else {
            ui.painter().text(
                outer.center(),
                egui::Align2::CENTER_CENTER,
                lang.text("Đang tạo preview…", "Generating preview…"),
                FontId::proportional(14.0),
                MUTED,
            );
            return;
        };
        let size = texture.size_vec2();
        let scale = (outer.width() / size.x).min(outer.height() / size.y);
        let image = Rect::from_center_size(outer.center(), size * scale);
        let target = self
            .info
            .map(|info| {
                self.output_size
                    .dimensions((info.width, info.height), self.crop)
            })
            .unwrap_or((size.x as u32, size.y as u32));
        let text_scale = (image.width() / target.0.max(1) as f32)
            .min(image.height() / target.1.max(1) as f32)
            .max(0.01);
        ui.painter().image(
            texture.id(),
            image,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if self.panel == EditorPanel::Media {
            let response = ui
                .interact(image, ui.id().with("crop_canvas"), Sense::click_and_drag())
                .on_hover_text(lang.text(
                    "Kéo để chọn vùng giữ lại",
                    "Drag to select the area to keep",
                ));
            if response.drag_started() {
                if let Some(p) = response.interact_pointer_pos() {
                    self.checkpoint();
                    self.crop_drag = Some(norm_point(p, image));
                }
            }
            if response.dragged() {
                if let (Some(start), Some(p)) = (self.crop_drag, response.interact_pointer_pos()) {
                    let end = norm_point(p, image);
                    self.crop = crop_points(start, end);
                }
            }
            if response.drag_stopped() {
                self.crop_drag = None;
            }
        }
        let crop_rect = Rect::from_min_max(
            egui::pos2(
                image.left() + image.width() * self.crop.x,
                image.top() + image.height() * self.crop.y,
            ),
            egui::pos2(
                image.left() + image.width() * (self.crop.x + self.crop.w),
                image.top() + image.height() * (self.crop.y + self.crop.h),
            ),
        );
        if self.panel == EditorPanel::Media {
            shade_outside(ui, image, crop_rect);
            ui.painter().rect_stroke(
                crop_rect,
                2,
                Stroke::new(2.0, ACCENT),
                egui::StrokeKind::Inside,
            );
            for corner in [
                crop_rect.left_top(),
                crop_rect.right_top(),
                crop_rect.left_bottom(),
                crop_rect.right_bottom(),
            ] {
                ui.painter().rect_filled(
                    Rect::from_center_size(corner, Vec2::splat(8.0)),
                    2,
                    Color32::WHITE,
                );
            }
        }
        if self.panel == EditorPanel::Text {
            let safe = image.shrink2(Vec2::new(image.width() * 0.05, image.height() * 0.05));
            ui.painter().rect_stroke(
                safe,
                2,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(150, 174, 195, 100)),
                egui::StrokeKind::Inside,
            );
        }
        let visible_layers = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| self.preview_time >= layer.start && self.preview_time <= layer.end)
            .map(|(index, layer)| {
                let pos = egui::pos2(
                    image.left() + image.width() * layer.x,
                    image.top() + image.height() * layer.y,
                );
                let font_size = (layer.size * text_scale).clamp(8.0, 96.0);
                let width = (layer.text.chars().count().max(1) as f32 * font_size * 0.58)
                    .min(image.width() * 0.95);
                let rect = Rect::from_min_max(
                    egui::pos2(pos.x, pos.y - font_size * 1.25),
                    egui::pos2(pos.x + width, pos.y + 3.0),
                );
                (index, layer.text.clone(), layer.style, font_size, pos, rect)
            })
            .collect::<Vec<_>>();
        for (index, text_value, style, font_size, pos, rect) in visible_layers {
            paint_text_preset(ui.painter(), pos, &text_value, font_size, style, rect);
            if self.panel != EditorPanel::Text {
                continue;
            }
            let selected = self.selected_layer == index;
            let handle =
                Rect::from_center_size(rect.right_bottom() + Vec2::splat(12.0), Vec2::splat(16.0));
            let scale_response = if selected {
                Some(
                    ui.interact(
                        handle,
                        ui.id().with(("text_scale", index)),
                        Sense::click_and_drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::ResizeNwSe)
                    .on_hover_text(lang.text("Kéo để đổi cỡ chữ", "Drag to scale text")),
                )
            } else {
                None
            };
            let move_response = ui
                .interact(
                    rect.expand(4.0),
                    ui.id().with(("text_move", index)),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(lang.text("Kéo để di chuyển chữ", "Drag to move text"));
            if move_response.clicked() {
                self.selected_layer = index;
            }
            if move_response.drag_started() {
                self.checkpoint();
                self.selected_layer = index;
                self.text_drag_origin = Some((self.layers[index].x, self.layers[index].y));
            }
            if move_response.dragged() && self.selected_layer == index {
                if let Some((origin_x, origin_y)) = self.text_drag_origin {
                    let delta = move_response.drag_delta();
                    let width = (rect.width() / image.width()).clamp(0.01, 0.95);
                    let mut x = (origin_x + delta.x / image.width()).clamp(0.0, 1.0 - width);
                    let mut y = (origin_y + delta.y / image.height()).clamp(0.05, 0.98);
                    self.snap_guides = (None, None);
                    if self.snap_enabled {
                        let (snapped_x, guide_x) = snap_value(
                            x,
                            &[0.05, (0.5 - width * 0.5).max(0.0), (0.95 - width).max(0.0)],
                            9.0 / image.width().max(1.0),
                        );
                        let (snapped_y, guide_y) = snap_value(
                            y,
                            &[0.10 + font_size / image.height(), 0.5, 0.90],
                            9.0 / image.height().max(1.0),
                        );
                        x = snapped_x;
                        y = snapped_y;
                        self.snap_guides = (
                            guide_x.then_some(if (snapped_x - (0.5 - width * 0.5)).abs() < 0.001 {
                                0.5
                            } else if snapped_x > 0.5 {
                                snapped_x + width
                            } else {
                                snapped_x
                            }),
                            guide_y.then_some(snapped_y),
                        );
                    }
                    self.layers[index].x = x;
                    self.layers[index].y = y;
                }
            }
            if move_response.drag_stopped() {
                self.text_drag_origin = None;
                self.snap_guides = (None, None);
            }
            if selected {
                ui.painter().rect_stroke(
                    rect.expand(5.0),
                    3,
                    Stroke::new(1.5, ACCENT),
                    egui::StrokeKind::Outside,
                );
                ui.painter()
                    .circle_filled(handle.center(), 7.0, Color32::WHITE);
                ui.painter()
                    .circle_stroke(handle.center(), 7.0, Stroke::new(2.0, ACCENT));
                let scale_response = scale_response.unwrap();
                if scale_response.drag_started() {
                    self.checkpoint();
                    self.text_scale_origin = Some(self.layers[index].size);
                }
                if scale_response.dragged() {
                    if let Some(origin) = self.text_scale_origin {
                        let delta = scale_response.drag_delta();
                        self.layers[index].size =
                            (origin + (delta.x + delta.y) / (2.0 * text_scale)).clamp(18.0, 160.0);
                    }
                }
                if scale_response.drag_stopped() {
                    self.text_scale_origin = None;
                }
            }
        }
        if let Some(x) = self.snap_guides.0 {
            ui.painter().line_segment(
                [
                    egui::pos2(image.left() + image.width() * x, image.top()),
                    egui::pos2(image.left() + image.width() * x, image.bottom()),
                ],
                Stroke::new(1.5, ACCENT),
            );
        }
        if let Some(y) = self.snap_guides.1 {
            ui.painter().line_segment(
                [
                    egui::pos2(image.left(), image.top() + image.height() * y),
                    egui::pos2(image.right(), image.top() + image.height() * y),
                ],
                Stroke::new(1.5, ACCENT),
            );
        }
        ui.horizontal(|ui| {
            if ui
                .small_button("−1s")
                .on_hover_text(lang.text("Lùi một giây", "Back one second"))
                .clicked()
            {
                self.preview_time = (self.preview_time - 1.0).max(self.trim_start);
                self.playing = false;
            }
            if ui
                .add_sized(
                    [58.0, 28.0],
                    egui::Button::new(if self.playing { "❚❚" } else { "▶" }),
                )
                .on_hover_text(if self.playing {
                    lang.text("Tạm dừng preview", "Pause preview")
                } else {
                    lang.text("Phát preview", "Play preview")
                })
                .clicked()
            {
                if self.preview_time >= self.trim_end {
                    self.preview_time = self.trim_start;
                }
                self.playing = !self.playing;
                self.play_tick = Instant::now();
            }
            if ui
                .small_button("+1s")
                .on_hover_text(lang.text("Tiến một giây", "Forward one second"))
                .clicked()
            {
                self.preview_time = (self.preview_time + 1.0).min(self.trim_end);
                self.playing = false;
            }
            ui.label(
                RichText::new(format!(
                    "{}  /  {}",
                    format_time(self.preview_time),
                    format_time(self.info.map(|value| value.duration).unwrap_or(0.0))
                ))
                .monospace()
                .small()
                .color(MUTED),
            );
        });
    }

    fn timeline_ui(&mut self, ui: &mut egui::Ui, lang: Language) {
        let info = self.info.unwrap();
        let duration = self.project_duration().max(0.001);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(lang.text("TIMELINE", "TIMELINE"))
                    .small()
                    .strong()
                    .color(MUTED),
            );
            if ui
                .small_button(lang.text("✂ Tách", "✂ Split"))
                .on_hover_text(lang.text(
                    "Tách clip tại playhead nhưng giữ nguyên toàn bộ nội dung",
                    "Split the clip at the playhead while keeping all content",
                ))
                .clicked()
            {
                self.split_at_marker();
            }
            if ui
                .add_enabled(
                    self.segments.len() > 1,
                    egui::Button::new(lang.text("Xóa đoạn", "Delete clip")),
                )
                .on_hover_text(lang.text(
                    "Xóa đoạn đang chọn khỏi bản dựng",
                    "Remove the selected clip from the edit",
                ))
                .clicked()
            {
                self.delete_selected_segment();
            }
            if ui
                .add_enabled(self.selected_segment > 0, egui::Button::new("◀"))
                .on_hover_text(lang.text("Đưa đoạn sang trái", "Move clip left"))
                .clicked()
            {
                self.move_selected_segment(-1);
            }
            if ui
                .add_enabled(
                    self.selected_segment + 1 < self.segments.len(),
                    egui::Button::new("▶"),
                )
                .on_hover_text(lang.text("Đưa đoạn sang phải", "Move clip right"))
                .clicked()
            {
                self.move_selected_segment(1);
            }
            ui.separator();
            if ui
                .small_button("−")
                .on_hover_text(lang.text("Giảm tốc độ đoạn", "Slow down selected clip"))
                .clicked()
            {
                self.step_segment_speed(-1);
            }
            ui.label(
                RichText::new(format!(
                    "{:.2}×",
                    self.segments
                        .get(self.selected_segment)
                        .map(|segment| segment.speed)
                        .unwrap_or(1.0)
                ))
                .monospace()
                .strong()
                .color(ACCENT),
            );
            if ui
                .small_button("+")
                .on_hover_text(lang.text("Tăng tốc độ đoạn", "Speed up selected clip"))
                .clicked()
            {
                self.step_segment_speed(1);
            }
            if ui
                .small_button(lang.text("◁ Cắt trước", "◁ Trim left"))
                .on_hover_text(lang.text(
                    "Cắt bỏ phần trước playhead",
                    "Trim everything before the playhead",
                ))
                .clicked()
            {
                self.trim_left_at_marker();
            }
            if ui
                .small_button(lang.text("Cắt sau ▷", "Trim right ▷"))
                .on_hover_text(lang.text(
                    "Cắt bỏ phần sau playhead",
                    "Trim everything after the playhead",
                ))
                .clicked()
            {
                self.trim_right_at_marker();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} {}  ·  {}  ·  {}×{}",
                        lang.text("Đoạn", "Clip"),
                        self.selected_segment + 1,
                        format_time(duration),
                        info.width,
                        info.height
                    ))
                    .small()
                    .color(MUTED),
                );
            });
        });
        let canvas_height = ui.available_height().max(66.0);
        let (canvas, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), canvas_height),
            Sense::click_and_drag(),
        );
        let painter = ui.painter();
        painter.rect_filled(canvas, 6, Color32::from_rgb(10, 15, 23));
        let left = canvas.left() + 8.0;
        let right = canvas.right() - 8.0;
        let timeline_width = (right - left).max(1.0);
        let x_at = |time: f64| left + timeline_width * (time / duration) as f32;
        for step in 0..=10 {
            let time = duration * step as f64 / 10.0;
            let x = x_at(time);
            painter.line_segment(
                [
                    egui::pos2(x, canvas.top() + 4.0),
                    egui::pos2(x, canvas.top() + 12.0),
                ],
                Stroke::new(1.0, Color32::from_rgb(70, 80, 94)),
            );
            painter.text(
                egui::pos2(x, canvas.top() + 14.0),
                egui::Align2::CENTER_TOP,
                format_time(time),
                FontId::proportional(9.0),
                MUTED,
            );
        }
        let track_top = canvas.top() + 30.0;
        let track_bottom = (track_top + (canvas.height() * 0.48).max(42.0))
            .min(canvas.bottom() - 24.0)
            .max(track_top + 34.0);
        let video_track =
            Rect::from_min_max(egui::pos2(left, track_top), egui::pos2(right, track_bottom));
        painter.rect_filled(video_track, 4, Color32::from_rgb(17, 31, 43));
        let texture = self.preview_texture.as_ref().map(|texture| texture.id());
        let mut project_start = 0.0;
        let mut clicked_segment = None;
        for (index, segment) in self.segments.iter().copied().enumerate() {
            let project_end = project_start + segment.duration();
            let rect = Rect::from_min_max(
                egui::pos2(x_at(project_start) + 1.0, video_track.top() + 1.0),
                egui::pos2(x_at(project_end) - 1.0, video_track.bottom() - 1.0),
            );
            let selected = index == self.selected_segment;
            painter.rect_filled(
                rect,
                4,
                if selected {
                    Color32::from_rgb(35, 104, 116)
                } else {
                    Color32::from_rgb(27, 73, 82)
                },
            );
            if let Some(texture) = texture {
                painter.image(
                    texture,
                    Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.center().y)),
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::from_white_alpha(if selected { 210 } else { 150 }),
                );
            }
            if !self.waveform.is_empty() && rect.width() > 2.0 {
                let mid = rect.center().y + rect.height() * 0.16;
                let max_height = rect.height() * 0.27;
                let steps = rect.width().max(1.0) as usize;
                for px in (0..steps).step_by(2) {
                    let local = px as f64 / steps.max(1) as f64;
                    let source_time = segment.source_start + segment.source_duration() * local;
                    let wave_index = ((source_time / info.duration.max(0.001))
                        * (self.waveform.len() - 1) as f64)
                        .round() as usize;
                    let amplitude = self.waveform[wave_index.min(self.waveform.len() - 1)];
                    let x = rect.left() + px as f32;
                    painter.line_segment(
                        [
                            egui::pos2(x, mid - amplitude * max_height),
                            egui::pos2(x, mid + amplitude * max_height),
                        ],
                        Stroke::new(1.0, Color32::from_rgb(111, 198, 237)),
                    );
                }
            }
            painter.text(
                rect.left_bottom() + Vec2::new(5.0, -8.0),
                egui::Align2::LEFT_CENTER,
                format!(
                    "{} {}  ·  {:.2}×",
                    lang.text("Đoạn", "Clip"),
                    index + 1,
                    segment.speed
                ),
                FontId::proportional(9.5),
                Color32::WHITE,
            );
            painter.rect_stroke(
                rect,
                4,
                Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        ACCENT
                    } else {
                        Color32::from_rgb(49, 74, 84)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            if ui
                .interact(rect, ui.id().with(("clip_segment", index)), Sense::click())
                .on_hover_text(lang.text(
                    "Chọn đoạn để xóa, đổi thứ tự hoặc tốc độ",
                    "Select this clip to delete, reorder or change speed",
                ))
                .clicked()
            {
                clicked_segment = Some(index);
            }
            project_start = project_end;
        }
        if let Some(index) = clicked_segment {
            self.selected_segment = index;
        }
        for (index, layer) in self.layers.iter().take(2).enumerate() {
            let y = video_track.bottom() + 5.0 + index as f32 * 22.0;
            if y + 17.0 > canvas.bottom() {
                break;
            }
            let rect = Rect::from_min_max(
                egui::pos2(x_at(layer.start), y),
                egui::pos2(x_at(layer.end), y + 17.0),
            );
            painter.rect_filled(rect, 3, Color32::from_rgb(86, 63, 140));
            painter.text(
                rect.left_center() + Vec2::new(5.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("T{}  {}", index + 1, layer.text),
                FontId::proportional(9.5),
                Color32::WHITE,
            );
        }
        if response.clicked() || response.dragged() {
            if let Some(position) = response.interact_pointer_pos() {
                self.preview_time = (((position.x - left) / timeline_width).clamp(0.0, 1.0)
                    * info.duration as f32) as f64;
                self.playing = false;
            }
        }
        self.preview_time = self.preview_time.clamp(0.0, duration);
        let playhead = x_at(self.preview_time);
        painter.line_segment(
            [
                egui::pos2(playhead, canvas.top()),
                egui::pos2(playhead, canvas.bottom()),
            ],
            Stroke::new(2.0, Color32::from_rgb(244, 244, 247)),
        );
        painter.circle_filled(
            egui::pos2(playhead, canvas.top() + 2.0),
            4.0,
            Color32::WHITE,
        );
    }

    fn split_at_marker(&mut self) {
        let Some((index, project_start, source_at)) = self.segment_at(self.preview_time) else {
            return;
        };
        let segment = self.segments[index];
        if self.preview_time <= project_start + 0.01
            || source_at <= segment.source_start + 0.01
            || source_at >= segment.source_end - 0.01
        {
            return;
        }
        self.checkpoint();
        self.segments[index].source_end = source_at;
        self.segments.insert(
            index + 1,
            ClipSegment {
                source_start: source_at,
                source_end: segment.source_end,
                speed: segment.speed,
            },
        );
        self.selected_segment = index + 1;
        self.sync_project_duration();
    }

    fn trim_left_at_marker(&mut self) {
        let Some((index, _, source_at)) = self.segment_at(self.preview_time) else {
            return;
        };
        self.checkpoint();
        self.segments[index].source_start = source_at.min(self.segments[index].source_end - 0.01);
        self.segments.drain(0..index);
        self.selected_segment = 0;
        self.preview_time = 0.0;
        self.sync_project_duration();
    }

    fn trim_right_at_marker(&mut self) {
        let Some((index, _, source_at)) = self.segment_at(self.preview_time) else {
            return;
        };
        self.checkpoint();
        self.segments[index].source_end = source_at.max(self.segments[index].source_start + 0.01);
        self.segments.truncate(index + 1);
        self.selected_segment = self.selected_segment.min(index);
        self.sync_project_duration();
        self.preview_time = self.trim_end;
    }

    fn segment_project_start(&self, index: usize) -> f64 {
        self.segments
            .iter()
            .take(index)
            .map(|segment| segment.duration())
            .sum()
    }

    fn delete_selected_segment(&mut self) {
        if self.segments.len() <= 1 {
            return;
        }
        self.checkpoint();
        let index = self.selected_segment.min(self.segments.len() - 1);
        self.segments.remove(index);
        self.selected_segment = index.min(self.segments.len() - 1);
        self.sync_project_duration();
        self.preview_time = self.segment_project_start(self.selected_segment);
    }

    fn move_selected_segment(&mut self, direction: isize) {
        if self.segments.is_empty() {
            return;
        }
        let from = self.selected_segment.min(self.segments.len() - 1);
        let to = (from as isize + direction).clamp(0, self.segments.len() as isize - 1) as usize;
        if from == to {
            return;
        }
        self.checkpoint();
        self.segments.swap(from, to);
        self.selected_segment = to;
        self.preview_time = self.segment_project_start(to);
    }

    fn step_segment_speed(&mut self, direction: isize) {
        const SPEEDS: [f32; 7] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0];
        let Some(segment) = self.segments.get(self.selected_segment).copied() else {
            return;
        };
        let current = SPEEDS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - segment.speed)
                    .abs()
                    .total_cmp(&(*b - segment.speed).abs())
            })
            .map(|(index, _)| index)
            .unwrap_or(3);
        let next = (current as isize + direction).clamp(0, SPEEDS.len() as isize - 1) as usize;
        if SPEEDS[next] == segment.speed {
            return;
        }
        self.checkpoint();
        self.segments[self.selected_segment].speed = SPEEDS[next];
        self.sync_project_duration();
        self.preview_time = self
            .segment_project_start(self.selected_segment)
            .min(self.trim_end);
    }

    fn inspector_ui(&mut self, ui: &mut egui::Ui, output_folder: &Path, lang: Language) {
        egui::ScrollArea::vertical()
            .id_salt("editor_inspector_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                panel(ui, |ui| {
                    ui.label(
                        RichText::new(lang.text("ĐOẠN ĐANG CHỌN", "SELECTED CLIP"))
                            .small()
                            .strong()
                            .color(MUTED),
                    );
                    if let Some(segment) = self.segments.get(self.selected_segment).copied() {
                        ui.label(format!(
                            "{} {}  ·  {}  ·  {:.2}×",
                            lang.text("Đoạn", "Clip"),
                            self.selected_segment + 1,
                            format_time(segment.duration()),
                            segment.speed
                        ));
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(self.selected_segment > 0, egui::Button::new("◀"))
                                .on_hover_text(lang.text("Đưa sang trái", "Move left"))
                                .clicked()
                            {
                                self.move_selected_segment(-1);
                            }
                            if ui
                                .add_enabled(
                                    self.selected_segment + 1 < self.segments.len(),
                                    egui::Button::new("▶"),
                                )
                                .on_hover_text(lang.text("Đưa sang phải", "Move right"))
                                .clicked()
                            {
                                self.move_selected_segment(1);
                            }
                            if ui
                                .add_enabled(
                                    self.segments.len() > 1,
                                    egui::Button::new(lang.text("Xóa", "Delete")),
                                )
                                .clicked()
                            {
                                self.delete_selected_segment();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(lang.text("Tốc độ", "Speed"));
                            if ui.small_button("−").clicked() {
                                self.step_segment_speed(-1);
                            }
                            ui.label(RichText::new(format!("{:.2}×", segment.speed)).monospace());
                            if ui.small_button("+").clicked() {
                                self.step_segment_speed(1);
                            }
                        });
                    }
                });
                panel(ui, |ui| {
                    ui.label(
                        RichText::new(lang.text("CHỮ", "TEXT"))
                            .small()
                            .strong()
                            .color(MUTED),
                    );
                    ui.horizontal(|ui| {
                        if ui.button(lang.text("+ Thêm chữ", "+ Add text")).clicked() {
                            self.add_text(TextStylePreset::Clean);
                        }
                        if ui
                            .add_enabled(
                                !self.layers.is_empty(),
                                egui::Button::new(lang.text("Xóa", "Remove")),
                            )
                            .clicked()
                        {
                            self.checkpoint();
                            let index = self.selected_layer.min(self.layers.len() - 1);
                            self.layers.remove(index);
                            self.selected_layer = self.selected_layer.saturating_sub(1);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.text_tab,
                            TextInspectorTab::Basic,
                            lang.text("Cơ bản", "Basic"),
                        );
                        ui.selectable_value(
                            &mut self.text_tab,
                            TextInspectorTab::Animation,
                            "Animation",
                        );
                    });
                    if !self.layers.is_empty() {
                        egui::ComboBox::from_id_salt("text_layer")
                            .selected_text(format!(
                                "{} {}",
                                lang.text("Lớp", "Layer"),
                                self.selected_layer + 1
                            ))
                            .show_ui(ui, |ui| {
                                for i in 0..self.layers.len() {
                                    ui.selectable_value(
                                        &mut self.selected_layer,
                                        i,
                                        format!("{} {}", lang.text("Lớp", "Layer"), i + 1),
                                    );
                                }
                            });
                        let index = self.selected_layer.min(self.layers.len() - 1);
                        match self.text_tab {
                            TextInspectorTab::Basic => {
                                let before = self.snapshot();
                                let changed = {
                                    let layer = &mut self.layers[index];
                                    let mut changed =
                                        ui.text_edit_singleline(&mut layer.text).changed();
                                    egui::ComboBox::from_label(lang.text("Kiểu chữ", "Text style"))
                                        .selected_text(layer.style.label(lang))
                                        .show_ui(ui, |ui| {
                                            for style in TextStylePreset::ALL {
                                                changed |= ui
                                                    .selectable_value(
                                                        &mut layer.style,
                                                        style,
                                                        style.label(lang),
                                                    )
                                                    .changed();
                                            }
                                        });
                                    changed |= ui
                                        .add(
                                            egui::Slider::new(&mut layer.size, 18.0..=160.0)
                                                .text(lang.text("Cỡ chữ", "Text size")),
                                        )
                                        .changed();
                                    changed |= ui
                                        .add(egui::Slider::new(&mut layer.x, 0.0..=0.95).text("X"))
                                        .changed();
                                    changed |= ui
                                        .add(egui::Slider::new(&mut layer.y, 0.05..=0.98).text("Y"))
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::Slider::new(
                                                &mut layer.start,
                                                self.trim_start..=layer.end,
                                            )
                                            .text(lang.text("Hiện từ", "Show from")),
                                        )
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::Slider::new(
                                                &mut layer.end,
                                                layer.start..=self.trim_end,
                                            )
                                            .text(lang.text("Ẩn lúc", "Hide at")),
                                        )
                                        .changed();
                                    changed
                                };
                                if changed {
                                    self.record_previous(before);
                                }
                            }
                            TextInspectorTab::Animation => {
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut self.animation_phase,
                                        AnimationPhase::In,
                                        "In",
                                    );
                                    ui.selectable_value(
                                        &mut self.animation_phase,
                                        AnimationPhase::Out,
                                        "Out",
                                    );
                                });
                                let before = self.snapshot();
                                let layer = &mut self.layers[index];
                                let mut changed = false;
                                let motions: &[TextMotion] =
                                    if self.animation_phase == AnimationPhase::In {
                                        &TextMotion::ALL
                                    } else {
                                        &TextMotion::ALL[..4]
                                    };
                                for row in motions.chunks(2) {
                                    ui.columns(2, |columns| {
                                        for (column, motion) in row.iter().copied().enumerate() {
                                            let current = match self.animation_phase {
                                                AnimationPhase::In => layer.motion_in,
                                                AnimationPhase::Out => layer.motion_out,
                                            };
                                            let clicked = columns[column]
                                                .add_sized(
                                                    [columns[column].available_width(), 42.0],
                                                    egui::Button::new(motion.label(lang)).fill(
                                                        if current == motion {
                                                            Color32::from_rgb(35, 70, 83)
                                                        } else {
                                                            Color32::from_rgb(31, 43, 59)
                                                        },
                                                    ),
                                                )
                                                .clicked();
                                            if clicked {
                                                changed = true;
                                                match self.animation_phase {
                                                    AnimationPhase::In => layer.motion_in = motion,
                                                    AnimationPhase::Out => {
                                                        layer.motion_out = motion
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                                if changed {
                                    self.record_previous(before);
                                }
                            }
                        }
                    } else {
                        ui.label(
                            RichText::new(lang.text(
                                "Thêm chữ để tạo tiêu đề hoặc chú thích.",
                                "Add text for a title or caption.",
                            ))
                            .small()
                            .color(MUTED),
                        );
                    }
                    let before_motion = self.snapshot();
                    let mut motion_changed = false;
                    egui::ComboBox::from_label(lang.text("Preset video", "Video preset"))
                        .selected_text(self.video_motion.label(lang))
                        .show_ui(ui, |ui| {
                            for motion in VideoMotion::ALL {
                                motion_changed |= ui
                                    .selectable_value(
                                        &mut self.video_motion,
                                        motion,
                                        motion.label(lang),
                                    )
                                    .changed();
                            }
                        });
                    if motion_changed {
                        self.record_previous(before_motion);
                    }
                });
                panel(ui, |ui| {
                    ui.label(
                        RichText::new(lang.text("XUẤT VIDEO", "EXPORT"))
                            .small()
                            .strong()
                            .color(MUTED),
                    );
                    egui::ComboBox::from_label(lang.text("Kích thước", "Size"))
                        .selected_text(self.output_size.label(lang))
                        .show_ui(ui, |ui| {
                            for size in OutputSize::ALL {
                                ui.selectable_value(&mut self.output_size, size, size.label(lang));
                            }
                        });
                    egui::ComboBox::from_label(lang.text("Định dạng", "Format"))
                        .selected_text(self.output_format.label())
                        .show_ui(ui, |ui| {
                            for format in OutputFormat::ALL {
                                ui.selectable_value(
                                    &mut self.output_format,
                                    format,
                                    format.label(),
                                );
                            }
                        });
                    let running = self.export_rx.is_some();
                    if running {
                        ui.add(egui::ProgressBar::new(self.export_progress).show_percentage());
                    }
                    if ui
                        .add_enabled(
                            !running && self.trim_end > self.trim_start,
                            egui::Button::new(
                                RichText::new(lang.text("Xuất video", "Export video")).strong(),
                            )
                            .fill(RED),
                        )
                        .on_hover_text(lang.text(
                            "Lưu bản dựng mới, không sửa file gốc",
                            "Save a new edit without changing the source",
                        ))
                        .clicked()
                    {
                        self.start_export(output_folder);
                    }
                    ui.label(
                        RichText::new(output_folder.display().to_string())
                            .small()
                            .color(MUTED),
                    );
                });
            });
    }

    fn start_export(&mut self, output_folder: &Path) {
        let (Some(input), Some(info)) = (self.input.clone(), self.info) else {
            return;
        };
        let name = format!(
            "Edit_{}.{}",
            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"),
            self.output_format.extension()
        );
        let output = output_folder.join(name);
        let job = ExportJob {
            input,
            output,
            info,
            segments: self.segments.clone(),
            crop: self.crop,
            layers: self.layers.clone(),
            video_motion: self.video_motion,
            output_size: self.output_size,
            output_format: self.output_format,
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(error) = export(job, &tx) {
                let _ = tx.send(ExportEvent::Failed(format!("{error:#}")));
            }
        });
        self.export_progress = 0.0;
        self.export_error = None;
        self.export_rx = Some(rx);
    }
}

#[derive(Clone)]
struct ExportJob {
    input: PathBuf,
    output: PathBuf,
    info: MediaInfo,
    segments: Vec<ClipSegment>,
    crop: Crop,
    layers: Vec<TextLayer>,
    video_motion: VideoMotion,
    output_size: OutputSize,
    output_format: OutputFormat,
}

#[derive(Deserialize)]
struct ProbeOutput {
    streams: Vec<ProbeStream>,
    format: ProbeFormat,
}
#[derive(Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}
#[derive(Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}

fn probe(path: &Path) -> anyhow::Result<MediaInfo> {
    let output = Command::new(recorder::tool("ffprobe")?)
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path)
        .output()?;
    anyhow::ensure!(output.status.success(), "FFprobe không đọc được video");
    let data: ProbeOutput = serde_json::from_slice(&output.stdout)?;
    let video = data
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("video"))
        .ok_or_else(|| anyhow::anyhow!("File không có luồng video"))?;
    let duration = data
        .format
        .duration
        .as_deref()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .ok_or_else(|| anyhow::anyhow!("Không xác định được thời lượng video"))?;
    Ok(MediaInfo {
        width: video.width.unwrap_or(0),
        height: video.height.unwrap_or(0),
        duration,
        has_audio: data
            .streams
            .iter()
            .any(|stream| stream.codec_type.as_deref() == Some("audio")),
    })
}

fn waveform(path: &Path) -> anyhow::Result<Vec<f32>> {
    let output = Command::new(recorder::tool("ffmpeg")?)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-map", "0:a:0?", "-vn", "-ac", "1", "-ar", "400", "-f", "s16le", "pipe:1",
        ])
        .output()?;
    if !output.status.success() || output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    let samples = output
        .stdout
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]).unsigned_abs() as f32 / 32768.0)
        .collect::<Vec<_>>();
    let bin_size = (samples.len() / 6000).max(1);
    Ok(samples
        .chunks(bin_size)
        .map(|chunk| chunk.iter().copied().fold(0.0, f32::max).sqrt())
        .collect())
}

fn atempo_filters(speed: f32) -> String {
    let mut value = speed.clamp(0.25, 4.0);
    let mut filters = Vec::new();
    while value < 0.5 {
        filters.push("atempo=0.5".to_string());
        value /= 0.5;
    }
    while value > 2.0 {
        filters.push("atempo=2.0".to_string());
        value /= 2.0;
    }
    filters.push(format!("atempo={value:.6}"));
    filters.join(",")
}

fn preview(path: &Path, at: f64) -> anyhow::Result<PreviewFrame> {
    let output = Command::new(recorder::tool("ffmpeg")?)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-hide_banner", "-loglevel", "error", "-ss"])
        .arg(format!("{at:.3}"))
        .arg("-i")
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=960:-2",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "pipe:1",
        ])
        .output()?;
    anyhow::ensure!(output.status.success(), "Không tạo được preview");
    let image = image::load_from_memory(&output.stdout)?.to_rgba8();
    Ok(PreviewFrame {
        width: image.width() as usize,
        height: image.height() as usize,
        rgba: image.into_raw(),
    })
}

fn export(job: ExportJob, events: &std::sync::mpsc::Sender<ExportEvent>) -> anyhow::Result<()> {
    std::fs::create_dir_all(job.output.parent().unwrap_or_else(|| Path::new(".")))?;
    anyhow::ensure!(!job.segments.is_empty(), "Timeline không có đoạn video");
    let duration = job
        .segments
        .iter()
        .map(|segment| segment.duration())
        .sum::<f64>()
        .max(0.05);
    let crop_w = ((job.info.width as f32 * job.crop.w).round() as u32).max(16) & !1;
    let crop_h = ((job.info.height as f32 * job.crop.h).round() as u32).max(16) & !1;
    let crop_x = ((job.info.width as f32 * job.crop.x).round() as u32)
        .min(job.info.width.saturating_sub(crop_w))
        & !1;
    let crop_y = ((job.info.height as f32 * job.crop.y).round() as u32)
        .min(job.info.height.saturating_sub(crop_h))
        & !1;
    let target = job
        .output_size
        .dimensions((job.info.width, job.info.height), job.crop);
    let mut post_filters = vec![format!("crop={crop_w}:{crop_h}:{crop_x}:{crop_y}")];
    match job.video_motion {
        VideoMotion::SlowZoom => post_filters.push(format!(
            "zoompan=z='min(zoom+0.0007,1.08)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s={}x{}:fps=30",
            target.0, target.1
        )),
        _ => post_filters.push(format!(
            "scale={}:{}:force_original_aspect_ratio=decrease:flags=lanczos,pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1",
            target.0, target.1, target.0, target.1
        )),
    }
    if job.video_motion == VideoMotion::Fade {
        post_filters.push(format!(
            "fade=t=in:st=0:d=0.4,fade=t=out:st={:.3}:d=0.4",
            (duration - 0.4).max(0.0)
        ));
    }
    let ass = if job.layers.iter().any(|layer| !layer.text.trim().is_empty()) {
        let path = job.output.with_extension("editor.ass");
        std::fs::write(&path, ass_document(&job.layers, target))?;
        post_filters.push(format!("subtitles=filename='{}'", ffmpeg_path(&path)));
        Some(path)
    } else {
        None
    };
    post_filters.push("format=yuv420p".into());
    let mut graph = Vec::new();
    let count = job.segments.len();
    if count > 1 {
        graph.push(format!(
            "[0:v:0]split={count}{}",
            (0..count)
                .map(|index| format!("[vsrc{index}]"))
                .collect::<String>()
        ));
        if job.info.has_audio {
            graph.push(format!(
                "[0:a:0]asplit={count}{}",
                (0..count)
                    .map(|index| format!("[asrc{index}]"))
                    .collect::<String>()
            ));
        }
    }
    for (index, segment) in job.segments.iter().copied().enumerate() {
        let video_source = if count == 1 {
            "[0:v:0]".to_string()
        } else {
            format!("[vsrc{index}]")
        };
        graph.push(format!(
            "{video_source}trim=start={:.6}:end={:.6},setpts=(PTS-STARTPTS)/{:.6}[v{index}]",
            segment.source_start, segment.source_end, segment.speed
        ));
        if job.info.has_audio {
            let audio_source = if count == 1 {
                "[0:a:0]".to_string()
            } else {
                format!("[asrc{index}]")
            };
            graph.push(format!(
                "{audio_source}atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS,{}[a{index}]",
                segment.source_start,
                segment.source_end,
                atempo_filters(segment.speed)
            ));
        }
    }
    if count == 1 {
        graph.push("[v0]null[vjoined]".into());
        if job.info.has_audio {
            graph.push("[a0]anull[ajoined]".into());
        }
    } else if job.info.has_audio {
        graph.push(format!(
            "{}concat=n={count}:v=1:a=1[vjoined][ajoined]",
            (0..count)
                .map(|index| format!("[v{index}][a{index}]"))
                .collect::<String>()
        ));
    } else {
        graph.push(format!(
            "{}concat=n={count}:v=1:a=0[vjoined]",
            (0..count)
                .map(|index| format!("[v{index}]"))
                .collect::<String>()
        ));
    }
    graph.push(format!("[vjoined]{}[vout]", post_filters.join(",")));
    let mut command = Command::new(recorder::tool("ffmpeg")?);
    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&job.input)
        .args(["-filter_complex"])
        .arg(graph.join(";"))
        .args(["-map", "[vout]"]);
    if job.info.has_audio {
        command.args(["-map", "[ajoined]"]);
    }
    command.args(["-r", "30"]);
    match job.output_format {
        OutputFormat::Webm => {
            command.args([
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "27",
                "-b:v",
                "0",
                "-c:a",
                "libopus",
                "-b:a",
                "160k",
            ]);
        }
        OutputFormat::Mp4 | OutputFormat::Mkv => {
            command.args([
                "-c:v", "libx264", "-preset", "fast", "-crf", "20", "-c:a", "aac", "-b:a", "192k",
            ]);
            if job.output_format == OutputFormat::Mp4 {
                command.args(["-movflags", "+faststart"]);
            }
        }
    }
    command
        .args(["-progress", "pipe:1", "-nostats"])
        .arg(&job.output);
    let mut child = command.spawn()?;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(value) = line
                .strip_prefix("out_time_us=")
                .and_then(|v| v.parse::<f64>().ok())
            {
                let _ = events.send(ExportEvent::Progress(
                    (value / 1_000_000.0 / duration) as f32,
                ));
            }
        }
    }
    let output = child.wait_with_output()?;
    if let Some(path) = ass {
        let _ = std::fs::remove_file(path);
    }
    anyhow::ensure!(
        output.status.success(),
        "FFmpeg xuất video thất bại: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = events.send(ExportEvent::Done(job.output));
    Ok(())
}

fn ass_document(layers: &[TextLayer], target: (u32, u32)) -> String {
    let mut text = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {}\nPlayResY: {}\nWrapStyle: 2\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Segoe UI,48,&H00FFFFFF,&H00FFFFFF,&H90000000,&H70000000,0,0,0,0,100,100,0,0,1,2,1,7,0,0,0,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        target.0, target.1
    );
    for layer in layers.iter().filter(|layer| !layer.text.trim().is_empty()) {
        let x = (layer.x * target.0 as f32).round() as i32;
        let y = (layer.y * target.1 as f32).round() as i32;
        let escaped = layer
            .text
            .replace('{', "\\{")
            .replace('}', "\\}")
            .replace('\n', "\\N");
        let duration_ms = ((layer.end - layer.start).max(0.01) * 1000.0).round() as i32;
        let out_start = (duration_ms - 350).max(0);
        let style = ass_style_tag(layer.style);
        let mut animation = String::new();
        if layer.motion_in == TextMotion::Fade || layer.motion_out == TextMotion::Fade {
            animation.push_str(&format!(
                "\\fad({},{})",
                if layer.motion_in == TextMotion::Fade {
                    350
                } else {
                    0
                },
                if layer.motion_out == TextMotion::Fade {
                    350
                } else {
                    0
                }
            ));
        }
        match layer.motion_in {
            TextMotion::SlideUp => {
                animation.push_str(&format!("\\move({x},{},{x},{y},0,450)", y + 70));
            }
            TextMotion::Pop => {
                animation.push_str("\\fscx70\\fscy70\\t(0,300,\\fscx100\\fscy100)");
            }
            _ => animation.push_str(&format!("\\pos({x},{y})")),
        }
        match layer.motion_out {
            TextMotion::SlideUp => animation.push_str(&format!(
                "\\t({out_start},{duration_ms},\\alpha&HFF&\\frz-4)"
            )),
            TextMotion::Pop => animation.push_str(&format!(
                "\\t({out_start},{duration_ms},\\fscx65\\fscy65\\alpha&HFF&)"
            )),
            TextMotion::Typewriter => animation.push_str(&format!(
                "\\t({out_start},{duration_ms},\\fscx112\\fscy112\\alpha&HFF&)"
            )),
            _ => {}
        }
        let tag = format!("{{\\fs{}{}{} }}", layer.size.round(), style, animation);
        if layer.motion_in == TextMotion::Typewriter {
            let chars = escaped.chars().collect::<Vec<_>>();
            let step = ((layer.end - layer.start) / chars.len().max(1) as f64).min(0.09);
            for count in 1..=chars.len() {
                let start = layer.start + (count - 1) as f64 * step;
                let end = if count == chars.len() {
                    layer.end
                } else {
                    (start + step).min(layer.end)
                };
                text.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}{}\n",
                    ass_time(start),
                    ass_time(end),
                    tag,
                    chars[..count].iter().collect::<String>()
                ));
            }
        } else {
            text.push_str(&format!(
                "Dialogue: 0,{},{},Default,,0,0,0,,{}{}\n",
                ass_time(layer.start),
                ass_time(layer.end),
                tag,
                escaped
            ));
        }
    }
    text
}

fn ass_style_tag(style: TextStylePreset) -> &'static str {
    match style {
        TextStylePreset::Clean => "",
        TextStylePreset::Bold => "\\b1",
        TextStylePreset::Outline => "\\bord4\\3c&H00101010&",
        TextStylePreset::Neon => "\\bord3\\3c&H00FFE000&\\shad2\\4c&H00FF00FF&",
        TextStylePreset::Label => "\\bord10\\3c&H00201810&",
        TextStylePreset::Shadow => "\\shad4\\4c&H00000000&",
    }
}

fn ffmpeg_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

fn ass_time(value: f64) -> String {
    let value = value.max(0.0);
    let hours = (value / 3600.0) as u32;
    let minutes = (value / 60.0) as u32 % 60;
    let seconds = value as u32 % 60;
    let centiseconds = ((value.fract() * 100.0).round() as u32).min(99);
    format!("{hours}:{minutes:02}:{seconds:02}.{centiseconds:02}")
}

fn format_time(value: f64) -> String {
    let minutes = (value / 60.0) as u64;
    format!("{minutes:02}:{:05.2}", value % 60.0)
}

fn norm_point(point: Pos2, rect: Rect) -> (f32, f32) {
    (
        ((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
        ((point.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
    )
}

fn snap_value(value: f32, targets: &[f32], threshold: f32) -> (f32, bool) {
    targets
        .iter()
        .copied()
        .map(|target| (target, (target - value).abs()))
        .filter(|(_, distance)| *distance <= threshold)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(target, _)| (target, true))
        .unwrap_or((value, false))
}

fn crop_points(a: (f32, f32), b: (f32, f32)) -> Crop {
    let x = a.0.min(b.0);
    let y = a.1.min(b.1);
    Crop {
        x,
        y,
        w: (a.0 - b.0).abs().max(0.02).min(1.0 - x),
        h: (a.1 - b.1).abs().max(0.02).min(1.0 - y),
    }
}

fn shade_outside(ui: &egui::Ui, outer: Rect, crop: Rect) {
    let shade = Color32::from_black_alpha(150);
    for rect in [
        Rect::from_min_max(outer.min, egui::pos2(outer.right(), crop.top())),
        Rect::from_min_max(egui::pos2(outer.left(), crop.bottom()), outer.max),
        Rect::from_min_max(
            egui::pos2(outer.left(), crop.top()),
            egui::pos2(crop.left(), crop.bottom()),
        ),
        Rect::from_min_max(
            egui::pos2(crop.right(), crop.top()),
            egui::pos2(outer.right(), crop.bottom()),
        ),
    ] {
        ui.painter().rect_filled(rect, 0, shade);
    }
}

fn paint_text_preset(
    painter: &egui::Painter,
    position: Pos2,
    text: &str,
    size: f32,
    style: TextStylePreset,
    bounds: Rect,
) {
    let font = FontId::proportional(size);
    let draw = |offset: Vec2, color: Color32| {
        painter.text(
            position + offset,
            egui::Align2::LEFT_BOTTOM,
            text,
            font.clone(),
            color,
        );
    };
    match style {
        TextStylePreset::Clean => draw(Vec2::ZERO, Color32::WHITE),
        TextStylePreset::Bold => {
            draw(Vec2::new(1.5, 0.0), Color32::WHITE);
            draw(Vec2::ZERO, Color32::WHITE);
        }
        TextStylePreset::Outline => {
            for offset in [
                Vec2::new(-2.0, 0.0),
                Vec2::new(2.0, 0.0),
                Vec2::new(0.0, -2.0),
                Vec2::new(0.0, 2.0),
                Vec2::new(-1.5, -1.5),
                Vec2::new(1.5, 1.5),
            ] {
                draw(offset, Color32::from_black_alpha(230));
            }
            draw(Vec2::ZERO, Color32::WHITE);
        }
        TextStylePreset::Neon => {
            for offset in [
                Vec2::new(-3.0, 0.0),
                Vec2::new(3.0, 0.0),
                Vec2::new(0.0, -3.0),
                Vec2::new(0.0, 3.0),
            ] {
                draw(offset, Color32::from_rgba_unmultiplied(0, 210, 255, 125));
            }
            draw(Vec2::ZERO, Color32::from_rgb(238, 252, 255));
        }
        TextStylePreset::Label => {
            painter.rect_filled(
                bounds.expand2(Vec2::new(8.0, 5.0)),
                5,
                Color32::from_rgba_unmultiplied(8, 16, 28, 220),
            );
            draw(Vec2::ZERO, Color32::WHITE);
        }
        TextStylePreset::Shadow => {
            draw(Vec2::new(4.0, 4.0), Color32::from_black_alpha(185));
            draw(Vec2::ZERO, Color32::WHITE);
        }
    }
}

fn panel(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, Color32::from_rgb(39, 49, 64)))
        .corner_radius(10)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            content(ui);
        });
    ui.add_space(7.0);
}

fn empty_editor(ui: &mut egui::Ui, lang: Language) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 390.0), Sense::hover());
    ui.painter().rect_filled(rect, 12, PANEL);
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        12,
        Stroke::new(1.0, Color32::from_rgb(39, 49, 64)),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center() - Vec2::new(0.0, 18.0),
        egui::Align2::CENTER_CENTER,
        lang.text("Chọn một clip để bắt đầu", "Choose a clip to start"),
        FontId::proportional(22.0),
        Color32::WHITE,
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 18.0),
        egui::Align2::CENTER_CENTER,
        lang.text(
            "Crop, cắt thời gian, thêm chữ và xuất 1080p.",
            "Crop, trim, add animated text and export at 1080p.",
        ),
        FontId::proportional(14.0),
        MUTED,
    );
}

pub fn smoke_export(input: &Path, output: &Path) -> anyhow::Result<PathBuf> {
    let info = probe(input)?;
    let end = info.duration.min(2.5);
    let layer = TextLayer {
        text: "Screen Recorder Studio".into(),
        x: 0.07,
        y: 0.86,
        size: 42.0,
        start: 0.1,
        end: end.max(0.2),
        style: TextStylePreset::Outline,
        motion_in: TextMotion::SlideUp,
        motion_out: TextMotion::Fade,
    };
    let output_format = match output.extension().and_then(|value| value.to_str()) {
        Some("mkv") => OutputFormat::Mkv,
        Some("webm") => OutputFormat::Webm,
        _ => OutputFormat::Mp4,
    };
    let job = ExportJob {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        info,
        // Exercise the production split/reorder/speed filter graph: the latter
        // half is exported first at 2×, followed by the former half at 0.5×.
        segments: vec![
            ClipSegment {
                source_start: end / 2.0,
                source_end: end,
                speed: 2.0,
            },
            ClipSegment {
                source_start: 0.0,
                source_end: end / 2.0,
                speed: 0.5,
            },
        ],
        crop: Crop {
            x: 0.05,
            y: 0.05,
            w: 0.90,
            h: 0.90,
        },
        layers: vec![layer],
        video_motion: VideoMotion::Fade,
        output_size: OutputSize::FullHd,
        output_format,
    };
    let (tx, _rx) = mpsc::channel();
    export(job, &tx)?;
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_is_normalized_and_has_minimum_size() {
        let crop = crop_points((0.8, 0.7), (0.2, 0.1));
        assert!((crop.x - 0.2).abs() < 0.001);
        assert!((crop.w - 0.6).abs() < 0.001);
        assert!((crop.h - 0.6).abs() < 0.001);
    }

    #[test]
    fn ass_contains_time_based_motion() {
        let layer = TextLayer {
            text: "Hello".into(),
            style: TextStylePreset::Outline,
            motion_in: TextMotion::SlideUp,
            motion_out: TextMotion::Fade,
            ..Default::default()
        };
        let ass = ass_document(&[layer], (1920, 1080));
        assert!(ass.contains("\\move("));
        assert!(ass.contains("\\fad(0,350)"));
        assert!(ass.contains("\\bord4"));
        assert!(ass.contains("Hello"));
    }

    #[test]
    fn split_and_one_sided_trim_follow_playhead() {
        let mut editor = EditorState {
            info: Some(MediaInfo {
                width: 1280,
                height: 720,
                duration: 20.0,
                has_audio: true,
            }),
            trim_end: 20.0,
            preview_time: 8.0,
            segments: vec![ClipSegment {
                source_start: 0.0,
                source_end: 20.0,
                speed: 1.0,
            }],
            ..Default::default()
        };
        editor.split_at_marker();
        assert_eq!(editor.segments.len(), 2);
        assert_eq!(editor.segments[0].source_end, 8.0);
        assert_eq!(editor.segments[1].source_start, 8.0);
        editor.preview_time = 5.0;
        editor.trim_left_at_marker();
        assert_eq!(editor.segments[0].source_start, 5.0);
        assert_eq!(editor.trim_end, 15.0);
        editor.preview_time = 10.0;
        editor.trim_right_at_marker();
        assert_eq!(editor.segments.len(), 2);
        assert_eq!(editor.segments[1].source_end, 15.0);
        assert_eq!(editor.trim_end, 10.0);
    }

    #[test]
    fn clip_reorder_speed_and_editor_history_are_independent() {
        let mut editor = EditorState {
            segments: vec![
                ClipSegment {
                    source_start: 0.0,
                    source_end: 4.0,
                    speed: 1.0,
                },
                ClipSegment {
                    source_start: 4.0,
                    source_end: 10.0,
                    speed: 1.0,
                },
            ],
            selected_segment: 1,
            trim_end: 10.0,
            ..Default::default()
        };
        editor.move_selected_segment(-1);
        assert_eq!(editor.segments[0].source_start, 4.0);
        editor.step_segment_speed(1);
        assert_eq!(editor.segments[0].speed, 1.5);
        assert!((editor.project_duration() - 8.0).abs() < 0.001);
        editor.undo();
        assert_eq!(editor.segments[0].speed, 1.0);
        editor.undo();
        assert_eq!(editor.segments[0].source_start, 0.0);
        editor.redo();
        assert_eq!(editor.segments[0].source_start, 4.0);
    }

    #[test]
    fn text_snap_and_audio_tempo_cover_safe_zone_and_extreme_speeds() {
        assert_eq!(snap_value(0.493, &[0.05, 0.5, 0.95], 0.01), (0.5, true));
        assert_eq!(snap_value(0.7, &[0.05, 0.5, 0.95], 0.01), (0.7, false));
        assert_eq!(atempo_filters(0.25), "atempo=0.5,atempo=0.500000");
        assert_eq!(atempo_filters(4.0), "atempo=2.0,atempo=2.000000");
    }
}
