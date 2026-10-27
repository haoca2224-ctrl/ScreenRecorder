use crate::{
    library::{Item, Kind, Request, Service, Snapshot, State},
    model::Language,
};
use eframe::egui::{self, Color32, RichText, Vec2};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
};

pub enum Action {
    Edit(PathBuf),
    Recover(PathBuf),
}
pub struct LibraryUi {
    service: Service,
    snapshot: Snapshot,
    root: PathBuf,
    query: String,
    filter: u8,
    sort: u8,
    descending: bool,
    favorites: bool,
    grid: bool,
    selection: HashSet<String>,
    filtered: Vec<usize>,
    filter_key: String,
    detail: Option<String>,
    rename: Option<(String, String)>,
    trash: Option<Vec<String>>,
    textures: HashMap<PathBuf, egui::TextureHandle>,
    pending: HashSet<PathBuf>,
    thumb_tx: mpsc::Sender<PathBuf>,
    thumb_rx: mpsc::Receiver<(PathBuf, Option<egui::ColorImage>)>,
}
impl LibraryUi {
    pub fn refresh(&self) {
        self.send(Request::Refresh);
    }
    pub fn recording_result(
        &mut self,
        ui: &mut egui::Ui,
        path: &std::path::Path,
        lang: Language,
    ) -> bool {
        let item = self.snapshot.items.iter().find(|i| i.path == path).cloned();
        let mut edit = false;
        ui.horizontal(|ui| {
            if let Some(item) = &item {
                self.thumbnail(ui, item, Vec2::new(144., 81.));
            } else {
                ui.spinner();
            }
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(path.file_name().unwrap_or_default().to_string_lossy())
                        .truncate(),
                );
                if let Some(item) = &item {
                    if let Some(m) = &item.metadata {
                        ui.label(format!(
                            "{}:{:02} · {} × {} · {}",
                            m.duration as u64 / 60,
                            m.duration as u64 % 60,
                            m.width,
                            m.height,
                            size_label(item.size)
                        ));
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(lang.text("Xem", "View")).clicked() {
                        open(path);
                    }
                    let project = item.as_ref().is_some_and(|i| {
                        i.project
                            .as_deref()
                            .is_some_and(crate::concat_bridge::is_project)
                    });
                    edit = ui
                        .button(if project {
                            lang.text("Tiếp tục chỉnh sửa", "Continue editing")
                        } else {
                            lang.text("Chỉnh sửa", "Edit")
                        })
                        .clicked();
                    if ui.button(lang.text("Mở thư mục", "Open folder")).clicked() {
                        if let Some(p) = path.parent() {
                            open(p);
                        }
                    }
                });
            });
        });
        edit
    }
    pub fn new(root: PathBuf) -> Self {
        let service = Service::start(root.clone());
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let (done, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(p) = rx.recv() {
                let img = image::open(&p).ok().map(|i| {
                    let i = i.to_rgba8();
                    egui::ColorImage::from_rgba_unmultiplied(
                        [i.width() as usize, i.height() as usize],
                        i.as_raw(),
                    )
                });
                if done.send((p, img)).is_err() {
                    break;
                }
            }
        });
        Self {
            service,
            snapshot: Snapshot::default(),
            root,
            query: String::new(),
            filter: 0,
            sort: 0,
            descending: true,
            favorites: false,
            grid: true,
            selection: HashSet::new(),
            filtered: vec![],
            filter_key: String::new(),
            detail: None,
            rename: None,
            trash: None,
            textures: HashMap::new(),
            pending: HashSet::new(),
            thumb_tx: tx,
            thumb_rx: results,
        }
    }
    pub fn tick(&mut self, root: &std::path::Path, ctx: &egui::Context) {
        if self.root != root {
            self.root = root.to_owned();
            self.send(Request::Root(self.root.clone()));
        }
        if let Ok(s) = self.service.snapshot.try_lock() {
            if self.snapshot.revision != s.revision {
                self.snapshot = s.clone();
            }
        }
        for (p, image) in self.thumb_rx.try_iter() {
            self.pending.remove(&p);
            if let Some(image) = image {
                if self.textures.len() >= 128 {
                    self.textures.clear();
                }
                self.textures.insert(
                    p.clone(),
                    ctx.load_texture(p.to_string_lossy(), image, egui::TextureOptions::LINEAR),
                );
            }
        }
    }
    fn send(&self, r: Request) {
        let _ = self.service.tx.send(r);
    }
    fn thumbnail(&mut self, ui: &mut egui::Ui, item: &Item, size: Vec2) {
        let thumb = item.thumbnail.clone().or_else(|| {
            item.sources.iter().find_map(|p| {
                self.snapshot
                    .items
                    .iter()
                    .find(|i| i.path == *p)
                    .and_then(|i| i.thumbnail.clone())
            })
        });
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 6, Color32::from_rgb(12, 19, 29));
        if let Some(p) = thumb {
            if let Some(texture) = self.textures.get(&p) {
                let ratio = (size.x / texture.size_vec2().x).min(size.y / texture.size_vec2().y);
                let image_rect =
                    egui::Rect::from_center_size(rect.center(), texture.size_vec2() * ratio);
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                    Color32::WHITE,
                );
                return;
            } else if self.pending.len() < 32 && self.pending.insert(p.clone()) {
                let _ = self.thumb_tx.send(p);
            }
        }
        let label = match item.kind {
            Kind::Image => "ẢNH / IMAGE",
            Kind::Project => "PROJECT",
            Kind::Session => "PHIÊN GHI / SESSION",
            _ => "VIDEO",
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.),
            Color32::from_rgb(157, 174, 195),
        );
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, lang: Language, can_recover: bool) -> Option<Action> {
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            ui.heading(lang.text("Thư viện", "Library"));
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    self.snapshot.items.len(),
                    lang.text("mục", "items")
                ))
                .color(Color32::LIGHT_BLUE),
            );
            if ui
                .button(lang.text("Mở thư mục lưu", "Storage folder"))
                .clicked()
            {
                open(&self.root);
            }
            if ui.button(lang.text("Quét lại", "Refresh")).clicked() {
                self.send(Request::Refresh);
            }
            ui.menu_button(
                lang.text("Thư mục & dữ liệu", "Folders & data"),
                |ui| {
                    for root in &self.snapshot.roots {
                        if ui.button(root.to_string_lossy()).clicked() {
                            open(root);
                        }
                    }
                    if ui
                        .button(lang.text("Thêm thư mục…", "Add folder…"))
                        .clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            self.send(Request::Root(p));
                        }
                        ui.close_menu();
                    }
                    if ui
                        .button(lang.text("Xây dựng lại cache", "Rebuild cache"))
                        .clicked()
                    {
                        self.send(Request::Rebuild);
                        ui.close_menu();
                    }
                },
            );
        });
        ui.label(
            RichText::new(lang.text(
                "Video gốc, ảnh, project và bản xuất — tất cả trong một thư viện.",
                "Recordings, images, projects and exports in one library.",
            ))
            .small()
            .color(Color32::from_rgb(157, 174, 195)),
        );
        ui.add_space(8.);
        ui.horizontal_wrapped(|ui| {
            ui.label(lang.text("Tìm tên", "Find name"));
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(210.)
                    .hint_text(lang.text("Nhập tên…", "Search…")),
            );
            for (value, label) in [
                (0, lang.text("Tất cả", "All")),
                (1, "Video"),
                (2, lang.text("Ảnh", "Images")),
                (3, "Project"),
                (4, lang.text("Bản xuất", "Exports")),
            ] {
                ui.selectable_value(&mut self.filter, value, label);
            }
            ui.checkbox(&mut self.favorites, lang.text("Yêu thích", "Favorites"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(lang.text("Sắp xếp", "Sort"));
            egui::ComboBox::from_id_salt("library-sort")
                .selected_text(match self.sort {
                    1 => lang.text("Tên", "Name"),
                    2 => lang.text("Dung lượng", "Size"),
                    _ => lang.text("Ngày tạo", "Created"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort, 0, lang.text("Ngày tạo", "Created"));
                    ui.selectable_value(&mut self.sort, 1, lang.text("Tên", "Name"));
                    ui.selectable_value(&mut self.sort, 2, lang.text("Dung lượng", "Size"));
                });
            ui.checkbox(&mut self.descending, lang.text("Giảm dần", "Descending"));
            ui.separator();
            ui.selectable_value(&mut self.grid, true, lang.text("Lưới", "Grid"));
            ui.selectable_value(&mut self.grid, false, lang.text("Danh sách", "List"));
            if self.snapshot.busy > 0 || self.snapshot.revision == 0 {
                ui.spinner();
                ui.label(lang.text("Đang đọc metadata…", "Reading metadata…"));
            }
        });
        let key = format!(
            "{}|{}|{}|{}|{}|{}",
            self.snapshot.revision,
            self.query,
            self.filter,
            self.sort,
            self.descending,
            self.favorites
        );
        if key != self.filter_key {
            self.filtered = filter_items(
                &self.snapshot.items,
                &self.query,
                self.filter,
                self.favorites,
                self.sort,
                self.descending,
            );
            self.filter_key = key;
            self.selection
                .retain(|id| self.snapshot.items.iter().any(|i| &i.id == id));
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} {} · {} {}",
                self.filtered.len(),
                lang.text("kết quả", "results"),
                self.selection.len(),
                lang.text("đã chọn", "selected")
            ));
            if ui
                .small_button(lang.text("Chọn kết quả", "Select results"))
                .clicked()
            {
                self.selection.extend(
                    self.filtered
                        .iter()
                        .map(|&n| self.snapshot.items[n].id.clone()),
                );
            }
            if ui
                .small_button(lang.text("Bỏ chọn", "Clear selection"))
                .clicked()
            {
                self.selection.clear();
            }
            if !self.selection.is_empty() {
                if ui
                    .small_button(lang.text("Yêu thích", "Favorite"))
                    .clicked()
                {
                    self.send(Request::Favorite(
                        self.selection.iter().cloned().collect(),
                        true,
                    ));
                }
                if ui
                    .small_button(lang.text("Bỏ yêu thích", "Unfavorite"))
                    .clicked()
                {
                    self.send(Request::Favorite(
                        self.selection.iter().cloned().collect(),
                        false,
                    ));
                }
                if ui
                    .small_button(lang.text("Recycle Bin…", "Recycle Bin…"))
                    .clicked()
                {
                    self.trash = Some(self.selection.iter().cloned().collect());
                }
            }
        });
        if let Some(m) = &self.snapshot.message {
            ui.colored_label(Color32::from_rgb(255, 202, 134), m);
        }
        if let Some(id) = self.detail.clone() {
            if let Some(item) = self.snapshot.items.iter().find(|i| i.id == id).cloned() {
                egui::Frame::new()
                    .fill(Color32::from_rgb(26, 40, 57))
                    .corner_radius(6)
                    .inner_margin(10.)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(&item.name);
                            if ui
                                .small_button("×")
                                .on_hover_text(lang.text("Đóng chi tiết", "Close details"))
                                .clicked()
                            {
                                self.detail = None;
                            }
                        });
                        ui.label(RichText::new(item.path.to_string_lossy()).small());
                        if let Some(error) = &item.error {
                            ui.colored_label(Color32::LIGHT_YELLOW, error);
                        }
                        self.actions(ui, &item, lang, can_recover, &mut action);
                        if let Some(p) = &item.project {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Project:");
                                if ui
                                    .link(p.file_name().unwrap_or_default().to_string_lossy())
                                    .on_hover_text(p.to_string_lossy())
                                    .clicked()
                                {
                                    action = Some(Action::Edit(p.clone()));
                                }
                            });
                        }
                        if !item.sources.is_empty() {
                            ui.label(format!(
                                "{}: {}",
                                lang.text("Media nguồn", "Sources"),
                                item.sources.len()
                            ));
                        }
                        for p in &item.exports {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(lang.text("Bản xuất:", "Export:"));
                                if ui
                                    .link(p.file_name().unwrap_or_default().to_string_lossy())
                                    .clicked()
                                {
                                    open(p);
                                }
                            });
                        }
                    });
            }
        }
        ui.separator();
        if self.filtered.is_empty() {
            ui.label(lang.text(
                "Chưa có mục phù hợp. Thư viện tự cập nhật khi có file mới.",
                "No matching items. New files appear automatically.",
            ));
        }
        let cols = if self.grid {
            ((ui.available_width() + 12.) / 260.).floor().max(1.) as usize
        } else {
            1
        };
        let width = (ui.available_width() - (cols - 1) as f32 * 12.) / cols as f32;
        let row_height = if self.grid { 294. } else { 146. };
        let rows = self.filtered.len().div_ceil(cols);
        egui::ScrollArea::vertical()
            .id_salt("library-items")
            .auto_shrink([false, false])
            .show_rows(ui, row_height, rows, |ui, range| {
                for row in range {
                    ui.horizontal_top(|ui| {
                        for col in 0..cols {
                            let Some(&index) = self.filtered.get(row * cols + col) else {
                                break;
                            };
                            let item = self.snapshot.items[index].clone();
                            ui.allocate_ui_with_layout(
                                Vec2::new(width, row_height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(23, 32, 46))
                                        .stroke(egui::Stroke::new(
                                            1.,
                                            if self.selection.contains(&item.id) {
                                                Color32::LIGHT_BLUE
                                            } else {
                                                Color32::from_rgb(42, 54, 71)
                                            },
                                        ))
                                        .corner_radius(8)
                                        .inner_margin(10.)
                                        .show(ui, |ui| {
                                            ui.set_width(width - 20.);
                                            if self.grid {
                                                self.thumbnail(
                                                    ui,
                                                    &item,
                                                    Vec2::new(width - 20., 126.),
                                                );
                                            }
                                            ui.horizontal(|ui| {
                                                if !self.grid {
                                                    self.thumbnail(ui, &item, Vec2::new(112., 72.));
                                                }
                                                ui.vertical(|ui| {
                                                    ui.horizontal(|ui| {
                                                        let mut selected =
                                                            self.selection.contains(&item.id);
                                                        if ui
                                                            .checkbox(&mut selected, "")
                                                            .on_hover_text(
                                                                lang.text(
                                                                    "Chọn mục",
                                                                    "Select item",
                                                                ),
                                                            )
                                                            .changed()
                                                        {
                                                            if selected {
                                                                self.selection
                                                                    .insert(item.id.clone());
                                                            } else {
                                                                self.selection.remove(&item.id);
                                                            }
                                                        }
                                                        let name = if item.favorite {
                                                            format!("★ {}", item.name)
                                                        } else {
                                                            item.name.clone()
                                                        };
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new(name).strong(),
                                                                )
                                                                .frame(false)
                                                                .truncate(),
                                                            )
                                                            .on_hover_text(&item.name)
                                                            .clicked()
                                                        {
                                                            self.detail = Some(item.id.clone());
                                                        }
                                                    });
                                                    let meta = item
                                                        .metadata
                                                        .as_ref()
                                                        .map(|m| {
                                                            if item.kind == Kind::Image {
                                                                format!(
                                                                    "{} × {}",
                                                                    m.width, m.height
                                                                )
                                                            } else if item.kind == Kind::Project {
                                                                if crate::concat_bridge::is_project(
                                                                    &item.path,
                                                                ) {
                                                                    "Project Concat".into()
                                                                } else {
                                                                    "Project TaoClip (cũ / legacy)"
                                                                        .into()
                                                                }
                                                            } else {
                                                                format!(
                                                                    "{}:{:02} · {} × {}",
                                                                    m.duration as u64 / 60,
                                                                    m.duration as u64 % 60,
                                                                    m.width,
                                                                    m.height
                                                                )
                                                            }
                                                        })
                                                        .unwrap_or_else(|| "—".into());
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} · {}",
                                                            meta,
                                                            size_label(item.size)
                                                        ))
                                                        .small(),
                                                    );
                                                    ui.label(
                                                        RichText::new(date_label(item.created))
                                                            .small()
                                                            .color(Color32::from_rgb(
                                                                157, 174, 195,
                                                            )),
                                                    );
                                                });
                                            });
                                            ui.horizontal(|ui| {
                                                let (text, color) = state_label(&item.state, lang);
                                                ui.label(RichText::new(text).small().color(color));
                                                if item.is_export {
                                                    ui.label(
                                                        RichText::new(
                                                            lang.text("Bản xuất", "Export"),
                                                        )
                                                        .small(),
                                                    );
                                                }
                                                if item
                                                    .project
                                                    .as_deref()
                                                    .is_some_and(crate::concat_bridge::is_project)
                                                    && item.kind == Kind::Video
                                                {
                                                    ui.label(RichText::new("· Project").small());
                                                }
                                                ui.menu_button("…", |ui| {
                                                    self.actions(
                                                        ui,
                                                        &item,
                                                        lang,
                                                        can_recover,
                                                        &mut action,
                                                    );
                                                });
                                            });
                                        });
                                },
                            );
                            if col + 1 < cols {
                                ui.add_space(4.);
                            }
                        }
                    });
                }
            });
        if let Some((id, mut name)) = self.rename.clone() {
            egui::Window::new(lang.text("Đổi tên hiển thị", "Rename display title"))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(lang.text(
                        "Tên file media được giữ nguyên.",
                        "The media filename stays unchanged.",
                    ));
                    ui.text_edit_singleline(&mut name);
                    ui.horizontal(|ui| {
                        if ui.button(lang.text("Lưu", "Save")).clicked() {
                            self.send(Request::Rename(id.clone(), name.clone()));
                            self.rename = None;
                        }
                        if ui.button(lang.text("Hủy", "Cancel")).clicked() {
                            self.rename = None;
                        }
                    });
                });
            if let Some(v) = &mut self.rename {
                v.1 = name;
            }
        }
        if let Some(ids) = self.trash.clone() {
            egui::Window::new("Recycle Bin").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
                ui.label(format!("{} {}", ids.len(), lang.text("file sẽ được chuyển vào Recycle Bin.", "files will be moved to Recycle Bin.")));
                ui.label(lang.text("Project có thể mất media nếu bạn xóa file nguồn. Chỉ các mục đã chọn bị xóa.", "Projects may lose media if you delete source files. Only selected files are removed."));
                ui.horizontal(|ui| { if ui.button(lang.text("Chuyển vào Recycle Bin", "Move to Recycle Bin")).clicked() { self.send(Request::Trash(ids.clone())); self.trash = None; self.selection.clear(); }
 if ui.button(lang.text("Hủy", "Cancel")).clicked() { self.trash = None; } });
            });
        }
        action
    }
    fn actions(
        &mut self,
        ui: &mut egui::Ui,
        item: &Item,
        lang: Language,
        can_recover: bool,
        action: &mut Option<Action>,
    ) {
        ui.horizontal_wrapped(|ui| {
            if item.kind == Kind::Session {
                if ui
                    .add_enabled(
                        can_recover && item.state == State::Recovery,
                        egui::Button::new(lang.text("Phục hồi phiên", "Recover session")),
                    )
                    .clicked()
                {
                    *action = Some(Action::Recover(item.path.clone()));
                }
            } else {
                if ui
                    .add_enabled(
                        item.state != State::Missing && item.state != State::Finalizing,
                        egui::Button::new(lang.text("Mở file", "Open file")),
                    )
                    .clicked()
                {
                    if item.kind == Kind::Project {
                        *action = Some(Action::Edit(item.path.clone()));
                    } else {
                        open(&item.path);
                    }
                }
                if item.kind == Kind::Video || item.kind == Kind::Project {
                    let target = &item.path;
                    if ui
                        .add_enabled(
                            item.state != State::Finalizing
                                && (item.state != State::Missing
                                    || item
                                        .project
                                        .as_deref()
                                        .is_some_and(crate::concat_bridge::is_project)),
                            egui::Button::new(
                                if item
                                    .project
                                    .as_deref()
                                    .is_some_and(crate::concat_bridge::is_project)
                                    || item.kind == Kind::Project
                                {
                                    lang.text("Tiếp tục chỉnh sửa", "Continue editing")
                                } else {
                                    lang.text("Chỉnh sửa", "Edit")
                                },
                            ),
                        )
                        .clicked()
                    {
                        *action = Some(Action::Edit(target.clone()));
                    }
                }
                if ui.button(lang.text("Đổi tên", "Rename")).clicked() {
                    self.rename = Some((item.id.clone(), item.name.clone()));
                }
                if ui
                    .button(if item.favorite {
                        lang.text("Bỏ yêu thích", "Unfavorite")
                    } else {
                        lang.text("Yêu thích", "Favorite")
                    })
                    .clicked()
                {
                    self.send(Request::Favorite(vec![item.id.clone()], !item.favorite));
                }
                if item.state == State::Missing
                    && ui
                        .button(lang.text("Tìm lại file…", "Locate file…"))
                        .clicked()
                {
                    if let Some(p) = rfd::FileDialog::new().pick_file() {
                        self.send(Request::Relink(item.id.clone(), p));
                    }
                }
                if item.state == State::Recovery
                    && ui.button(lang.text("Đọc lại", "Retry")).clicked()
                {
                    self.send(Request::Retry(item.id.clone()));
                }
                if ui
                    .add_enabled(
                        item.state != State::Finalizing && item.state != State::Missing,
                        egui::Button::new("Recycle Bin…"),
                    )
                    .clicked()
                {
                    self.trash = Some(vec![item.id.clone()]);
                }
            }
            if ui.button(lang.text("Mở thư mục", "Open folder")).clicked() {
                if let Some(p) = item.path.parent() {
                    open(p);
                }
            }
        });
    }
}
fn open(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
}
fn size_label(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1e9)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1e6)
    } else if bytes >= 1000 {
        format!("{:.1} KB", bytes as f64 / 1000.)
    } else {
        format!("{bytes} B")
    }
}

fn date_label(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%d/%m/%Y %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}
fn state_label(state: &State, lang: Language) -> (&'static str, Color32) {
    match state {
        State::Ready => (
            lang.text("Sẵn sàng", "Ready"),
            Color32::from_rgb(132, 221, 182),
        ),
        State::Finalizing => (
            lang.text("Đang hoàn thiện", "Finalizing"),
            Color32::LIGHT_BLUE,
        ),
        State::Missing => (
            lang.text("Thiếu file", "Missing file"),
            Color32::from_rgb(255, 164, 164),
        ),
        State::Recovery => (
            lang.text("Cần phục hồi", "Needs recovery"),
            Color32::from_rgb(255, 207, 139),
        ),
    }
}
fn filter_items(
    items: &[Item],
    query: &str,
    filter: u8,
    favorites: bool,
    sort: u8,
    descending: bool,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut found = items
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            (!favorites || i.favorite)
                && i.name.to_lowercase().contains(&query)
                && match filter {
                    1 => i.kind == Kind::Video,
                    2 => i.kind == Kind::Image,
                    3 => i.kind == Kind::Project,
                    4 => i.is_export,
                    _ => true,
                }
        })
        .map(|(n, _)| n)
        .collect::<Vec<_>>();
    // Cache sort keys once, instead of allocating lowercase names in comparisons.
    match sort {
        1 => found.sort_by_cached_key(|&n| items[n].name.to_lowercase()),
        2 => found.sort_by_key(|&n| items[n].size),
        _ => found.sort_by_key(|&n| items[n].created),
    }
    if descending {
        found.reverse();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twenty_thousand_items_search_sort_and_virtualization() {
        let template: Item = serde_json::from_value(serde_json::json!({"id":"0", "path":"D:/fixture.mp4", "identity":null, "name":"", "favorite":false, "kind":"Video", "state":"Ready", "size":1, "modified":1, "created":1, "metadata":null,"thumbnail":null,"error":null})).unwrap();
        let items = (0..20_000)
            .map(|n| {
                let mut i = template.clone();
                i.id = n.to_string();
                i.name = format!("Bản quay thử {n:05}");
                i.size = n;
                i.created = n;
                i.favorite = n % 2 == 0;
                i
            })
            .collect::<Vec<_>>();
        let begin = std::time::Instant::now();
        let result = filter_items(&items, "BẢN QUAY THỬ 199", 1, false, 1, false);
        assert_eq!(result.len(), 100);
        assert_eq!(items[result[0]].name, "Bản quay thử 19900");
        let sorted = filter_items(&items, "", 1, true, 2, true);
        assert_eq!(sorted.len(), 10_000);
        assert_eq!(items[sorted[0]].size, 19998);
        println!("20k search + favorite filter + sort: {:?}", begin.elapsed());
        assert!(begin.elapsed().as_millis() < 500);
        let ctx = egui::Context::default();
        let mut rendered = 0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1000., 700.),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show_rows(ui, 146., 20_000, |ui, rows| {
                    for row in rows {
                        rendered += 1;
                        ui.label(&items[row].name);
                    }
                });
            });
        });
        assert!(rendered < 12, "Only visible rows should render: {rendered}");
        println!("20k virtual list: {rendered} rows rendered in 700px viewport");
        let mut samples = Vec::new();
        let mut furthest = 0;
        for page in 0..120 {
            let start = std::time::Instant::now();
            let mut count = 0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1000., 700.),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .vertical_scroll_offset(page as f32 * 20000.)
                        .show_rows(ui, 146., 20000, |ui, rows| {
                            for row in rows {
                                count += 1;
                                furthest = furthest.max(row);
                                ui.label(&items[row].name);
                            }
                        });
                });
            });
            assert!(count < 12);
            samples.push(start.elapsed().as_micros());
        }
        samples.sort();
        assert!(furthest > 10000);
        println!(
            "20k list / 120 scroll frames: p95 {} us; furthest row {}",
            samples[114], furthest
        );
    }
}
