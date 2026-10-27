//! Rebuildable local catalogue. Only the coordinator writes the index; editor
//! processes publish immutable events. No filesystem work runs in egui paint.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::{
            GetFileInformationByHandle, MoveFileExW, BY_HANDLE_FILE_INFORMATION,
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        UI::Shell::{FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT},
    },
};

pub fn data_dir() -> PathBuf {
    std::env::var_os("SCREENRECORDER_LIBRARY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("ScreenRecorder")
                .join("library")
        })
}
fn now() -> u64 {
    millis(SystemTime::now())
}
fn millis(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
pub fn path_key(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").to_lowercase()
}
fn wide(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str().encode_wide().chain(Some(0)).collect()
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("Missing parent")?)?;
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut f = fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    unsafe {
        MoveFileExW(
            PCWSTR(wide(&tmp).as_ptr()),
            PCWSTR(wide(path).as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )?;
    }
    Ok(())
}
fn identity(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info).ok()?;
    }
    Some(format!(
        "{:x}-{:x}-{:x}",
        info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
    ))
}
fn hash(value: &str) -> String {
    // Stable FNV-1a for cache names (not an integrity/security hash).
    let h = value.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    format!("{h:016x}")
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Video,
    Image,
    Project,
    Session,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum State {
    Finalizing,
    Ready,
    Missing,
    Recovery,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub audio: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub path: PathBuf,
    pub identity: Option<String>,
    pub name: String,
    pub favorite: bool,
    pub kind: Kind,
    pub state: State,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub metadata: Option<Metadata>,
    pub thumbnail: Option<PathBuf>,
    pub error: Option<String>,
    #[serde(default)]
    pub sources: Vec<PathBuf>,
    #[serde(default)]
    pub project: Option<PathBuf>,
    #[serde(default)]
    pub exports: Vec<PathBuf>,
    #[serde(default)]
    pub is_export: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub aliases: Vec<PathBuf>,
}
impl Item {
    fn stamp(&self) -> String {
        format!("{}-{}-{}", self.id, self.size, self.modified)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    pub roots: Vec<PathBuf>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub events: HashSet<String>,
    #[serde(skip)]
    paths: HashMap<String, usize>,
    #[serde(skip)]
    identities: HashMap<String, usize>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            version: 1,
            roots: Vec::new(),
            items: Vec::new(),
            events: HashSet::new(),
            paths: HashMap::new(),
            identities: HashMap::new(),
        }
    }
}
impl Index {
    pub fn load(dir: &Path) -> (Self, Option<String>) {
        let file = dir.join("index.json");
        let read = |p: &Path| -> Option<Self> {
            let db: Self = serde_json::from_slice(&fs::read(p).ok()?).ok()?;
            (db.version == 1).then_some(db)
        };
        if let Some(db) = read(&file) {
            return (db, None);
        }
        if let Some(db) = read(&dir.join("index.backup.json")) {
            return (
                db,
                Some("Đã phục hồi chỉ mục từ bản dự phòng / Index restored from backup".into()),
            );
        }
        if file.is_file() {
            let _ = fs::copy(&file, dir.join(format!("index-unreadable-{}.json", now())));
        }
        let mut db = Self::default();
        if let Ok(bytes) = fs::read(dir.join("roots.json")) {
            db.roots = serde_json::from_slice(&bytes).unwrap_or_default();
        }
        (db, file.exists().then(|| "Chỉ mục lỗi/khác phiên bản; đang xây dựng lại từ file và nhật ký / Rebuilding index".into()))
    }
    fn save(&self, dir: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self)?;
        atomic_write(&dir.join("index.json"), &bytes)?;
        atomic_write(&dir.join("index.backup.json"), &bytes)?;
        atomic_write(&dir.join("roots.json"), &serde_json::to_vec(&self.roots)?)
    }
    fn add_root(&mut self, p: PathBuf) {
        if !self.roots.iter().any(|r| path_key(r) == path_key(&p)) {
            self.roots.push(p);
        }
    }
    fn add_file(&mut self, path: PathBuf, seen: &mut HashSet<String>) {
        let Some(kind) = kind(&path) else { return };
        let Ok(meta) = path.metadata() else { return };
        if !meta.is_file() {
            return;
        }
        let key = path_key(&path);
        let by_path = self.paths.get(&key).copied();
        if by_path.is_some_and(|pos| seen.contains(&self.items[pos].id)) {
            return;
        }
        let modified = meta.modified().map(millis).unwrap_or(0);
        let fid = by_path
            .filter(|&pos| {
                self.items[pos].size == meta.len() && self.items[pos].modified == modified
            })
            .and_then(|pos| self.items[pos].identity.clone())
            .or_else(|| identity(&path));
        let existing = by_path.or_else(|| {
            fid.as_ref()
                .and_then(|f| self.identities.get(f).copied())
                .filter(|&pos| {
                    !self.items[pos].path.exists()
                        && meta
                            .created()
                            .map(millis)
                            .is_ok_and(|created| created == self.items[pos].created)
                })
        });
        let pos = existing.unwrap_or_else(|| {
            let id = hash(&format!("{key}|{}", now()));
            self.items.push(Item {
                id,
                path: path.clone(),
                identity: fid.clone(),
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                favorite: false,
                kind,
                state: State::Finalizing,
                size: 0,
                modified: 0,
                created: meta.created().map(millis).unwrap_or(modified),
                metadata: None,
                thumbnail: None,
                error: None,
                sources: vec![],
                project: None,
                exports: vec![],
                is_export: false,
                hidden: false,
                aliases: vec![],
            });
            self.items.len() - 1
        });
        // Update references when Windows file identity proves a same-volume move.
        self.paths.insert(key.clone(), pos);
        if let Some(f) = &fid {
            self.identities.insert(f.clone(), pos);
        }
        let old = self.items[pos].path.clone();
        if path_key(&old) != key {
            self.items[pos].aliases.push(old.clone());
            for i in &mut self.items {
                for p in i
                    .sources
                    .iter_mut()
                    .chain(i.exports.iter_mut())
                    .chain(i.project.iter_mut())
                {
                    if path_key(p) == path_key(&old) {
                        *p = path.clone();
                    }
                }
            }
        }
        let item = &mut self.items[pos];
        if item.hidden && item.state == State::Missing {
            item.hidden = false;
        }
        if item.size != meta.len() || item.modified != modified || item.state == State::Missing {
            item.metadata = None;
            item.thumbnail = None;
            item.error = None;
            item.state = State::Finalizing;
        }
        item.path = path;
        item.identity = fid;
        item.size = meta.len();
        item.modified = modified;
        seen.insert(item.id.clone());
    }
    fn reindex(&mut self) {
        self.paths = self
            .items
            .iter()
            .enumerate()
            .map(|(n, i)| (path_key(&i.path), n))
            .collect();
        self.identities = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(n, i)| i.identity.clone().map(|f| (f, n)))
            .collect();
    }
    pub fn scan(&mut self) -> Vec<String> {
        self.reindex();
        let previous_sessions = self
            .items
            .iter()
            .filter(|i| i.kind == Kind::Session)
            .map(session_output)
            .collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        let mut errors = vec![];
        // Keep exact paths registered by the editor, including files outside roots.
        let known = self
            .items
            .iter()
            .filter(|i| i.kind != Kind::Session)
            .map(|i| i.path.clone())
            .collect::<Vec<_>>();
        for p in known {
            self.add_file(p, &mut seen);
        }
        let mut visited = HashSet::new();
        let mut dirs = self.roots.clone();
        let concat_projects = data_dir().join("concat-projects");
        if concat_projects.is_dir() {
            dirs.push(concat_projects.clone());
        }
        while let Some(dir) = dirs.pop() {
            if !visited.insert(path_key(&dir)) {
                continue;
            }
            match fs::read_dir(&dir) {
                Ok(entries) => {
                    for e in entries.flatten() {
                        let p = e.path();
                        let Ok(ft) = e.file_type() else { continue };
                        if ft.is_symlink() {
                            continue;
                        }
                        if ft.is_dir() {
                            let n = e.file_name().to_string_lossy().into_owned();
                            if n == ".sessions" {
                                self.scan_sessions(&p, &mut seen);
                            } else if !n.starts_with('.')
                                && !["node_modules", "thumbnails"].contains(&n.as_str())
                                && !(dir.starts_with(&concat_projects)
                                    && (n == "cache" || n.starts_with("creating-")))
                            {
                                dirs.push(p);
                            }
                        } else {
                            self.add_file(p, &mut seen);
                        }
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", dir.display())),
            }
        }
        self.items
            .retain(|i| i.kind != Kind::Session || seen.contains(&i.id));
        for i in &mut self.items {
            if !seen.contains(&i.id) {
                i.state = State::Missing;
            }
            if i.kind == Kind::Project && seen.contains(&i.id) && !i.sources.is_empty() {
                let missing = i.sources.iter().any(|p| !p.is_file());
                if missing && i.state == State::Ready {
                    i.state = State::Recovery;
                    i.error = Some("Project có media bị thiếu / Missing project media".into());
                } else if !missing
                    && i.state == State::Recovery
                    && i.error
                        .as_ref()
                        .is_some_and(|e| e.contains("Missing project media"))
                {
                    i.state = State::Finalizing;
                }
            }
            if i.state == State::Ready && i.thumbnail.as_ref().is_some_and(|p| !p.is_file()) {
                i.state = State::Finalizing;
            }
        }
        // A muxed MP4 must not become actionable before its session finishes.
        let sessions = self
            .items
            .iter()
            .filter(|i| i.kind == Kind::Session)
            .map(|i| {
                (
                    i.path
                        .parent()
                        .and_then(Path::parent)
                        .unwrap_or(Path::new(""))
                        .join(format!(
                            "{}.mp4",
                            i.path.file_name().unwrap_or_default().to_string_lossy()
                        )),
                    i.state.clone(),
                )
            })
            .collect::<Vec<_>>();
        for i in &mut self.items {
            if let Some((_, state)) = sessions.iter().find(|(p, _)| *p == i.path) {
                i.state = state.clone();
            } else if previous_sessions.contains(&i.path) && i.state != State::Missing {
                i.state = State::Finalizing;
            }
        }
        let sidecars = self
            .items
            .iter()
            .filter(|i| i.kind == Kind::Video && !i.is_export)
            .map(|i| (i.id.clone(), i.path.clone()))
            .collect::<Vec<_>>();
        for (id, p) in sidecars {
            if let Ok(bytes) = fs::read(format!("{}.screenrecorder.json", p.display())) {
                if bytes.len() < 1_000_000 {
                    if let Ok(e) = serde_json::from_slice::<BridgeEvent>(&bytes) {
                        if e.version == 1 {
                            if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
                                i.is_export = true;
                                i.sources = e.sources;
                                i.project = e.project;
                            }
                        }
                    }
                }
            }
        }
        self.reindex();
        errors
    }
    fn scan_sessions(&mut self, dir: &Path, seen: &mut HashSet<String>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if !p.join("settings.json").is_file() {
                continue;
            }
            let id = format!("session-{}", hash(&path_key(&p)));
            seen.insert(id.clone());
            let active = fs::read_to_string(p.join("active.pid"))
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .is_some_and(process_alive);
            let state = if active {
                State::Finalizing
            } else {
                State::Recovery
            };
            if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
                i.state = state;
                continue;
            }
            self.items.push(Item {
                id,
                path: p.clone(),
                identity: None,
                name: p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                favorite: false,
                kind: Kind::Session,
                state,
                size: 0,
                modified: 0,
                created: p
                    .metadata()
                    .and_then(|m| m.created())
                    .map(millis)
                    .unwrap_or(0),
                metadata: None,
                thumbnail: None,
                error: Some("Phiên ghi tạm / Recording session".into()),
                sources: vec![],
                project: None,
                exports: vec![],
                is_export: false,
                hidden: false,
                aliases: vec![],
            });
        }
    }
    pub fn consume_events(&mut self, dir: &Path) -> bool {
        self.reindex();
        let mut changed = false;
        let mut entries = fs::read_dir(dir.join("events"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect::<Vec<_>>();
        entries.sort();
        for file in entries {
            let key = file.file_name().unwrap().to_string_lossy().into_owned();
            if self.events.contains(&key) {
                continue;
            }
            let Ok(bytes) = fs::read(&file) else { continue };
            if bytes.len() > 1_000_000 {
                continue;
            }
            let Ok(event) = serde_json::from_slice::<BridgeEvent>(&bytes) else {
                continue;
            };
            if event.version != 1 {
                continue;
            }
            let mut seen = HashSet::new();
            for p in event
                .sources
                .iter()
                .chain(event.project.iter())
                .chain(event.output.iter())
            {
                self.add_file(p.clone(), &mut seen);
            }
            if let Some(ref project) = event.project {
                for i in &mut self.items {
                    if event
                        .sources
                        .iter()
                        .any(|s| path_key(s) == path_key(&i.path))
                    {
                        i.project = Some(project.clone());
                    }
                }
            }
            if let Some(ref output) = event.output {
                for i in &mut self.items {
                    if path_key(&i.path) == path_key(output) {
                        i.is_export = true;
                        i.sources = event.sources.clone();
                        i.project = event.project.clone();
                    }
                    if event
                        .sources
                        .iter()
                        .chain(event.project.iter())
                        .any(|s| path_key(s) == path_key(&i.path))
                        && !i.exports.contains(output)
                    {
                        i.exports.push(output.clone());
                    }
                }
            }
            self.events.insert(key);
            changed = true;
        }
        changed
    }
    fn link_projects(&mut self) {
        let aliases: HashMap<String, PathBuf> = self
            .items
            .iter()
            .flat_map(|i| i.aliases.iter().map(|p| (path_key(p), i.path.clone())))
            .collect();
        for item in &mut self.items {
            for p in item
                .sources
                .iter_mut()
                .chain(item.project.iter_mut())
                .chain(item.exports.iter_mut())
            {
                if let Some(new) = aliases.get(&path_key(p)) {
                    *p = new.clone();
                }
            }
            if item.kind == Kind::Project
                && item.state == State::Recovery
                && item
                    .error
                    .as_ref()
                    .is_some_and(|e| e.contains("Missing project media"))
                && item.sources.iter().all(|p| p.is_file())
            {
                item.state = State::Ready;
                item.error = None;
            }
        }
        let mut by_source: HashMap<String, (PathBuf, u64)> = HashMap::new();
        for i in self
            .items
            .iter()
            .filter(|i| i.kind == Kind::Project && !i.hidden)
        {
            for s in &i.sources {
                let entry = by_source
                    .entry(path_key(s))
                    .or_insert((i.path.clone(), i.modified));
                if (crate::concat_bridge::is_project(&i.path)
                    && !crate::concat_bridge::is_project(&entry.0))
                    || (crate::concat_bridge::is_project(&i.path)
                        == crate::concat_bridge::is_project(&entry.0)
                        && i.modified > entry.1)
                {
                    *entry = (i.path.clone(), i.modified);
                }
            }
        }
        for i in &mut self.items {
            if i.kind == Kind::Project || i.kind == Kind::Session {
                continue;
            }
            let link = by_source.get(&path_key(&i.path)).or_else(|| {
                if i.is_export {
                    i.sources.iter().find_map(|p| by_source.get(&path_key(p)))
                } else {
                    None
                }
            });
            if let Some((p, _)) = link {
                if !i.is_export || i.project.is_none() {
                    i.project = Some(p.clone());
                }
            }
        }
        let mut exports: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for i in self.items.iter().filter(|i| i.is_export && !i.hidden) {
            for p in i.sources.iter().chain(i.project.iter()) {
                exports.entry(path_key(p)).or_default().push(i.path.clone());
            }
        }
        for i in &mut self.items {
            i.exports = exports.remove(&path_key(&i.path)).unwrap_or_default();
        }
    }
}
#[derive(Serialize, Deserialize)]
struct BridgeEvent {
    version: u32,
    #[serde(default)]
    sources: Vec<PathBuf>,
    project: Option<PathBuf>,
    output: Option<PathBuf>,
}
fn kind(path: &Path) -> Option<Kind> {
    if crate::concat_bridge::is_project(path) {
        return Some(Kind::Project);
    }
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v" | "ts" => Some(Kind::Video),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => Some(Kind::Image),
        "taoclip" => Some(Kind::Project),
        _ => None,
    }
}
fn session_output(i: &Item) -> PathBuf {
    i.path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new(""))
        .join(format!(
            "{}.mp4",
            i.path.file_name().unwrap_or_default().to_string_lossy()
        ))
}
fn process_alive(pid: u32) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0;
        let ok = GetExitCodeProcess(h, &mut code).is_ok();
        let _ = CloseHandle(h);
        ok && code == 259
    }
}

fn run_tool(name: &str, args: &[String], timeout: u64) -> Result<Vec<u8>> {
    let mut child = Command::new(crate::recorder::tool(name)?)
        .args(args)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut data = vec![];
        let _ = stdout.take(1_000_000).read_to_end(&mut data);
        data
    });
    let start = Instant::now();
    let result = loop {
        if let Some(s) = child.try_wait()? {
            break if s.success() {
                Ok(())
            } else {
                Err(anyhow::anyhow!("Không đọc được media / Invalid media"))
            };
        }
        if start.elapsed() > Duration::from_secs(timeout) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!("Đọc media quá thời gian / Media timed out"));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let bytes = reader.join().unwrap_or_default();
    result?;
    Ok(bytes)
}
fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|v| v.to_string()).collect()
}
fn project_sources(v: &serde_json::Value, base: &Path) -> Vec<PathBuf> {
    fn walk(v: &serde_json::Value, base: &Path, out: &mut Vec<PathBuf>) {
        match v {
            serde_json::Value::Object(o) => {
                if let Some(p) = o.get("path").and_then(|s| s.as_str()) {
                    if !p.contains("://") {
                        let p = PathBuf::from(p);
                        let p = if p.is_absolute() { p } else { base.join(p) };
                        let p = std::path::absolute(&p).unwrap_or(p);
                        if !out.contains(&p) {
                            out.push(p);
                        }
                    }
                }
                for (k, v) in o {
                    if k != "screenRecorder" {
                        walk(v, base, out);
                    }
                }
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    walk(v, base, out);
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    walk(v, base, &mut out);
    out
}
fn inspect(mut item: Item, dir: &Path) -> Item {
    let result = (|| -> Result<()> {
        if item.kind == Kind::Project {
            if item.size > 25 * 1024 * 1024 {
                bail!("Project lớn hơn 25 MB");
            }
            let value: serde_json::Value = serde_json::from_slice(&fs::read(&item.path)?)?;
            if !value.is_object()
                || !(value.get("timelines").is_some() || value.get("clips").is_some())
            {
                bail!("Project không hợp lệ / Invalid project");
            }
            item.sources = project_sources(&value, item.path.parent().unwrap());
            let timeline = value["timelines"]
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .find(|t| t["id"] == value["activeTimelineId"])
                        .or_else(|| a.first())
                })
                .unwrap_or(&value);
            let concat = crate::concat_bridge::is_project(&item.path);
            let duration = timeline["clips"]
                .as_array()
                .map(|clips| {
                    if concat {
                        clips
                            .iter()
                            .map(|c| {
                                c["start"].as_f64().unwrap_or(0.0)
                                    + c["duration"].as_f64().unwrap_or(0.0)
                            })
                            .fold(0.0, f64::max)
                    } else {
                        clips
                            .iter()
                            .map(|c| {
                                (c["out"].as_f64().unwrap_or(0.0) - c["in"].as_f64().unwrap_or(0.0))
                                    .max(0.0)
                                    / c["speed"].as_f64().unwrap_or(1.0).max(0.01)
                            })
                            .sum()
                    }
                })
                .unwrap_or(0.0);
            let media = if concat {
                &value["video"]
            } else {
                &value["assets"][0]["metadata"]
            };
            item.metadata = Some(Metadata {
                duration,
                width: media["width"].as_u64().unwrap_or(0) as u32,
                height: media["height"].as_u64().unwrap_or(0) as u32,
                audio: if concat {
                    value["media"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|m| m["hasAudio"] == true))
                } else {
                    media["hasAudio"].as_bool().unwrap_or(false)
                },
            });
            if item.sources.iter().any(|p| !p.is_file()) {
                bail!(
                    "Project có media bị thiếu; mở editor để liên kết lại / Missing project media"
                );
            }
            return Ok(());
        }
        let mut args = strings(&[
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,width,height,duration:format=duration",
            "-of",
            "json",
        ]);
        args.push(item.path.to_string_lossy().into_owned());
        let probe: serde_json::Value = serde_json::from_slice(&run_tool("ffprobe", &args, 12)?)?;
        let streams = probe["streams"].as_array().context("Missing streams")?;
        let stream = streams
            .iter()
            .find(|s| s["codec_type"] == "video")
            .context("Missing video stream")?;
        let width = stream["width"].as_u64().unwrap_or(0) as u32;
        let height = stream["height"].as_u64().unwrap_or(0) as u32;
        if width == 0 || height == 0 {
            bail!("Không có hình hợp lệ / No valid video stream");
        }
        let duration = probe["format"]["duration"]
            .as_str()
            .or_else(|| stream["duration"].as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        if item.kind == Kind::Video && (!duration.is_finite() || duration <= 0.0) {
            bail!("Video chưa hoàn chỉnh / Incomplete video");
        }
        let thumb = dir.join("thumbnails").join(format!("{}.png", item.stamp()));
        fs::create_dir_all(thumb.parent().unwrap())?;
        if thumb.is_file() && image::image_dimensions(&thumb).is_err() {
            let _ = fs::remove_file(&thumb);
        }
        if !thumb.is_file() {
            let mut args = strings(&["-v", "error", "-nostdin", "-y", "-threads", "1", "-i"]);
            args.push(item.path.to_string_lossy().into_owned());
            args.extend(strings(&[
                "-frames:v",
                "1",
                "-vf",
                "scale=320:180:force_original_aspect_ratio=decrease",
                "-threads",
                "1",
            ]));
            args.push(thumb.to_string_lossy().into_owned());
            run_tool("ffmpeg", &args, 15)?;
        }
        item.metadata = Some(Metadata {
            duration,
            width,
            height,
            audio: streams.iter().any(|s| s["codec_type"] == "audio"),
        });
        item.thumbnail = Some(thumb);
        Ok(())
    })();
    match result {
        Ok(()) => {
            item.state = State::Ready;
            item.error = None;
        }
        Err(e) => {
            item.state = State::Recovery;
            item.error = Some(e.to_string());
        }
    }
    item
}
pub enum Request {
    Shutdown,
    Root(PathBuf),
    Refresh,
    Rebuild,
    Rename(String, String),
    Favorite(Vec<String>, bool),
    Trash(Vec<String>),
    Relink(String, PathBuf),
    Retry(String),
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub items: Arc<Vec<Item>>,
    pub roots: Vec<PathBuf>,
    pub revision: u64,
    pub busy: usize,
    pub message: Option<String>,
}
pub struct Service {
    pub tx: Sender<Request>,
    pub snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Service {
    pub fn start(root: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = snapshot.clone();
        let thread = std::thread::spawn(move || coordinator(root, rx, shared));
        Self {
            tx,
            snapshot,
            thread: Some(thread),
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn recycle(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("File không tồn tại / File is missing");
    }
    use windows::Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
            FOFX_ADDUNDORECORD, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE,
        },
    };
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = (|| -> Result<()> {
            let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
            op.SetOperationFlags(
                FOFX_RECYCLEONDELETE
                    | FOFX_ADDUNDORECORD
                    | FOFX_EARLYFAILURE
                    | FOF_NOCONFIRMATION
                    | FOF_NOERRORUI
                    | FOF_SILENT,
            )?;
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide(path).as_ptr()), None)?;
            op.DeleteItem(&item, None)?;
            op.PerformOperations()?;
            if op.GetAnyOperationsAborted()?.as_bool() || path.exists() {
                bail!("Không chuyển được file vào Recycle Bin / Recycle failed");
            }
            Ok(())
        })();
        CoUninitialize();
        result
    }
}

fn coordinator(root: PathBuf, rx: Receiver<Request>, shared: Arc<Mutex<Snapshot>>) {
    let dir = data_dir();
    use std::os::windows::fs::OpenOptionsExt;
    let _ = fs::create_dir_all(&dir);
    let _lock = match fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .share_mode(0)
        .open(dir.join("writer.lock"))
    {
        Ok(lock) => lock,
        Err(e) => {
            shared.lock().unwrap().message = Some(format!(
                "Thư viện đang mở ở cửa sổ khác hoặc không ghi được: {e}"
            ));
            shared.lock().unwrap().revision = 1;
            return;
        }
    };
    let (mut db, mut message) = Index::load(&dir);
    db.add_root(root);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let recordings = parent.join("recordings");
            if recordings.is_dir() {
                db.add_root(recordings);
            }
        }
    }
    let (jobs_tx, jobs_rx) = mpsc::sync_channel::<Item>(4);
    let jobs_rx = Arc::new(Mutex::new(jobs_rx));
    let (done_tx, done_rx) = mpsc::channel();
    for _ in 0..2 {
        let jobs = jobs_rx.clone();
        let done = done_tx.clone();
        let dir = dir.clone();
        std::thread::spawn(move || loop {
            let job = jobs.lock().unwrap().recv();
            let Ok(job) = job else { break };
            let stamp = job.stamp();
            if done.send((stamp, inspect(job, &dir))).is_err() {
                break;
            }
        });
    }
    drop(done_tx);
    let mut running = HashSet::new();
    let mut scan_at = Instant::now();
    let mut revision = 0;
    let mut dirty = true;
    let mut save_at = Instant::now();
    let mut needs_save = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Request::Shutdown) => {
                let _ = db.save(&dir);
                break;
            }
            Ok(request) => {
                let action = (|| -> Result<()> {
                    match request {
                        Request::Shutdown => unreachable!(),
                        Request::Root(p) => db.add_root(p),
                        Request::Refresh => {}
                        Request::Rebuild => {
                            db.events.clear();
                            for i in &mut db.items {
                                if i.kind != Kind::Session {
                                    i.metadata = None;
                                    i.thumbnail = None;
                                    i.error = None;
                                    i.state = State::Finalizing;
                                }
                            }
                        }
                        Request::Rename(id, name) => {
                            if let Some(i) = db.items.iter_mut().find(|i| i.id == id) {
                                let name = name.trim();
                                if name.is_empty() || name.chars().count() > 200 {
                                    bail!("Tên cần từ 1 đến 200 ký tự / Name must be 1–200 characters");
                                }
                                i.name = name.to_owned();
                            }
                        }
                        Request::Favorite(ids, favorite) => {
                            for i in &mut db.items {
                                if ids.contains(&i.id) {
                                    i.favorite = favorite;
                                }
                            }
                        }
                        Request::Trash(ids) => {
                            let mut errors = vec![];
                            for i in &mut db.items {
                                if ids.contains(&i.id) {
                                    if i.kind == Kind::Session || i.state == State::Finalizing {
                                        errors.push(format!("{}: đang xử lý / busy", i.name));
                                        continue;
                                    }
                                    match recycle(&i.path) {
                                        Ok(()) => {
                                            i.hidden = true;
                                            i.state = State::Missing;
                                        }
                                        Err(e) => errors.push(format!("{}: {e}", i.name)),
                                    }
                                }
                            }
                            if !errors.is_empty() {
                                bail!(errors.join("\n"));
                            }
                        }
                        Request::Relink(id, p) => {
                            let Some(pos) = db.items.iter().position(|i| i.id == id) else {
                                bail!("Missing entry")
                            };
                            if kind(&p) != Some(db.items[pos].kind.clone()) || !p.is_file() {
                                bail!("Loại file không phù hợp / Incompatible file type");
                            }
                            if db
                                .items
                                .iter()
                                .any(|i| i.id != id && path_key(&i.path) == path_key(&p))
                            {
                                bail!("File đã có trong thư viện / File already indexed");
                            }
                            let old = db.items[pos].path.clone();
                            db.items[pos].aliases.push(old.clone());
                            db.items[pos].path = p.clone();
                            db.items[pos].state = State::Missing;
                            for i in &mut db.items {
                                for r in i
                                    .sources
                                    .iter_mut()
                                    .chain(i.project.iter_mut())
                                    .chain(i.exports.iter_mut())
                                {
                                    if path_key(r) == path_key(&old) {
                                        *r = p.clone();
                                    }
                                }
                            }
                        }
                        Request::Retry(id) => {
                            if let Some(i) = db.items.iter_mut().find(|i| i.id == id) {
                                i.state = State::Finalizing;
                                i.error = None;
                            }
                        }
                    }
                    Ok(())
                })();
                message = action.err().map(|e| e.to_string());
                dirty = true;
                scan_at = Instant::now() - Duration::from_secs(3);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = db.save(&dir);
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if scan_at.elapsed() >= Duration::from_secs(3) || revision == 0 {
            let before = serde_json::to_vec(&db).unwrap_or_default();
            let errors = db.scan();
            db.consume_events(&dir);
            db.link_projects();
            if !errors.is_empty() {
                message = Some(errors.join("\n"));
            }
            dirty |= before != serde_json::to_vec(&db).unwrap_or_default();
            scan_at = Instant::now();
        }
        let session_outputs = db
            .items
            .iter()
            .filter(|i| i.kind == Kind::Session)
            .map(session_output)
            .collect::<HashSet<_>>();
        for (stamp, result) in done_rx.try_iter() {
            running.remove(&stamp);
            if session_outputs.contains(&result.path) {
                continue;
            }
            if let Some(i) = db
                .items
                .iter_mut()
                .find(|i| i.id == result.id && i.stamp() == stamp && i.state == State::Finalizing)
            {
                i.metadata = result.metadata;
                i.thumbnail = result.thumbnail;
                i.state = result.state;
                i.error = result.error;
                i.sources = result.sources;
                dirty = true;
            }
        }
        for i in &db.items {
            if i.hidden
                || i.kind == Kind::Session
                || session_outputs.contains(&i.path)
                || i.state != State::Finalizing
                || now().saturating_sub(i.modified) < 2000
                || running.contains(&i.stamp())
            {
                continue;
            }
            match jobs_tx.try_send(i.clone()) {
                Ok(()) => {
                    running.insert(i.stamp());
                }
                Err(_) => break,
            }
        }
        if dirty {
            db.link_projects();
            revision += 1;
            *shared.lock().unwrap() = Snapshot {
                items: Arc::new(db.items.iter().filter(|i| !i.hidden).cloned().collect()),
                roots: db.roots.clone(),
                revision,
                busy: running.len(),
                message: message.clone(),
            };
            dirty = false;
            needs_save = true;
        }
        if needs_save && save_at.elapsed() >= Duration::from_secs(1) {
            if let Err(e) = db.save(&dir) {
                message = Some(format!("Không lưu được chỉ mục: {e}"));
                dirty = true;
            } else {
                needs_save = false;
            }
            save_at = Instant::now();
        }
    }
}

/// Headless diagnostic uses the same worker service as the native library.
pub fn diagnose(args: &[String]) -> Result<()> {
    let root = PathBuf::from(args.get(2).context("--library-scan ROOT REPORT.json")?);
    let report = PathBuf::from(args.get(3).context("Missing report path")?);
    let service = Service::start(root);
    let start = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let snapshot = service.snapshot.lock().unwrap().clone();
        let settled =
            snapshot.revision > 0 && snapshot.items.iter().all(|i| i.state != State::Finalizing);
        if (settled && start.elapsed() > Duration::from_secs(4))
            || start.elapsed() > Duration::from_secs(45)
        {
            atomic_write(
                &report,
                &serde_json::to_vec_pretty(
                    &serde_json::json!({"items": snapshot.items.as_ref(), "roots": snapshot.roots, "busy": snapshot.busy, "message": snapshot.message, "elapsed_ms": start.elapsed().as_millis(), "settled": settled}),
                )?,
            )?;
            // Allow the coordinator's debounced durable write before process exit.
            std::thread::sleep(Duration::from_millis(1200));
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(label: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("library-{label}-{}-{}", std::process::id(), now()));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn concat_project_metadata_and_legacy_are_preserved() {
        let dir = fixture("concat");
        for folder in ["cache", "creating-user-recordings"] {
            fs::create_dir_all(dir.join(folder)).unwrap();
            fs::write(dir.join(folder).join("keep.mp4"), "fixture").unwrap();
        }
        let source = dir.join("Bản quay.mp4");
        fs::write(&source, "source fixture").unwrap();
        let legacy = dir.join("old.taoclip");
        let old = serde_json::json!({"clips":[],"assets":[{"path":source}]});
        fs::write(&legacy, serde_json::to_vec(&old).unwrap()).unwrap();
        let project = dir.join("concat.json");
        fs::write(
            &project,
            serde_json::to_vec(&serde_json::json!({
                "video":{"width":640,"height":360}, "media":[{"path":source,"hasAudio":true}],
                "clips":[{"start":0.0,"duration":2.0},{"start":1.0,"duration":2.0}]
            }))
            .unwrap(),
        )
        .unwrap();
        let mut db = Index::default();
        db.add_root(dir.clone());
        db.scan();
        db.items = db
            .items
            .into_iter()
            .map(|i| {
                if i.kind == Kind::Project {
                    inspect(i, &dir)
                } else {
                    i
                }
            })
            .collect();
        db.link_projects();
        assert!(db
            .items
            .iter()
            .any(|i| i.path == dir.join("cache/keep.mp4")));
        assert!(db
            .items
            .iter()
            .any(|i| i.path == dir.join("creating-user-recordings/keep.mp4")));
        let item = db.items.iter().find(|i| i.path == source).unwrap();
        assert_eq!(item.project.as_ref(), Some(&project));
        let meta = db
            .items
            .iter()
            .find(|i| i.path == project)
            .unwrap()
            .metadata
            .as_ref()
            .unwrap();
        assert_eq!(
            (meta.duration, meta.width, meta.height, meta.audio),
            (3.0, 640, 360, true)
        );
        assert_eq!(
            fs::read(&legacy).unwrap(),
            serde_json::to_vec(&old).unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rename_move_missing_and_rebuild() {
        let root = fixture("scan");
        let file = root.join("Bản quay thử.mp4");
        fs::write(&file, "invalid").unwrap();
        let mut db = Index::default();
        db.add_root(root.clone());
        db.scan();
        assert_eq!(db.items.len(), 1);
        db.items[0].name = "Tên hiển thị".into();
        db.items[0].favorite = true;
        let id = db.items[0].id.clone();
        let new = root.join("Moved video.mp4");
        fs::rename(&file, &new).unwrap();
        db.scan();
        assert_eq!(db.items.len(), 1);
        assert_eq!(db.items[0].id, id);
        assert_eq!(db.items[0].name, "Tên hiển thị");
        let store = root.join(".catalog");
        db.save(&store).unwrap();
        fs::write(store.join("index.json"), b"bad").unwrap();
        let (loaded, warning) = Index::load(&store);
        assert!(warning.is_some());
        assert!(loaded.items[0].favorite);
        fs::remove_file(new).unwrap();
        db.scan();
        assert_eq!(db.items[0].state, State::Missing);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn project_unicode_and_missing_media() {
        let dir = fixture("project");
        let p = dir.join("Dự án.taoclip");
        fs::write(
            &p,
            r#"{"timelines":[{"clips":[{"path":"video có dấu.mp4"}]}]}"#,
        )
        .unwrap();
        let mut db = Index::default();
        db.add_root(dir.clone());
        db.scan();
        let i = inspect(db.items[0].clone(), &dir);
        assert_eq!(i.state, State::Recovery);
        assert_eq!(i.sources, vec![dir.join("video có dấu.mp4")]);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn media_probe_cache_invalidation_and_export_rebuild() {
        let dir = fixture("metadata");
        let video = dir.join("Bản quay không audio.mp4");
        let status = Command::new(crate::recorder::tool("ffmpeg").unwrap())
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=160x90:d=0.5",
                "-c:v",
                "libx264",
                "-threads",
                "1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&video)
            .creation_flags(0x08000000)
            .status()
            .unwrap();
        assert!(status.success());
        let bad = dir.join("File lỗi.mp4");
        fs::write(&bad, "broken media").unwrap();
        let mut db = Index::default();
        db.add_root(dir.clone());
        db.scan();
        let pos = db.items.iter().position(|i| i.path == video).unwrap();
        db.items[pos] = inspect(db.items[pos].clone(), &dir.join(".cache"));
        let item = &db.items[pos];
        assert_eq!(item.state, State::Ready);
        assert_eq!(item.metadata.as_ref().unwrap().width, 160);
        assert!(!item.metadata.as_ref().unwrap().audio);
        let thumb = item.thumbnail.clone().unwrap();
        let before = fs::metadata(&thumb).unwrap().modified().unwrap();
        let cached = inspect(item.clone(), &dir.join(".cache"));
        assert_eq!(cached.state, State::Ready);
        assert_eq!(fs::metadata(&thumb).unwrap().modified().unwrap(), before);
        let bad_item = db.items.iter().find(|i| i.path == bad).unwrap();
        assert_eq!(inspect(bad_item.clone(), &dir).state, State::Recovery);
        let project = dir.join("Project.taoclip");
        fs::write(
            &project,
            serde_json::json!({"timelines":[{"clips":[{"path": video}]}]}).to_string(),
        )
        .unwrap();
        let output = dir.join("Export.mp4");
        fs::copy(&video, &output).unwrap();
        fs::write(
            format!("{}.screenrecorder.json", output.display()),
            serde_json::json!({"version":1,"sources":[video],"project":project,"output":output})
                .to_string(),
        )
        .unwrap();
        let mut rebuilt = Index::default();
        rebuilt.add_root(dir.clone());
        rebuilt.scan();
        for item in &mut rebuilt.items {
            if item.kind == Kind::Project {
                *item = inspect(item.clone(), &dir);
            }
        }
        rebuilt.link_projects();
        assert!(
            rebuilt
                .items
                .iter()
                .find(|i| i.path == output)
                .unwrap()
                .is_export
        );
        assert_eq!(
            rebuilt
                .items
                .iter()
                .find(|i| i.path == video)
                .unwrap()
                .project,
            Some(project.clone())
        );
        assert!(rebuilt
            .items
            .iter()
            .find(|i| i.path == project)
            .unwrap()
            .exports
            .contains(&output));
        fs::write(&video, "replaced").unwrap();
        db.scan();
        assert_eq!(db.items[pos].state, State::Finalizing);
        assert!(db.items[pos].metadata.is_none());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn recycle_only_selected_fixture() {
        let dir = fixture("recycle");
        let selected = dir.join("selected.txt");
        let untouched = dir.join("untouched.txt");
        fs::write(&selected, "recycle test only").unwrap();
        fs::write(&untouched, "keep").unwrap();
        recycle(&selected).unwrap();
        assert!(!selected.exists());
        assert!(untouched.is_file());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn sessions_and_folder_change_preserve_old_recordings() {
        let dir = fixture("sessions");
        let old = dir.join("old");
        let new = dir.join("new");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("old.mp4"), "old").unwrap();
        fs::write(new.join("new.png"), "new").unwrap();
        let session = new.join(".sessions/Quay_fixture");
        fs::create_dir_all(&session).unwrap();
        fs::write(session.join("settings.json"), "{}").unwrap();
        let mut db = Index::default();
        db.add_root(old.clone());
        db.scan();
        db.add_root(new.clone());
        db.scan();
        assert_eq!(db.items.len(), 3);
        assert_eq!(
            db.items
                .iter()
                .find(|i| i.kind == Kind::Session)
                .unwrap()
                .state,
            State::Recovery
        );
        fs::write(session.join("active.pid"), std::process::id().to_string()).unwrap();
        db.scan();
        assert_eq!(
            db.items
                .iter()
                .find(|i| i.kind == Kind::Session)
                .unwrap()
                .state,
            State::Finalizing
        );
        assert_eq!(db.roots, vec![old, new]);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn queued_rename_favorite_flush_on_immediate_close() {
        let dir = fixture("shutdown");
        let media = dir.join("recordings");
        fs::create_dir_all(&media).unwrap();
        let source = media.join("Tên file gốc.png");
        fs::write(&source, b"unchanged fixture").unwrap();
        let catalog = dir.join("catalog");
        let previous = std::env::var_os("SCREENRECORDER_LIBRARY_DIR");
        std::env::set_var("SCREENRECORDER_LIBRARY_DIR", &catalog);
        let service = Service::start(media);
        let start = Instant::now();
        let id = loop {
            if let Some(i) = service.snapshot.lock().unwrap().items.first() {
                break i.id.clone();
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(20));
        };
        service
            .tx
            .send(Request::Rename(
                id.clone(),
                "Tên hiển thị tiếng Việt".into(),
            ))
            .unwrap();
        service.tx.send(Request::Favorite(vec![id], true)).unwrap();
        drop(service);
        if let Some(previous) = previous {
            std::env::set_var("SCREENRECORDER_LIBRARY_DIR", previous);
        } else {
            std::env::remove_var("SCREENRECORDER_LIBRARY_DIR");
        }
        let (loaded, _) = Index::load(&catalog);
        assert_eq!(loaded.items[0].name, "Tên hiển thị tiếng Việt");
        assert!(loaded.items[0].favorite);
        assert_eq!(loaded.items[0].path, source);
        assert_eq!(fs::read(&source).unwrap(), b"unchanged fixture");
        fs::remove_dir_all(dir).unwrap();
    }
}
