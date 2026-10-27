//! Concat is an unmodified, separately distributed application. This module
//! speaks its public MCP protocol; no Concat engine is linked into the recorder.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn is_project(p: &Path) -> bool {
    p.file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("concat.json"))
}
pub fn legacy(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("taoclip"))
}
pub fn binary() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let bundled = exe
        .parent()
        .context("Missing executable directory")?
        .join("concat/concat.exe");
    if bundled.is_file() {
        return Ok(bundled);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev = root.join("vendor/concat-extracted/PFiles64/Concat/concat.exe");
    if exe.starts_with(root.join("target")) && dev.is_file() {
        return Ok(dev);
    }
    bail!("Thiếu Concat đi kèm. Hãy giải nén đầy đủ ScreenRecorder-portable.zip. / Bundled Concat missing.")
}
pub fn command() -> Result<Command> {
    let exe = binary()?;
    let mut c = Command::new(&exe);
    c.current_dir(exe.parent().unwrap())
        .creation_flags(0x08000000);
    // Keep our bundled editor's settings separate from a standalone Concat.
    let profile = crate::library::data_dir()
        .parent()
        .unwrap_or(&crate::library::data_dir())
        .join("concat-profile");
    fs::create_dir_all(&profile)?;
    c.env("APPDATA", &profile).env("LOCALAPPDATA", &profile);
    Ok(c)
}

pub struct Mcp {
    child: Child,
    input: Option<ChildStdin>,
    lines: Receiver<std::io::Result<String>>,
    id: u64,
}
impl Mcp {
    pub fn start(roots: &[PathBuf]) -> Result<Self> {
        let logs = crate::library::data_dir().join("concat-logs");
        fs::create_dir_all(&logs)?;
        let mut c = command()?;
        c.args(["--mcp", "headless"])
            .env("CONCAT_MCP_ROOTS", std::env::join_paths(roots)?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(logs.join(format!("mcp-{}.log", unique())))?);
        let mut child = c.spawn()?;
        let input = child.stdin.take();
        let output = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut m = Self {
            child,
            input,
            lines,
            id: 0,
        };
        m.request("initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ScreenRecorder","version":env!("CARGO_PKG_VERSION")}}))?;
        writeln!(
            m.input.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )?;
        m.input.as_mut().unwrap().flush()?;
        Ok(m)
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        let input = self.input.as_mut().context("Concat pipe closed")?;
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params})
        )?;
        input.flush()?;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let line = self
                .lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .context("Concat không phản hồi / Concat timed out or exited")??;
            let response: Value = serde_json::from_str(&line).context("Invalid Concat response")?;
            if response["id"].as_u64() != Some(self.id) {
                continue;
            }
            if let Some(e) = response.get("error") {
                bail!("Concat: {e}");
            }
            return Ok(response["result"].clone());
        }
    }
    pub fn call(&mut self, name: &str, args: Value) -> Result<Value> {
        let result = self.request("tools/call", json!({"name":name,"arguments":args}))?;
        if result["isError"] == true {
            bail!("Concat: {}", result["content"]);
        }
        if let Some(v) = result.get("structuredContent") {
            return Ok(v.clone());
        }
        let text = result["content"][0]["text"]
            .as_str()
            .context("Missing Concat result")?;
        Ok(serde_json::from_str(text).unwrap_or(Value::String(text.into())))
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(3);
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn unique() -> String {
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}
fn key(path: &Path) -> String {
    format!(
        "{:016x}",
        crate::library::path_key(path)
            .bytes()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64)
                .wrapping_mul(0x100000001b3))
    )
}
pub fn publish(project: &Path, sources: &[PathBuf], output: Option<&Path>) -> Result<()> {
    crate::library::atomic_write(
        &crate::library::data_dir()
            .join("events")
            .join(format!("{}-concat.json", unique())),
        &serde_json::to_vec(
            &json!({"version":1,"project":project,"sources":sources,"output":output}),
        )?,
    )
}

/// A project is staged under a unique folder, then renamed only after the API
/// saved and released its lock. Failed imports never become the default edit.
pub fn prepare(video: &Path) -> Result<PathBuf> {
    let video = std::path::absolute(video)?;
    let video = if video.is_dir() {
        video.join("concat.json")
    } else {
        video
    };
    if legacy(&video) {
        bail!("Project TaoClip cũ được giữ nguyên. Concat không đọc .taoclip; hãy chọn video gốc để tạo bản dựng mới. / Legacy TaoClip project preserved; select the original video for a new Concat edit.");
    }
    if is_project(&video) {
        if !video.is_file() {
            bail!(
                "Không tìm thấy project / Project missing: {}",
                video.display()
            );
        }
        let value: Value = serde_json::from_slice(&fs::read(&video)?)
            .context("Project Concat bị hỏng / Corrupt Concat project")?;
        if !value.is_object() || value.get("video").is_none() {
            bail!("Project Concat không hợp lệ / Invalid Concat project");
        }
        return Ok(video);
    }
    let dir = crate::library::data_dir();
    let (mut db, _) = crate::library::Index::load(&dir);
    db.consume_events(&dir);
    if let Some(p) = db
        .items
        .iter()
        .find(|i| crate::library::path_key(&i.path) == crate::library::path_key(&video))
        .and_then(|i| i.project.as_ref())
        .filter(|p| is_project(p))
    {
        if !p.is_file() {
            bail!(
                "Project đã lưu bị thiếu; hãy tìm lại project trước. / Saved project missing: {}",
                p.display()
            );
        }
        return Ok(p.clone());
    }
    let projects = dir.join("concat-projects");
    fs::create_dir_all(&projects)?;
    let destination = projects.join(key(&video));
    let manifest = destination.join("concat.json");
    if manifest.is_file() {
        return Ok(manifest);
    }
    if destination.exists() {
        bail!(
            "Thư mục project chưa hoàn chỉnh / Incomplete project: {}",
            destination.display()
        );
    }
    if !video.is_file() {
        bail!("Video không tồn tại / Video missing: {}", video.display());
    }
    let mut m = Mcp::start(&[projects.clone(), video.parent().unwrap().to_path_buf()])?;
    let probe = m.call("media_probe", json!({"path":video}))?;
    let info = &probe["video"];
    let width = info["width"]
        .as_u64()
        .filter(|w| *w > 0)
        .context("Video has no picture")?;
    let height = info["height"]
        .as_u64()
        .filter(|h| *h > 0)
        .context("Video has no height")?;
    if probe["duration"]
        .as_f64()
        .filter(|d| d.is_finite() && *d > 0.0)
        .is_none()
    {
        bail!("Video chưa hoàn chỉnh / Invalid video duration");
    }
    let fraction = info["frameRateFraction"].as_str().unwrap_or("30/1");
    let (n, d) = fraction.split_once('/').context("Invalid frame rate")?;
    let n: u64 = n.parse()?;
    let d: u64 = d.parse()?;
    if n == 0 || d == 0 {
        bail!("Invalid frame rate");
    }
    let name = format!("creating-{}", unique());
    let staging = projects.join(&name);
    m.call("project_create", json!({"location":projects,"name":name,"video":{"width":width,"height":height,"rateNum":n,"rateDen":d}}))?;
    let imported = m.call("media_import", json!({"path":staging,"file":video}))?;
    let media = imported["createdId"]
        .as_str()
        .context("No imported media ID")?;
    let track = imported["timeline"]["tracks"][0]["id"]
        .as_str()
        .context("No timeline track")?;
    m.call(
        "clip_add",
        json!({"path":staging,"mediaId":media,"trackId":track,"start":0.0}),
    )?;
    m.call(
        "project_save",
        json!({"path":staging,"name":video.file_stem().unwrap_or_default().to_string_lossy()}),
    )?;
    m.call("project_close", json!({"path":staging,"save":true}))?;
    drop(m);
    fs::rename(&staging, &destination).context("Could not publish Concat project")?;
    publish(&manifest, &[video], None)?;
    Ok(manifest)
}

fn focus(pid: u32) -> bool {
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::*,
        },
    };
    struct Find {
        pid: u32,
        found: bool,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let find = &mut *(data.0 as *mut Find);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == find.pid && IsWindowVisible(hwnd).as_bool() {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
            find.found = true;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut find = Find { pid, found: false };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM((&mut find as *mut Find) as isize));
    }
    find.found
}

pub fn session(input: Option<&Path>, status: &Path) -> Result<()> {
    let result = run_session(input, status);
    if let Err(e) = &result {
        let _ = crate::library::atomic_write(
            status,
            &serde_json::to_vec(&json!({"error":format!("{e:#}")}))?,
        );
    }
    result
}
fn run_session(input: Option<&Path>, status: &Path) -> Result<()> {
    // An OS-held lock survives neither a crash nor a reboot. A duplicate click
    // focuses the window owned by the existing helper, without reopening media.
    let sessions = crate::library::data_dir().join("concat-sessions");
    fs::create_dir_all(&sessions)?;
    let k = input
        .map(|p| {
            let target = if is_project(p) {
                p.to_path_buf()
            } else {
                crate::library::data_dir()
                    .join("concat-projects")
                    .join(key(p))
                    .join("concat.json")
            };
            key(&target)
        })
        .unwrap_or_else(|| "empty".into());
    let lock = sessions.join(format!("{k}.lock"));
    let pid_file = sessions.join(format!("{k}.json"));
    let started = Instant::now();
    let _guard = loop {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(&lock)
        {
            Ok(f) => break f,
            Err(e) if e.raw_os_error() == Some(32) => {
                if let Ok(Ok(v)) = fs::read(&pid_file).map(|b| serde_json::from_slice::<Value>(&b))
                {
                    if v["guiPid"].as_u64().is_some_and(|pid| focus(pid as u32)) {
                        crate::library::atomic_write(status, &serde_json::to_vec(&v)?)?;
                        return Ok(());
                    }
                }
                if started.elapsed() > Duration::from_secs(120) {
                    bail!("Concat đang bận mở project / Concat is still opening this project");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.into()),
        }
    };
    let _ = fs::remove_file(&pid_file);
    let project = input.map(prepare).transpose()?;
    let mut c = command()?;
    if let Some(p) = &project {
        c.arg("--project").arg(p.parent().unwrap());
    }
    let logs = crate::library::data_dir().join("concat-logs");
    fs::create_dir_all(&logs)?;
    let mut child = c
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(File::create(logs.join(format!("window-{}.log", unique())))?)
        .spawn()?;
    let ready = json!({"guiPid":child.id(),"project":project,"error":null});
    crate::library::atomic_write(&pid_file, &serde_json::to_vec(&ready)?)?;
    // Only report success after the window exists, not just after CreateProcess.
    let start = Instant::now();
    loop {
        if let Some(exit) = child.try_wait()? {
            bail!("Concat thoát trước khi mở cửa sổ / Concat exited during startup: {exit}");
        }
        if focus(child.id()) {
            break;
        }
        if start.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Concat không mở được cửa sổ / Concat window startup timed out");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    crate::library::atomic_write(status, &serde_json::to_vec(&ready)?)?;
    let exit = child.wait()?;
    let _ = fs::remove_file(pid_file);
    if !exit.success() {
        bail!("Concat đã thoát do lỗi / Concat exited: {exit}");
    }
    Ok(())
}

/// Runs the real shipped engine, saving/reopening and exporting the project.
pub fn check(input: &Path, output: &Path) -> Result<Value> {
    if output.exists() {
        bail!(
            "Refusing to overwrite existing export: {}",
            output.display()
        );
    }
    let project = prepare(input)?;
    let folder = project.parent().unwrap();
    let output = std::path::absolute(output)?;
    fs::create_dir_all(output.parent().unwrap())?;
    let mut m = Mcp::start(&[
        folder.to_path_buf(),
        input.parent().unwrap().to_path_buf(),
        output.parent().unwrap().to_path_buf(),
    ])?;
    let before = m.call("project_open", json!({"path":folder}))?;
    m.call("project_save", json!({"path":folder}))?;
    m.call("project_close", json!({"path":folder,"save":true}))?;
    let after = m.call("project_open", json!({"path":folder}))?;
    if before["timeline"] != after["timeline"] {
        bail!("Timeline changed across save/reopen");
    }
    let job = m.call(
        "export_start",
        json!({"path":folder,"output":output,"crf":23,"codec":"h264"}),
    )?;
    let end = Instant::now() + Duration::from_secs(600);
    let export = loop {
        let state = m.call("export_wait", json!({"job":job["job"],"timeoutSeconds":30}))?;
        if state["state"] == "done" {
            break state;
        }
        if state["state"] == "failed" || state["state"] == "cancelled" || Instant::now() > end {
            bail!("Export failed: {state}");
        }
    };
    m.call("project_close", json!({"path":folder,"save":true}))?;
    let sources: Vec<PathBuf> = before["media"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["path"].as_str().map(PathBuf::from))
        .collect();
    publish(&project, &sources, Some(&output))?;
    Ok(json!({"project":project,"timeline":after,"export":export,"output":output}))
}
