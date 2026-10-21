//! Hardware integration check for a nonzero-offset screen region.
#![allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/capture.rs"]
mod capture;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/native.rs"]
mod native;
#[path = "../src/recorder.rs"]
mod recorder;
#[path = "../src/studio.rs"]
mod studio;

fn main() -> anyhow::Result<()> {
    use model::*;
    use std::time::{Duration, Instant};
    native::setup_dpi();
    let m = native::displays().into_iter().next().unwrap();
    let area = Area {
        x: m.area.x + 100,
        y: m.area.y + 80,
        width: 640,
        height: 360,
    };
    anyhow::ensure!(area.fits(m.area), "Display too small for integration check");
    let prefs = Preferences {
        system_audio: false,
        output: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output"),
        ..Default::default()
    };
    let control = recorder::start(RecordingOptions {
        source: Source::Monitor {
            handle: m.handle,
            bounds: m.area,
            region: area,
        },
        prefs,
    });
    let mut begin = None;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Ok(event) = control.events.recv_timeout(Duration::from_millis(50)) {
            match event {
                Event::Recording { .. } => begin = Some(Instant::now()),
                Event::Saved(path) => {
                    let output = std::process::Command::new(recorder::tool("ffprobe")?)
                        .args([
                            "-v",
                            "error",
                            "-select_streams",
                            "v:0",
                            "-show_entries",
                            "stream=width,height",
                            "-of",
                            "json",
                        ])
                        .arg(&path)
                        .output()?;
                    let info: serde_json::Value = serde_json::from_slice(&output.stdout)?;
                    anyhow::ensure!(
                        info["streams"][0]["width"] == 640 && info["streams"][0]["height"] == 360,
                        "Incorrect crop size"
                    );
                    println!(
                        "PASS nonzero-offset region (100,80) 640x360: {}",
                        path.display()
                    );
                    break;
                }
                Event::Failed(e) => anyhow::bail!(e),
                _ => {}
            }
        }
        if begin.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) {
            control.commands.send(Command::Stop)?;
            begin = None;
        }
        anyhow::ensure!(Instant::now() < deadline, "Region capture timed out");
    }
    Ok(())
}
