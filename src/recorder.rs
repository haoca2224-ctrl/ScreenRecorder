use crate::{
    audio::{self, AudioSession},
    capture::ScreenSession,
    model::*,
};
use anyhow::{bail, Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command as Process, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

pub fn tool(name: &str) -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let local = exe.parent().unwrap().join(format!("{name}.exe"));
    if local.exists() {
        return Ok(local);
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join(format!("{name}.exe"));
    if dev.exists() {
        return Ok(dev);
    }
    bail!("Thiếu {name}.exe. Hãy giải nén đầy đủ bộ portable.")
}
fn process(name: &str) -> Result<Process> {
    let mut c = Process::new(tool(name)?);
    c.creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    Ok(c)
}
fn wait(child: &mut Child, timeout: Duration) -> Result<std::process::ExitStatus> {
    let begin = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if begin.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("FFmpeg không phản hồi trong {} giây.", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn run_logged(mut c: Process, log: &Path, timeout: Duration) -> Result<()> {
    c.stderr(Stdio::from(
        OpenOptions::new().create(true).append(true).open(log)?,
    ));
    let mut child = c.spawn().context("Không chạy được FFmpeg")?;
    let status = wait(&mut child, timeout)?;
    if !status.success() {
        bail!("FFmpeg thất bại ({status}). Nhật ký: {}", log.display());
    }
    Ok(())
}
fn output_size(width: u32, height: u32, limit: u32) -> (u32, u32) {
    if limit == 0 || height <= limit {
        (width & !1, height & !1)
    } else {
        (
            ((width as u64 * limit as u64 / height as u64) as u32 & !1).max(16),
            limit & !1,
        )
    }
}
pub fn select_encoder(prefs: &Preferences, size: (u32, u32), log: &Path) -> Result<String> {
    let options: &[&str] = match prefs.encoder.as_str() {
        "cpu" => &["libx264"],
        "qsv" => &["h264_qsv"],
        _ => &["h264_qsv", "libx264"],
    };
    for &encoder in options {
        let mut c = process("ffmpeg")?;
        c.args(["-hide_banner", "-loglevel", "warning", "-f", "lavfi", "-i"])
            .arg(format!(
                "color=size={}x{}:rate={}",
                size.0, size.1, prefs.fps
            ))
            .args([
                "-frames:v",
                "3",
                "-vf",
                "format=nv12",
                "-c:v",
                encoder,
                "-b:v",
            ])
            .arg(format!("{}M", prefs.bitrate))
            .args(["-f", "null", "-"]);
        if run_logged(c, log, Duration::from_secs(15)).is_ok() {
            return Ok(encoder.into());
        }
    }
    bail!("Không khởi tạo được bộ mã hóa. Xem {}.", log.display())
}

struct Encoder {
    child: Child,
    heartbeat: Arc<AtomicU64>,
    watch_stop: Arc<AtomicBool>,
    born: Instant,
}
impl Encoder {
    fn start(
        path: &Path,
        size: (u32, u32),
        prefs: &Preferences,
        encoder: &str,
        log: &Path,
    ) -> Result<Self> {
        let out = output_size(size.0, size.1, prefs.max_height);
        let mut c = process("ffmpeg")?;
        c.stdin(Stdio::piped())
            .stderr(Stdio::from(File::create(log)?))
            .args([
                "-hide_banner",
                "-loglevel",
                "warning",
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "bgra",
                "-video_size",
            ])
            .arg(format!("{}x{}", size.0, size.1))
            .arg("-framerate")
            .arg(prefs.fps.to_string())
            .args(["-i", "pipe:0", "-an", "-vf"])
            .arg(format!(
                "scale={}:{}:flags=fast_bilinear,format=nv12",
                out.0, out.1
            ))
            .args(["-c:v", encoder, "-b:v"])
            .arg(format!("{}M", prefs.bitrate))
            .arg("-g")
            .arg((prefs.fps * 2).to_string())
            .args(["-bf", "0"]);
        if encoder == "libx264" {
            c.args(["-preset", "veryfast", "-threads", "4"]);
        } else {
            c.args(["-preset", "veryfast", "-look_ahead", "0"]);
        }
        c.args(["-flush_packets", "1", "-f", "matroska"]).arg(path);
        let child = c.spawn()?;
        let pid = child.id();
        let heartbeat = Arc::new(AtomicU64::new(0));
        let watch_stop = Arc::new(AtomicBool::new(false));
        let (beat, stop, born) = (heartbeat.clone(), watch_stop.clone(), Instant::now());
        // If a driver stalls FFmpeg while stdin is blocked, terminate only our
        // encoder process so Stop can complete and the recovery data survives.
        std::thread::spawn(move || {
            use windows::Win32::{Foundation::CloseHandle, System::Threading::*};
            unsafe {
                let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) else {
                    return;
                };
                while !stop.load(Ordering::Relaxed) {
                    if born.elapsed().as_millis() as u64 > beat.load(Ordering::Relaxed) + 15_000 {
                        let _ = TerminateProcess(handle, 124);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
                let _ = CloseHandle(handle);
            }
        });
        Ok(Self {
            child,
            heartbeat,
            watch_stop,
            born,
        })
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<()> {
        self.heartbeat
            .store(self.born.elapsed().as_millis() as u64, Ordering::Relaxed);
        self.child
            .stdin
            .as_mut()
            .context("Ống ghi hình đã đóng")?
            .write_all(bytes)
            .context("Không ghi được khung hình vào FFmpeg")?;
        self.heartbeat
            .store(self.born.elapsed().as_millis() as u64, Ordering::Relaxed);
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        self.child.stdin.take();
        self.watch_stop.store(true, Ordering::Relaxed);
        let status = wait(&mut self.child, Duration::from_secs(30))?;
        if !status.success() {
            bail!("Bộ mã hóa dừng bất thường: {status}");
        }
        Ok(())
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.child.stdin.take();
        self.watch_stop.store(true, Ordering::Relaxed);
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DesktopSource {
    output_index: usize,
    offset_x: i32,
    offset_y: i32,
    width: u32,
    height: u32,
}

impl DesktopSource {
    fn from_options(options: &RecordingOptions) -> Option<Self> {
        let Source::Monitor {
            handle,
            bounds,
            region,
        } = options.source
        else {
            return None;
        };
        if options.prefs.encoder == "cpu"
            || (options.prefs.max_height != 0 && region.height > options.prefs.max_height)
        {
            return None;
        }
        let output_index = crate::native::displays()
            .iter()
            .position(|display| display.handle == handle)?;
        Some(Self {
            output_index,
            offset_x: region.x - bounds.x,
            offset_y: region.y - bounds.y,
            width: region.width & !1,
            height: region.height & !1,
        })
    }

    fn input(self, fps: u32, cursor: bool) -> String {
        format!(
            "ddagrab=output_idx={}:draw_mouse={}:framerate={}:video_size={}x{}:offset_x={}:offset_y={}:dup_frames=1",
            self.output_index,
            u8::from(cursor),
            fps,
            self.width,
            self.height,
            self.offset_x,
            self.offset_y
        )
    }
}

/// Zero-copy desktop encoder. FFmpeg acquires a D3D11 texture, maps that
/// surface into QSV video memory, and the QSV video processor converts RGB to
/// the NV12 surface required by H.264. Stdin carries only the final `q` control
/// byte; no frame data crosses the process boundary.
struct DesktopEncoder {
    child: Child,
}

impl DesktopEncoder {
    fn command(source: DesktopSource, prefs: &Preferences) -> Result<Process> {
        let mut command = process("ffmpeg")?;
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "warning",
                "-y",
                "-init_hw_device",
                "qsv=qsv,child_device_type=d3d11va",
                "-f",
                "lavfi",
                "-i",
            ])
            .arg(source.input(prefs.fps, prefs.cursor))
            .args([
                "-an",
                "-vf",
                "hwmap=derive_device=qsv:extra_hw_frames=16,format=qsv",
                "-c:v",
                "h264_qsv",
                "-async_depth",
                "1",
                "-look_ahead",
                "0",
                "-bf",
                "0",
                "-preset",
                "veryfast",
                "-scenario",
                "displayremoting",
                "-b:v",
            ])
            .arg(format!("{}M", prefs.bitrate))
            .arg("-g")
            .arg((prefs.fps * 2).to_string());
        Ok(command)
    }

    fn probe(source: DesktopSource, prefs: &Preferences, log: &Path) -> Result<()> {
        let mut command = Self::command(source, prefs)?;
        command
            .args(["-frames:v", "3", "-f", "null", "-"])
            .stderr(Stdio::from(
                OpenOptions::new().create(true).append(true).open(log)?,
            ));
        let mut child = command
            .spawn()
            .context("Không thử được pipeline D3D11/QSV")?;
        let status = wait(&mut child, Duration::from_secs(15))?;
        anyhow::ensure!(status.success(), "Pipeline D3D11/QSV không khả dụng");
        Ok(())
    }

    fn start(path: &Path, source: DesktopSource, prefs: &Preferences, log: &Path) -> Result<Self> {
        let mut command = Self::command(source, prefs)?;
        command
            .stdin(Stdio::piped())
            .stderr(Stdio::from(File::create(log)?))
            .args(["-flush_packets", "1", "-f", "matroska"])
            .arg(path);
        Ok(Self {
            child: command
                .spawn()
                .context("Không mở được pipeline D3D11/QSV")?,
        })
    }

    fn failed(&mut self) -> Result<bool> {
        // The desktop encoder must stay alive until we explicitly finish it.
        // An early zero exit is just as fatal as a non-zero exit here.
        Ok(self.child.try_wait()?.is_some())
    }

    fn finish(&mut self) -> Result<()> {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }
        self.child.stdin.take();
        let status = wait(&mut self.child, Duration::from_secs(30))?;
        anyhow::ensure!(
            status.success(),
            "Pipeline D3D11/QSV dừng bất thường: {status}"
        );
        Ok(())
    }
}

impl Drop for DesktopEncoder {
    fn drop(&mut self) {
        self.child.stdin.take();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct DesktopOverlayGuard(Option<crate::studio::Shared>);

impl DesktopOverlayGuard {
    fn new(live: Option<crate::studio::Shared>, enabled: bool) -> Self {
        if enabled {
            if let Some(shared) = &live {
                shared.lock().unwrap().desktop_capture = true;
            }
            Self(live)
        } else {
            Self(None)
        }
    }
}

impl Drop for DesktopOverlayGuard {
    fn drop(&mut self) {
        if let Some(shared) = &self.0 {
            shared.lock().unwrap().desktop_capture = false;
        }
    }
}

pub struct Controller {
    pub commands: Sender<Command>,
    pub events: Receiver<Event>,
}
pub fn start(options: RecordingOptions) -> Controller {
    start_live(options, None)
}
pub fn start_live(options: RecordingOptions, live: Option<crate::studio::Shared>) -> Controller {
    let (tx, rx) = mpsc::channel();
    let (events, incoming) = mpsc::channel();
    std::thread::spawn(move || {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(options, rx, &events, live)
        })) {
            Ok(Err(e)) => {
                let _ = events.send(Event::Failed(format!("{e:#}")));
            }
            Err(_) => {
                let _ = events.send(Event::Failed(
                    "Luồng ghi bị lỗi. Dữ liệu tạm được giữ trong thư mục .sessions để phục hồi."
                        .into(),
                ));
            }
            _ => {}
        }
    });
    Controller {
        commands: tx,
        events: incoming,
    }
}

fn run(
    options: RecordingOptions,
    commands: Receiver<Command>,
    events: &Sender<Event>,
    live: Option<crate::studio::Shared>,
) -> Result<()> {
    fs::create_dir_all(&options.prefs.output).context("Không tạo được thư mục lưu")?;
    crate::native::free_space(&options.prefs.output)?;
    let id = format!(
        "Quay_{}_{}",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_%3f"),
        std::process::id()
    );
    let folder = options.prefs.output.join(".sessions").join(&id);
    fs::create_dir_all(&folder)?;
    fs::write(
        folder.join("settings.json"),
        serde_json::to_vec_pretty(&options.prefs)?,
    )?;
    fs::write(
        folder.join("live-audio.txt"),
        "Gains and mute applied to PCM",
    )?;
    let result = run_inner(&options, commands, events, &folder, &id, live);
    result.with_context(|| format!("Dữ liệu phục hồi được giữ tại {}", folder.display()))
}

fn run_inner(
    options: &RecordingOptions,
    commands: Receiver<Command>,
    events: &Sender<Event>,
    folder: &Path,
    id: &str,
    live: Option<crate::studio::Shared>,
) -> Result<()> {
    let prefs = &options.prefs;
    let mut index = 0;
    let mut total_time = 0.0;
    let mut desktop_source = DesktopSource::from_options(options);
    if let Some(source) = desktop_source {
        if DesktopEncoder::probe(source, prefs, &folder.join("encoder.log")).is_err() {
            desktop_source = None;
        }
    }
    let mut encoder_name = desktop_source
        .map(|_| "h264_qsv · D3D11→NV12 zero-copy".to_string())
        .unwrap_or_default();
    let mut parts = Vec::new();
    let mut notice = None;
    let mut first_size = None;
    loop {
        let _ = events.send(Event::Starting);
        let screen = if desktop_source.is_none() {
            Some(ScreenSession::start(
                &options.source,
                prefs.cursor,
                prefs.fps,
            )?)
        } else {
            None
        };
        let size = if let Some(source) = desktop_source {
            (source.width, source.height)
        } else {
            let image = screen
                .as_ref()
                .unwrap()
                .latest
                .lock()
                .unwrap()
                .image
                .clone()
                .unwrap();
            (image.width, image.height)
        };
        if let Some(previous) = first_size {
            if previous != size {
                notice =
                    Some("Cửa sổ đã đổi kích thước khi tạm dừng. Đã lưu các đoạn trước đó.".into());
                break;
            }
        } else {
            first_size = Some(size);
        }
        if encoder_name.is_empty() {
            encoder_name = select_encoder(
                prefs,
                output_size(size.0, size.1, prefs.max_height),
                &folder.join("encoder.log"),
            )?;
        }
        let prefix = format!("segment-{index:04}");
        let video = folder.join(format!("{prefix}-video.mkv"));
        let mut audio_files = Vec::new();
        if prefs.system_audio {
            audio_files.push((false, folder.join(format!("{prefix}-system.pcm"))));
        }
        if prefs.microphone {
            audio_files.push((true, folder.join(format!("{prefix}-mic.pcm"))));
        }
        let mut sound = AudioSession::prepare(audio_files.clone(), prefs.clone(), live.clone())?;
        let overlay_guard = DesktopOverlayGuard::new(live.clone(), desktop_source.is_some());
        let mut desktop_encoder = if let Some(source) = desktop_source {
            Some(DesktopEncoder::start(
                &video,
                source,
                prefs,
                &folder.join(format!("{prefix}.log")),
            )?)
        } else {
            None
        };
        let mut pipe_encoder = if desktop_source.is_none() {
            Some(Encoder::start(
                &video,
                size,
                prefs,
                &encoder_name,
                &folder.join(format!("{prefix}.log")),
            )?)
        } else {
            None
        };
        let mut composite_buffer = Vec::with_capacity(size.0 as usize * size.1 as usize * 4);
        if let Some(encoder) = pipe_encoder.as_mut() {
            // Prime the process before starting either timeline. The same BGRA
            // allocation is reused for every annotated frame in this segment.
            let initial = screen
                .as_ref()
                .unwrap()
                .latest
                .lock()
                .unwrap()
                .image
                .clone()
                .unwrap();
            if let Some(shared) = &live {
                crate::studio::composite_into(&initial, shared, &mut composite_buffer);
                encoder.frame(&composite_buffer)?;
            } else {
                encoder.frame(&initial.pixels)?;
            }
        }
        let epoch = audio::clock_100ns()?;
        let start = Instant::now();
        sound.start(epoch)?;
        let _ = events.send(Event::Recording {
            encoder: encoder_name.clone(),
        });
        let mut frames = u64::from(pipe_encoder.is_some());
        let mut paused = false;
        let mut last_progress = Instant::now();
        loop {
            match if frames > 0 {
                commands.try_recv()
            } else {
                Err(mpsc::TryRecvError::Empty)
            } {
                Ok(Command::Pause) => {
                    paused = true;
                    break;
                }
                Ok(Command::Stop) | Err(mpsc::TryRecvError::Disconnected) => break,
                _ => {}
            }
            if sound.is_finished() {
                notice = Some("Thiết bị âm thanh bị ngắt hoặc không còn truy cập được.".into());
                break;
            }
            if let Some(encoder) = desktop_encoder.as_mut() {
                if encoder.failed()? {
                    notice = Some(
                        "Pipeline D3D11/QSV dừng bất thường; dữ liệu phục hồi đã được giữ.".into(),
                    );
                    break;
                }
                frames = ((start.elapsed().as_secs_f64() * prefs.fps as f64).floor() as u64).max(1);
                if last_progress.elapsed() > Duration::from_millis(500) {
                    let bytes = fs::metadata(&video).map(|m| m.len()).unwrap_or(0);
                    let _ = events.send(Event::Progress {
                        seconds: total_time + frames as f64 / prefs.fps as f64,
                        bytes,
                    });
                    if let Err(e) = crate::native::free_space(folder) {
                        notice = Some(e.to_string());
                        break;
                    }
                    last_progress = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            let next = Duration::from_secs_f64(frames as f64 / prefs.fps as f64);
            if start.elapsed() < next {
                std::thread::sleep(
                    next.saturating_sub(start.elapsed())
                        .min(Duration::from_millis(5)),
                );
                continue;
            }
            if start.elapsed().saturating_sub(next) > Duration::from_secs(2) {
                notice = Some(
                    "Máy không mã hóa kịp. Hãy giảm độ phân giải hoặc đóng tác vụ nặng.".into(),
                );
                break;
            }
            let latest = {
                let state = screen.as_ref().unwrap().latest.lock().unwrap();
                if let Some(ref error) = state.error {
                    notice = Some(error.clone());
                    break;
                }
                state.image.clone().unwrap()
            };
            let frame_result = if let Some(l) = &live {
                l.lock().unwrap().latest = Some(latest.clone());
                crate::studio::composite_into(&latest, l, &mut composite_buffer);
                pipe_encoder.as_mut().unwrap().frame(&composite_buffer)
            } else {
                pipe_encoder.as_mut().unwrap().frame(&latest.pixels)
            };
            if let Err(e) = frame_result {
                notice = Some(format!("{e:#}"));
                break;
            }
            frames += 1;
            if last_progress.elapsed() > Duration::from_millis(500) {
                let bytes = fs::metadata(&video).map(|m| m.len()).unwrap_or(0);
                let _ = events.send(Event::Progress {
                    seconds: total_time + frames as f64 / prefs.fps as f64,
                    bytes,
                });
                if let Err(e) = crate::native::free_space(folder) {
                    notice = Some(e.to_string());
                    break;
                }
                last_progress = Instant::now();
            }
        }
        // Stop acquisition first; finalization and AAC encoding run in this worker.
        let audio_result = sound.finish();
        drop(screen);
        let _ = events.send(Event::Finishing);
        if let Some(encoder) = desktop_encoder.as_mut() {
            encoder.finish()?;
        }
        if let Some(encoder) = pipe_encoder.as_mut() {
            encoder.finish()?;
        }
        drop(overlay_guard);
        if let Err(e) = audio_result {
            notice = Some(format!("{e:#}"));
        }
        if frames > 0 {
            let duration = frames as f64 / prefs.fps as f64;
            let samples = (frames * 48_000 / prefs.fps as u64) * 8;
            for (_, file) in &audio_files {
                // An available device may stay muted for the entire segment.
                // Keep that track empty; do not allocate hours of unused PCM.
                if fs::metadata(file)?.len() == 0 {
                    continue;
                }
                OpenOptions::new()
                    .write(true)
                    .open(file)?
                    .set_len(samples)?;
            }
            fs::write(
                folder.join(format!("{prefix}-duration.txt")),
                duration.to_string(),
            )?;
            let part = mux_segment(folder, &prefix, prefs, duration)?;
            parts.push(part);
            total_time += duration;
        }
        if !paused || notice.is_some() {
            break;
        }
        let _ = events.send(Event::Paused);
        match commands.recv() {
            Ok(Command::Resume) => {
                index += 1;
            }
            _ => break,
        }
    }
    let _ = events.send(Event::Finishing);
    let output = prefs.output.join(format!("{id}.mp4"));
    concat_parts(folder, &parts, &output)?;
    // Only remove this app-created session after the completed MP4 has been verified.
    let base = prefs.output.join(".sessions").canonicalize()?;
    let target = folder.canonicalize()?;
    if target.parent() == Some(base.as_path())
        && target
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("Quay_"))
    {
        let _ = fs::remove_dir_all(&target);
    }
    if let Some(message) = notice {
        fs::write(output.with_extension("notice.txt"), &message)?;
        let _ = events.send(Event::Failed(format!(
            "{message}\nVideo đã lưu: {}",
            output.display()
        )));
    } else {
        let _ = events.send(Event::Saved(output));
    }
    Ok(())
}

fn mux_segment(folder: &Path, prefix: &str, prefs: &Preferences, duration: f64) -> Result<PathBuf> {
    let output = folder.join(format!("{prefix}-ready.mkv"));
    let repaired = folder.join(format!("{prefix}-repaired.mkv"));
    let video = if repaired.exists() {
        repaired
    } else {
        folder.join(format!("{prefix}-video.mkv"))
    };
    let mut c = process("ffmpeg")?;
    c.args(["-hide_banner", "-loglevel", "warning", "-y", "-i"])
        .arg(video);
    let mut filters = Vec::new();
    let mut audio_count = 0;
    for (enabled, name, volume) in [
        (prefs.system_audio, "system", prefs.system_volume),
        (prefs.microphone, "mic", prefs.mic_volume),
    ] {
        if !enabled {
            continue;
        }
        let input_path = folder.join(format!("{prefix}-{name}.pcm"));
        if fs::metadata(&input_path).is_ok_and(|m| m.len() == 0) {
            continue;
        }
        c.args(["-f", "f32le", "-ar", "48000", "-ac", "2", "-i"])
            .arg(input_path);
        audio_count += 1;
        let gain = if folder.join("live-audio.txt").exists() {
            1.0
        } else {
            volume
        };
        filters.push(format!("[{audio_count}:a]volume={gain}[a{audio_count}]"));
    }
    if audio_count > 0 {
        let inputs = (1..=audio_count)
            .map(|i| format!("[a{i}]"))
            .collect::<String>();
        filters.push(format!("{inputs}amix=inputs={audio_count}:duration=longest:normalize=0,alimiter=limit=0.98:latency=1,apad[aout]"));
        c.arg("-filter_complex").arg(filters.join(";")).args([
            "-map", "0:v:0", "-map", "[aout]", "-c:a", "aac", "-b:a", "192k",
        ]);
    } else {
        c.args(["-map", "0:v:0", "-an"]);
    }
    c.args(["-c:v", "copy", "-t"])
        .arg(format!("{duration:.6}"))
        .arg(&output);
    run_logged(
        c,
        &folder.join("export.log"),
        Duration::from_secs((duration / 3.0 + 60.0) as u64),
    )?;
    Ok(output)
}

fn concat_parts(folder: &Path, parts: &[PathBuf], output: &Path) -> Result<()> {
    if parts.is_empty() {
        bail!("Chưa có khung hình nào để lưu.");
    }
    if output.exists() {
        bail!("File đích đã tồn tại: {}", output.display());
    }
    let contents = parts
        .iter()
        .map(|p| format!("file '{}'\n", p.file_name().unwrap().to_string_lossy()))
        .collect::<String>();
    fs::write(folder.join("concat.txt"), contents)?;
    let mut c = process("ffmpeg")?;
    c.current_dir(folder)
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-n",
            "-f",
            "concat",
            "-safe",
            "1",
            "-i",
            "concat.txt",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
        ])
        .arg(output);
    run_logged(c, &folder.join("export.log"), Duration::from_secs(600))?;
    if fs::metadata(output)?.len() < 1024 {
        bail!("File xuất nhỏ bất thường; giữ dữ liệu phục hồi.");
    }
    let mut probe = process("ffprobe")?;
    probe
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(output);
    let result = probe.stdout(Stdio::piped()).output()?;
    let duration = String::from_utf8_lossy(&result.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0);
    if !result.status.success() || duration <= 0.0 {
        bail!("Không xác thực được video đã xuất.");
    }
    Ok(())
}

pub fn recover_async(folder: PathBuf) -> Controller {
    let (commands, _) = mpsc::channel();
    let (events, incoming) = mpsc::channel();
    std::thread::spawn(move || {
        let result = recover(&folder);
        let _ = events.send(match result {
            Ok(path) => Event::Saved(path),
            Err(e) => Event::Failed(format!("Không phục hồi được: {e:#}")),
        });
    });
    Controller {
        commands,
        events: incoming,
    }
}

pub fn recover(folder: &Path) -> Result<PathBuf> {
    let folder = folder.canonicalize()?;
    let prefs: Preferences = serde_json::from_slice(&fs::read(folder.join("settings.json"))?)?;
    let mut videos = fs::read_dir(&folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-video.mkv")
        })
        .collect::<Vec<_>>();
    videos.sort();
    let mut parts = Vec::new();
    for video in videos {
        let name = video.file_name().unwrap().to_string_lossy();
        let prefix = name.trim_end_matches("-video.mkv");
        let repaired = folder.join(format!("{prefix}-repaired.mkv"));
        let mut repair = process("ffmpeg")?;
        repair
            .args([
                "-hide_banner",
                "-loglevel",
                "warning",
                "-y",
                "-fflags",
                "+genpts",
                "-i",
            ])
            .arg(&video)
            .args(["-map", "0:v:0", "-c:v", "copy"])
            .arg(&repaired);
        run_logged(
            repair,
            &folder.join("recovery.log"),
            Duration::from_secs(600),
        )?;
        let mut c = process("ffprobe")?;
        let result = c
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=nw=1:nk=1",
            ])
            .arg(&repaired)
            .stdout(Stdio::piped())
            .output()?;
        let duration: f64 = fs::read_to_string(folder.join(format!("{prefix}-duration.txt")))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .or_else(|| String::from_utf8_lossy(&result.stdout).trim().parse().ok())
            .context("Không xác định được thời lượng đoạn bị gián đoạn")?;
        parts.push(mux_segment(&folder, prefix, &prefs, duration)?);
    }
    let output = prefs.output.join(format!(
        "Phuc_hoi_{}.mp4",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_%3f")
    ));
    concat_parts(&folder, &parts, &output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaling_keeps_aspect_ratio_even_and_does_not_upscale() {
        assert_eq!(output_size(1920, 1080, 720), (1280, 720));
        assert_eq!(output_size(801, 601, 1080), (800, 600));
        assert_eq!(output_size(1080, 1920, 720), (404, 720));
    }
}
