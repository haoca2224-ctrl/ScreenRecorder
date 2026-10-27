use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Language {
    #[default]
    Vi,
    En,
}

impl Language {
    pub fn text(self, vi: &'static str, en: &'static str) -> &'static str {
        match self {
            Self::Vi => vi,
            Self::En => en,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    pub fn from_points(a: (i32, i32), b: (i32, i32)) -> Self {
        Self {
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            width: a.0.abs_diff(b.0) & !1,
            height: a.1.abs_diff(b.1) & !1,
        }
    }

    pub fn fits(&self, bounds: Self) -> bool {
        self.width >= 16
            && self.height >= 16
            && self.x >= bounds.x
            && self.y >= bounds.y
            && self.x as i64 + self.width as i64 <= bounds.x as i64 + bounds.width as i64
            && self.y as i64 + self.height as i64 <= bounds.y as i64 + bounds.height as i64
    }
}

#[derive(Clone, Debug)]
pub enum Source {
    Monitor {
        handle: isize,
        bounds: Area,
        region: Area,
    },
    Window {
        handle: isize,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: Language,
    pub system_audio: bool,
    pub microphone: bool,
    pub system_volume: f32,
    pub mic_volume: f32,
    pub cursor: bool,
    pub fps: u32,
    pub max_height: u32,
    pub bitrate: u32,
    pub output: PathBuf,
    pub encoder: String,
    pub system_device: String,
    pub mic_device: String,
    pub cursor_highlight: bool,
    pub click_effect: bool,
    pub edit_after_recording: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        let root = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_owned()))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            language: Language::Vi,
            system_audio: true,
            microphone: false,
            system_volume: 1.0,
            mic_volume: 1.0,
            cursor: true,
            fps: 30,
            max_height: 0,
            bitrate: 8,
            output: root.join("recordings"),
            encoder: "auto".into(),
            system_device: String::new(),
            mic_device: String::new(),
            cursor_highlight: false,
            click_effect: false,
            edit_after_recording: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityPreset {
    Light,
    Balanced,
    High,
}
impl QualityPreset {
    pub fn apply(self, prefs: &mut Preferences) {
        prefs.fps = 30;
        prefs.encoder = "auto".into();
        (prefs.max_height, prefs.bitrate) = match self {
            Self::Light => (720, 4),
            Self::Balanced => (1080, 8),
            Self::High => (0, 16),
        };
    }
    pub fn matches(self, prefs: &Preferences) -> bool {
        let mut expected = prefs.clone();
        self.apply(&mut expected);
        (prefs.fps, prefs.max_height, prefs.bitrate, &prefs.encoder)
            == (
                expected.fps,
                expected.max_height,
                expected.bitrate,
                &expected.encoder,
            )
    }
}

#[derive(Clone)]
pub struct RecordingOptions {
    pub source: Source,
    pub prefs: Preferences,
}

#[derive(Debug)]
pub enum Command {
    Pause,
    Resume,
    Stop,
}

#[derive(Debug)]
pub enum Event {
    Starting,
    Recording { encoder: String },
    Paused,
    Finishing,
    Progress { seconds: f64, bytes: u64 },
    Saved(PathBuf),
    SavedWithWarning { path: PathBuf, warning: String },
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_apply_encoder_dimensions_rate_and_preserve_workflow() {
        let mut prefs = Preferences {
            encoder: "cpu".into(),
            edit_after_recording: true,
            ..Default::default()
        };
        for (preset, height, rate) in [
            (QualityPreset::Light, 720, 4),
            (QualityPreset::Balanced, 1080, 8),
            (QualityPreset::High, 0, 16),
        ] {
            preset.apply(&mut prefs);
            assert_eq!(
                (prefs.max_height, prefs.bitrate, prefs.fps),
                (height, rate, 30)
            );
            assert!(preset.matches(&prefs));
            assert!(prefs.edit_after_recording);
            prefs.encoder = "cpu".into();
            assert!(!preset.matches(&prefs));
        }
        let legacy: Preferences = serde_json::from_str(r#"{"bitrate":12}"#).unwrap();
        assert!(!legacy.edit_after_recording);
        assert_eq!(legacy.bitrate, 12);
    }
    #[test]
    fn selection_handles_reverse_drag_and_negative_monitor_origin() {
        let a = Area::from_points((-10, 501), (-809, 100));
        assert_eq!(
            a,
            Area {
                x: -809,
                y: 100,
                width: 798,
                height: 400
            }
        );
        assert!(a.fits(Area {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080
        }));
        assert!(!a.fits(Area {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }));
    }
}
