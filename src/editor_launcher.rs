//! Launch the bundled Concat editor in a supervised background process.
use crate::model::Language;
use anyhow::{bail, Context, Result};
use eframe::egui;
use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

fn validate_video(path: &Path) -> Result<PathBuf> {
    let path = if path.is_dir() {
        path.join("concat.json")
    } else {
        path.to_path_buf()
    };
    if !path.is_file() {
        bail!(
            "Video/project không tồn tại / File not found: {}",
            path.display()
        );
    }
    if crate::concat_bridge::legacy(&path) {
        bail!("Project TaoClip cũ được giữ nguyên. Hãy dùng TaoClip để mở .taoclip, hoặc chọn video gốc để chỉnh sửa bằng Concat. / Legacy project preserved; choose the original video for Concat.");
    }
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !crate::concat_bridge::is_project(&path)
        && !["mp4", "mkv", "webm", "mov", "avi", "m4v", "ts"].contains(&extension.as_str())
    {
        bail!(
            "Định dạng không hỗ trợ / Unsupported file: {}",
            path.display()
        );
    }
    Ok(std::path::absolute(path)?)
}
pub fn launch(video: Option<&Path>) -> Result<Child> {
    spawn_session(
        video,
        &crate::library::data_dir()
            .join("concat-launches")
            .join(format!("{}.json", crate::concat_bridge::unique())),
    )
}
fn spawn_session(video: Option<&Path>, status: &Path) -> Result<Child> {
    crate::concat_bridge::binary()?;
    let video = video
        .map(|p| {
            let (mut db, _) = crate::library::Index::load(&crate::library::data_dir());
            db.consume_events(&crate::library::data_dir());
            let target = db
                .items
                .iter()
                .find(|i| crate::library::path_key(&i.path) == crate::library::path_key(p))
                .and_then(|i| i.project.as_deref())
                .filter(|p| crate::concat_bridge::is_project(p))
                .unwrap_or(p);
            validate_video(target)
        })
        .transpose()?;
    let mut c = Command::new(std::env::current_exe()?);
    c.arg("--concat-session").arg(status);
    if let Some(video) = video {
        c.arg(video);
    }
    c.creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Không mở được Concat / Could not start Concat")
}

#[derive(Default)]
pub struct EditorLauncher {
    last_video: Option<PathBuf>,
    child: Option<Child>,
    error: Option<String>,
    opened: bool,
    language: Language,
    status: Option<PathBuf>,
}

impl EditorLauncher {
    pub fn set_language(&mut self, lang: Language) {
        self.language = lang;
    }
    pub fn child_id(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }
    pub fn is_opened(&self) -> bool {
        self.opened
    }
    pub fn show_error(&mut self, ui: &mut egui::Ui, _lang: Language) {
        self.poll();
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    pub fn open(&mut self, path: PathBuf) {
        self.last_video = Some(path.clone());
        self.start(Some(&path));
    }

    fn start(&mut self, video: Option<&Path>) {
        self.error = None;
        self.opened = false;
        self.status = None;
        self.child = None;
        let status = crate::library::data_dir()
            .join("concat-launches")
            .join(format!("{}.json", crate::concat_bridge::unique()));
        match spawn_session(video, &status) {
            Ok(child) => {
                self.child = Some(child);
                self.status = Some(status);
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }
    fn poll(&mut self) {
        if let Some(status) = &self.status {
            if let Ok(bytes) = std::fs::read(status) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Some(error) = value["error"].as_str() {
                        self.error = Some(error.into());
                        self.opened = false;
                    } else {
                        self.opened = true;
                    }
                }
            }
        }
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(exit)) = child.try_wait() {
                if !exit.success() && self.error.is_none() {
                    self.error = Some("Không mở được Concat. Xem nhật ký trong thư viện. / Concat failed; see library logs.".into());
                }
                self.child = None;
            }
        }
    }

    pub fn ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, output: &Path, lang: Language) {
        self.poll();
        ui.heading("Concat Editor");
        ui.label(lang.text(
            "Dựng video trong cửa sổ riêng với timeline nhiều lớp, chữ, âm thanh và chuyển cảnh.",
            "Edit in a separate window with multiple tracks, text, audio and transitions.",
        ));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui
                .button(lang.text("Chọn video để chỉnh sửa…", "Choose video to edit…"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(output)
                    .add_filter(
                        "Video",
                        &["mp4", "mkv", "webm", "mov", "avi", "m4v", "ts", "json"],
                    )
                    .pick_file()
                {
                    self.open(path);
                }
            }
            if ui
                .button(lang.text("Mở editor trống", "Open empty editor"))
                .clicked()
            {
                self.start(None);
            }
        });
        if let Some(path) = self.last_video.clone() {
            ui.add_space(12.0);
            ui.label(path.display().to_string());
            if ui
                .button(lang.text("Trở lại cửa sổ chỉnh sửa", "Return to editor"))
                .clicked()
            {
                self.start(Some(&path));
            }
        }
        if let Some(error) = &self.error {
            ui.add_space(12.0);
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        } else if self.child.is_some() && !self.opened {
            ui.spinner();
            ui.label(lang.text(
                "Đang chuẩn bị timeline và mở Concat…",
                "Preparing timeline and opening Concat…",
            ));
            _ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else if self.opened {
            ui.add_space(12.0);
            ui.label(lang.text("Đã gửi yêu cầu mở editor. Video được đưa tự động vào timeline và giữ tỷ lệ gốc.", "Editor launch requested. The video is imported into the timeline at its original aspect ratio."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_and_non_video_files() {
        assert!(validate_video(Path::new("missing-recording.mp4")).is_err());
        assert!(validate_video(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("Cargo.toml")
                .as_path()
        )
        .is_err());
    }

    #[test]
    fn keeps_unicode_and_spaces_in_video_path() {
        let dir = std::env::temp_dir().join(format!("recorder-launch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Bản quay thử 01.MP4");
        std::fs::write(&file, b"fixture").unwrap();
        assert_eq!(validate_video(&file).unwrap(), file);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
