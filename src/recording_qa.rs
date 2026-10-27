//! Opt-in acceptance driver. Uses the real App handlers and recorder events.
//! It only records the deterministic `Recorder Test Pattern` window.
use super::*;
impl App {
    pub(super) fn qa_tick(&mut self, ctx: &egui::Context) {
        if !std::env::args().any(|a| a == "--ui-smoke") {
            return;
        }
        if self.qa_stage == 255 {
            return;
        }
        let Ok(folder) = std::env::var("SCREENRECORDER_QA_OUTPUT") else {
            return;
        };
        let folder = PathBuf::from(folder);
        let scenario =
            std::env::var("SCREENRECORDER_QA_SCENARIO").unwrap_or_else(|_| "quick".into());
        let phase = format!("{:?}", self.phase);
        if self.qa_trace.last() != Some(&phase) {
            self.qa_trace.push(phase);
            self.qa_capture_at = Instant::now() - Duration::from_secs(3);
        }
        if self.qa_stage == 0 && self.inventory_ready {
            let Some(index) = self
                .windows
                .iter()
                .position(|w| w.name == "Recorder Test Pattern")
            else {
                return;
            };
            self.mode = 2;
            self.window = index;
            self.prefs.system_audio = false;
            self.prefs.microphone = false;
            self.prefs.edit_after_recording = scenario == "edit";
            QualityPreset::Balanced.apply(&mut self.prefs);
            self.prefs.encoder = "cpu".into();
            self.prefs.output = folder.join("recordings");
            if scenario != "manual" {
                self.error = None;
            }
            self.qa_clock = Instant::now();
            self.qa_stage = 1;
            if scenario == "disconnected" {
                self.prefs.microphone = true;
                self.prefs.mic_device = "qa-disconnected-device".into();
            }
            if scenario == "region" {
                // Only stage the region controls here; no desktop recording.
                self.mode = 0;
                self.monitor = 0;
                if let Some(m) = self.monitors.first() {
                    self.set_region(Area {
                        x: m.area.x + 32,
                        y: m.area.y + 80,
                        width: 640,
                        height: 360,
                    });
                }
            }
        }
        if scenario == "manual" {
            let _=std::fs::write(folder.join("trace.json"),serde_json::to_vec_pretty(&serde_json::json!({"states":self.qa_trace,"video":self.last_file,"error":self.error})).unwrap());
            return;
        }
        let elapsed = self.qa_clock.elapsed().as_secs_f32();
        match self.qa_stage {
            1 if elapsed > 4. => {
                if ["ready", "region"].contains(&scenario.as_str()) {
                    self.qa_finish(ctx, &folder);
                    return;
                }
                self.begin();
                self.qa_stage = 2;
            }
            2 if scenario == "disconnected" && self.error.is_some() => {
                self.qa_stage = 6;
                self.qa_clock = Instant::now();
            }
            2 if self.phase == Phase::Recording && self.seconds >= 1.2 => {
                if scenario == "source-loss" {
                    let _ = std::fs::write(folder.join("close-source.flag"), b"close test pattern");
                    self.qa_stage = 6;
                    self.qa_clock = Instant::now();
                } else {
                    self.pause_resume(ctx);
                    self.qa_stage = 3;
                    self.qa_clock = Instant::now();
                }
            }
            3 if self.phase == Phase::Paused && elapsed > 2. => {
                self.pause_resume(ctx);
                self.qa_stage = 4;
            }
            4 if self.phase == Phase::Recording && self.seconds >= 3.5 => {
                self.stop();
                self.qa_stage = 5;
                self.qa_clock = Instant::now();
            }
            5 if self.phase == Phase::Ready && self.last_file.is_some() && elapsed > 12. => {
                self.qa_finish(ctx, &folder);
            }
            6 if self.phase == Phase::Ready && elapsed > 5. => {
                self.qa_finish(ctx, &folder);
            }
            _ => {}
        }
        if self.error.is_some()
            && self.qa_stage > 2
            && self.qa_stage < 6
            && self.phase == Phase::Ready
        {
            self.qa_stage = 6;
            self.qa_clock = Instant::now();
        }
        if elapsed > 55. {
            self.error = Some("UI acceptance timed out".into());
            self.stop();
            self.qa_finish(ctx, &folder);
        }
    }
    fn qa_finish(&mut self, ctx: &egui::Context, folder: &std::path::Path) {
        let report = serde_json::json!({"states":self.qa_trace,"video":self.last_file,"error":self.error,"edit_requested":self.prefs.edit_after_recording,"editor_opened":self.editor.is_opened(),"editor_pid":self.editor.child_id(),"stage":self.qa_stage,"pixels_per_point":ctx.pixels_per_point(),"viewport":format!("{:?}",ctx.screen_rect())});
        let _ = std::fs::create_dir_all(folder);
        let _ = std::fs::write(
            folder.join("report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        );
        self.qa_stage = 255;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
