//! Hardware acceptance: excluded floating controls + burned-in annotations.
#![allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/capture.rs"]
mod capture;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/native.rs"]
mod native;
#[path = "../src/overlay.rs"]
mod overlay;
#[path = "../src/recorder.rs"]
mod recorder;
#[path = "../src/studio.rs"]
mod studio;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use windows::{
    core::w,
    Win32::{
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
};
unsafe extern "system" fn scene_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(h, m, w, l)
}
struct Scene(Arc<AtomicBool>);
impl Drop for Scene {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
fn backdrop(a: model::Area) -> Scene {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || unsafe {
        let module = GetModuleHandleW(None).unwrap();
        let brush = CreateSolidBrush(COLORREF(0x00462d18));
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(scene_proc),
            hInstance: module.into(),
            lpszClassName: w!("StudioAcceptanceScene"),
            hbrBackground: brush,
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("StudioAcceptanceScene"),
            w!("Recorder acceptance backdrop"),
            WS_POPUP | WS_VISIBLE,
            a.x,
            a.y,
            a.width as i32,
            a.height as i32,
            None,
            None,
            Some(module.into()),
            None,
        )
        .unwrap();
        let _ = tx.send(());
        let mut msg = MSG::default();
        while !flag.load(Ordering::Relaxed) {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = DestroyWindow(hwnd);
        let _ = UnregisterClassW(w!("StudioAcceptanceScene"), Some(module.into()));
        let _ = DeleteObject(brush.into());
    });
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    Scene(stop)
}
fn main() -> anyhow::Result<()> {
    native::setup_dpi();
    let m = native::displays().into_iter().next().unwrap();
    let area = model::Area {
        x: m.area.x + 40,
        y: m.area.y + 60,
        width: m.area.width.min(1000) - 80,
        height: m.area.height.min(640) - 100,
    };
    let _scene = backdrop(area);
    let shared = Arc::new(Mutex::new(studio::Live::default()));
    shared.lock().unwrap().area = area;
    let source = model::Source::Monitor {
        handle: m.handle,
        bounds: m.area,
        region: area,
    };
    let mut overlay = Some(overlay::spawn(shared.clone(), area)); // Deliberately put toolbar INSIDE capture.
    std::thread::sleep(Duration::from_millis(600));
    if let Some(e) = shared.lock().unwrap().fault.clone() {
        anyhow::bail!(e)
    }
    let mut bar_rect = RECT::default();
    unsafe {
        let bar = FindWindowW(None, w!("Screen Recorder • Điều khiển"))?;
        anyhow::ensure!(IsWindowVisible(bar).as_bool(), "Toolbar not visible");
        GetWindowRect(bar, &mut bar_rect)?;
        let mut affinity = 0;
        GetWindowDisplayAffinity(bar, &mut affinity)?;
        anyhow::ensure!(
            affinity == WDA_EXCLUDEFROMCAPTURE.0,
            "Toolbar exclusion not set"
        );
    }
    let screen = capture::ScreenSession::start(&source, false, 30)?;
    std::thread::sleep(Duration::from_millis(300));
    let raw = screen.latest.lock().unwrap().image.clone().unwrap();
    studio::save_png(
        &raw,
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("test-output/studio-raw-check.png"),
    )?;
    // Compare the actual control rectangle. A separate computer-use pointer
    // indicator may legitimately be present elsewhere on this desktop.
    let mut unexpected = 0usize;
    for y in (bar_rect.top - area.y).max(0)..(bar_rect.bottom - area.y).min(area.height as i32) {
        for x in (bar_rect.left - area.x).max(0)..(bar_rect.right - area.x).min(area.width as i32) {
            let i = (y as usize * raw.width as usize + x as usize) * 4;
            if raw.pixels[i..i + 3]
                .iter()
                .zip([70u8, 45, 24])
                .any(|(a, b)| a.abs_diff(b) > 3)
            {
                unexpected += 1;
            }
        }
    }
    anyhow::ensure!(
        unexpected == 0,
        "Toolbar leaked: {unexpected} pixels in its rectangle"
    );
    let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output/studio-v2");
    std::fs::create_dir_all(&output)?;
    studio::save_png(&raw, &output.join("excluded-toolbar-raw.png"))?;
    {
        let mut l = shared.lock().unwrap();
        l.marks.push(studio::Mark {
            tool: studio::Tool::Rectangle,
            points: vec![(80., 80.), (340., 240.)],
            color: [255, 193, 64],
            width: 6.,
            text: String::new(),
        });
        l.marks.push(studio::Mark {
            tool: studio::Tool::Text,
            points: vec![(100., 100.)],
            color: [255, 255, 255],
            width: 5.,
            text: "Ghi chú tiếng Việt".into(),
        });
        l.marks.push(studio::Mark {
            tool: studio::Tool::Highlight,
            points: vec![(100., 175.), (300., 175.)],
            color: [255, 193, 64],
            width: 5.,
            text: String::new(),
        });
        l.changed();
    }
    let composed = studio::composite(&raw, &shared);
    anyhow::ensure!(composed.as_slice() != &**raw.pixels, "Annotations missing");
    studio::save_png(
        &capture::Image::from_bgra(raw.width, raw.height, composed),
        &output.join("screenshot-annotations.png"),
    )?;
    drop(screen);
    let want_audio = std::env::args().any(|s| s == "--audio");
    let want_mic = std::env::args().any(|s| s == "--mic");
    let prefs = model::Preferences {
        output: output.clone(),
        system_audio: want_audio,
        microphone: want_mic,
        cursor: false,
        ..Default::default()
    };
    let controller = recorder::start_live(
        model::RecordingOptions { source, prefs },
        Some(shared.clone()),
    );
    let begin = Instant::now();
    let mut phase = None;
    let mut paused = false;
    let mut changed = false;
    let mut stopped = false;
    loop {
        if let Ok(event) = controller.events.recv_timeout(Duration::from_millis(30)) {
            println!("{event:?}");
            match event {
                model::Event::Recording { .. } => {
                    phase = Some(Instant::now());
                    shared.lock().unwrap().status = studio::Status::Recording;
                }
                model::Event::Progress { seconds, .. } => shared.lock().unwrap().seconds = seconds,
                model::Event::Paused => {
                    shared.lock().unwrap().status = studio::Status::Paused;
                    std::thread::sleep(Duration::from_millis(300));
                    controller.commands.send(model::Command::Resume)?;
                }
                model::Event::Saved(path) => {
                    std::fs::write(
                        output.join("acceptance.json"),
                        serde_json::to_vec_pretty(
                            &serde_json::json!({"video":path,"toolbar_visible_and_excluded":true,"unexpected_toolbar_pixels":unexpected,"annotations":true,"pause_resume":true,"live_audio_mute":want_audio}),
                        )?,
                    )?;
                    break;
                }
                model::Event::Failed(e) => anyhow::bail!(e),
                _ => {}
            }
        }
        if let Some(t) = phase {
            let secs = t.elapsed().as_secs_f64();
            if secs > 5.0 && want_mic {
                shared.lock().unwrap().mic_on = true;
            }
            if !paused {
                shared.lock().unwrap().system_on = !(2.0..4.0).contains(&secs);
            }
            if secs > 1.0 && !changed {
                let mut l = shared.lock().unwrap();
                l.marks.push(studio::Mark {
                    tool: studio::Tool::Arrow,
                    points: vec![(400., 120.), (600., 220.)],
                    color: [255, 82, 102],
                    width: 6.,
                    text: String::new(),
                });
                l.changed();
                changed = true;
            }
            if secs >= 6.0 && !paused {
                controller.commands.send(model::Command::Pause)?;
                phase = None;
                paused = true;
            } else if secs >= 3.0 && paused && !stopped {
                controller.commands.send(model::Command::Stop)?;
                phase = None;
                stopped = true;
            }
        }
        if begin.elapsed() > Duration::from_secs(90) {
            anyhow::bail!("Acceptance timed out")
        }
    }
    overlay.take();
    println!("Studio acceptance passed");
    Ok(())
}
