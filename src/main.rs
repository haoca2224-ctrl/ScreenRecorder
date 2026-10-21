#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod audio;
mod capture;
mod editor;
mod model;
mod native;
mod overlay;
mod recorder;
mod studio;
mod test_pattern;

fn main() {
    native::setup_dpi();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--test-pattern") {
        test_pattern::run();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--recover") {
        let result = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Thiếu đường dẫn phiên ghi"))
            .and_then(|p| recorder::recover(std::path::Path::new(p)));
        match result {
            Ok(p) => println!("{}", p.display()),
            Err(e) => {
                eprintln!("{e:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--smoke-test") {
        if let Err(e) = smoke(&args) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--editor-smoke") {
        let result = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Thiếu video đầu vào"))
            .and_then(|input| {
                let output = args
                    .get(3)
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| {
                        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("test-output/editor-smoke.mp4")
                    });
                editor::smoke_export(std::path::Path::new(input), &output)
            });
        match result {
            Ok(path) => println!("OUTPUT={}", path.display()),
            Err(error) => {
                eprintln!("{error:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--toolbar-snapshot") {
        let display = native::displays().into_iter().next().expect("display");
        let mut state = studio::Live::default();
        state.area = model::Area {
            x: display.area.x + 80,
            y: display.area.y + 180,
            width: display.area.width.min(960),
            height: display.area.height.min(540),
        };
        state.status = studio::Status::Recording;
        state.seconds = 83.0;
        state.system_on = true;
        state.mic_on = true;
        state.levels = [0.64, 0.42];
        let live = std::sync::Arc::new(std::sync::Mutex::new(state));
        let _overlay = overlay::spawn(live, display.area);
        std::thread::sleep(std::time::Duration::from_secs(2));
        return;
    }
    if args.get(1).map(String::as_str) == Some("--toolbar-dismiss-test") {
        let display = native::displays().into_iter().next().expect("display");
        let mut state = studio::Live::default();
        state.area = model::Area {
            x: display.area.x + 80,
            y: display.area.y + 180,
            width: display.area.width.min(960),
            height: display.area.height.min(540),
        };
        let live = std::sync::Arc::new(std::sync::Mutex::new(state));
        let overlay = overlay::spawn(live, display.area);
        std::thread::sleep(std::time::Duration::from_millis(500));
        unsafe {
            use windows::{
                core::w,
                Win32::{
                    Foundation::LPARAM,
                    UI::{Input::KeyboardAndMouse::VK_ESCAPE, WindowsAndMessaging::*},
                },
            };
            let toolbar =
                FindWindowW(w!("RecorderStudioToolbar"), None).expect("floating toolbar window");
            SendMessageW(
                toolbar,
                WM_KEYDOWN,
                Some(windows::Win32::Foundation::WPARAM(VK_ESCAPE.0 as usize)),
                Some(LPARAM(0)),
            );
        }
        match overlay
            .actions
            .recv_timeout(std::time::Duration::from_secs(2))
        {
            Ok(studio::Action::Dismiss) => println!("TOOLBAR_DISMISS=OK"),
            _ => panic!("Escape did not dismiss the ready toolbar"),
        }
        return;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 650.0])
            .with_position([24.0, 16.0])
            .with_min_inner_size([900.0, 540.0])
            .with_icon(app_icon())
            .with_title("Screen Recorder"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Screen Recorder",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    ) {
        let _ = rfd::MessageDialog::new()
            .set_title("Screen Recorder")
            .set_description(e.to_string())
            .show();
    }
}

fn app_icon() -> eframe::egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/app-icon-128.png"))
        .expect("embedded application icon")
        .to_rgba8();
    eframe::egui::IconData {
        rgba: image.into_raw(),
        width: 128,
        height: 128,
    }
}

fn smoke(args: &[String]) -> anyhow::Result<()> {
    use model::*;
    use std::time::{Duration, Instant};
    let m = native::displays()
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Không có màn hình"))?;
    let seconds = args
        .get(2)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(6.0);
    let mut prefs = Preferences {
        output: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output"),
        ..Default::default()
    };
    if args.iter().any(|s| s == "--mute") {
        prefs.system_audio = false;
    }
    if args.iter().any(|s| s == "--mic") {
        prefs.microphone = true;
    }
    if args.iter().any(|s| s == "--cpu") {
        prefs.encoder = "cpu".into();
    }
    let region = Area {
        width: m.area.width.min(1280),
        height: m.area.height.min(720),
        ..m.area
    };
    let source = if let Some(i) = args.iter().position(|s| s == "--hwnd") {
        Source::Window {
            handle: args.get(i + 1).unwrap().parse()?,
        }
    } else {
        Source::Monitor {
            handle: m.handle,
            bounds: m.area,
            region,
        }
    };
    let control = recorder::start(RecordingOptions { source, prefs });
    let started = Instant::now();
    let mut phase_start = Instant::now();
    let mut recording = false;
    let mut did_pause = false;
    let want_pause = args.iter().any(|s| s == "--pause");
    loop {
        if let Ok(event) = control.events.recv_timeout(Duration::from_millis(100)) {
            println!("{event:?}");
            match event {
                Event::Recording { .. } => {
                    phase_start = Instant::now();
                    recording = true;
                }
                Event::Paused => {
                    std::thread::sleep(Duration::from_millis(700));
                    control.commands.send(Command::Resume)?;
                }
                Event::Saved(path) => {
                    println!("OUTPUT={}", path.display());
                    break;
                }
                Event::Failed(e) => anyhow::bail!(e),
                _ => {}
            }
        }
        if recording && phase_start.elapsed().as_secs_f64() >= seconds {
            recording = false;
            if want_pause && !did_pause {
                control.commands.send(Command::Pause)?;
                did_pause = true;
            } else {
                control.commands.send(Command::Stop)?;
            }
        }
        if started.elapsed() > Duration::from_secs_f64(seconds * 3.0 + 100.0) {
            anyhow::bail!("Smoke test timed out");
        }
    }
    Ok(())
}
