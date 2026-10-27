use crate::model::{Area, Language};
use anyhow::{bail, Result};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};
use windows::{
    core::{w, BOOL},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};

#[derive(Clone)]
pub struct Display {
    pub handle: isize,
    pub name: String,
    pub area: Area,
}
#[derive(Clone)]
pub struct AppWindow {
    pub handle: isize,
    pub name: String,
    pub process_id: u32,
    pub app: String,
    pub area: Area,
}

pub fn setup_dpi() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn displays() -> Vec<Display> {
    windows_capture::monitor::Monitor::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| unsafe {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(HMONITOR(m.as_raw_hmonitor()), &mut info).as_bool() {
                return None;
            }
            let r = info.rcMonitor;
            Some(Display {
                handle: m.as_raw_hmonitor() as isize,
                name: m.name().unwrap_or_else(|_| "Màn hình".into()),
                area: Area {
                    x: r.left,
                    y: r.top,
                    width: (r.right - r.left) as u32,
                    height: (r.bottom - r.top) as u32,
                },
            })
        })
        .collect()
}

pub fn app_windows() -> Vec<AppWindow> {
    windows_capture::window::Window::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter(|w| w.process_id().ok() != Some(std::process::id()))
        .filter_map(|w| {
            let name = w.title().ok()?;
            if name.trim().is_empty() {
                return None;
            }
            let process_id = w.process_id().ok()?;
            let area = window_area(w.as_raw_hwnd() as isize)?;
            if area.width < 32 || area.height < 32 {
                return None;
            }
            Some(AppWindow {
                handle: w.as_raw_hwnd() as isize,
                name,
                process_id,
                app: process_name(process_id).unwrap_or_else(|| format!("PID {process_id}")),
                area,
            })
        })
        .collect::<Vec<_>>()
}

fn process_name(process_id: u32) -> Option<String> {
    unsafe {
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize]))
            .file_stem()
            .map(|name| name.to_string_lossy().to_string())
    }
}

// A dedicated Win32 message loop keeps physical-pixel selection independent of UI scaling.
struct Picker {
    bounds: Area,
    language: Language,
    start: Option<(i32, i32)>,
    selected: Option<Area>,
    dragging: bool,
    result: Option<Area>,
}

unsafe extern "system" fn picker_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lp.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Picker;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let s = &mut *ptr;
    let point = || {
        (
            (lp.0 & 0xffff) as u16 as i16 as i32,
            ((lp.0 >> 16) & 0xffff) as u16 as i16 as i32,
        )
    };
    match msg {
        WM_LBUTTONDOWN => {
            s.start = Some(point());
            s.dragging = true;
            SetCapture(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE if s.dragging => {
            let p = point();
            let p = (
                p.0.clamp(0, s.bounds.width as i32),
                p.1.clamp(0, s.bounds.height as i32),
            );
            s.selected = Some(Area::from_points(s.start.unwrap(), p));
            let _ = InvalidateRect(Some(hwnd), None, true);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            s.dragging = false;
            let _ = ReleaseCapture();
            let _ = InvalidateRect(Some(hwnd), None, true);
            LRESULT(0)
        }
        WM_KEYDOWN if wp.0 == VK_ESCAPE.0 as usize => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN if [VK_LEFT.0, VK_RIGHT.0, VK_UP.0, VK_DOWN.0].contains(&(wp.0 as u16)) => {
            let a = s.selected.unwrap_or(Area {
                x: s.bounds.width as i32 / 4,
                y: s.bounds.height as i32 / 4,
                width: (s.bounds.width / 2) & !1,
                height: (s.bounds.height / 2) & !1,
            });
            s.selected = Some(keyboard_region(
                a,
                s.bounds.width,
                s.bounds.height,
                wp.0 as u16,
                GetKeyState(VK_SHIFT.0 as i32) < 0,
            ));
            let _ = InvalidateRect(Some(hwnd), None, true);
            LRESULT(0)
        }
        WM_KEYDOWN if wp.0 == VK_RETURN.0 as usize => {
            if let Some(mut r) = s.selected {
                if r.width >= 16 && r.height >= 16 {
                    r.x += s.bounds.x;
                    r.y += s.bounds.y;
                    s.result = Some(r);
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            let brush = CreateSolidBrush(COLORREF(0x00201b16));
            FillRect(dc, &client, brush);
            let _ = DeleteObject(brush.into());
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, COLORREF(0x00ffffff));
            let instructions = s
                .language
                .text(
                    "Kéo chọn · Mũi tên: di chuyển · Shift+mũi tên: đổi cỡ · Enter: dùng · Esc: hủy",
                    "Drag to select · Arrows: move · Shift+arrows: resize · Enter: use · Esc: cancel",
                )
                .encode_utf16()
                .collect::<Vec<_>>();
            let _ = TextOutW(dc, 28, 26, &instructions);
            if let Some(r) = s.selected {
                let pen = CreatePen(PS_SOLID, 3, COLORREF(0x00efc06c));
                let old_pen = SelectObject(dc, pen.into());
                let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
                let _ = Rectangle(dc, r.x, r.y, r.x + r.width as i32, r.y + r.height as i32);
                SelectObject(dc, old_pen);
                SelectObject(dc, old_brush);
                let _ = DeleteObject(pen.into());
                let text = format!(
                    "{} × {} px  —  {}",
                    r.width,
                    r.height,
                    s.language
                        .text("Enter để dùng vùng này", "Enter to use this region")
                )
                .encode_utf16()
                .collect::<Vec<_>>();
                let _ = TextOutW(dc, r.x.max(24), (r.y - 26).max(58), &text);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn choose_area(bounds: Area, language: Language) -> Receiver<Result<Option<Area>>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = unsafe {
            (|| -> Result<Option<Area>> {
                let instance = GetModuleHandleW(None)?;
                let class = w!("ScreenRecorderPicker");
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(picker_proc),
                    hInstance: instance.into(),
                    lpszClassName: class,
                    hCursor: LoadCursorW(None, IDC_CROSS)?,
                    ..Default::default()
                };
                RegisterClassW(&wc);
                let mut state = Box::new(Picker {
                    bounds,
                    language,
                    start: None,
                    selected: None,
                    dragging: false,
                    result: None,
                });
                let title = language
                    .text("Chọn vùng ghi hình", "Select capture region")
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<_>>();
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                    class,
                    windows::core::PCWSTR(title.as_ptr()),
                    WS_POPUP | WS_VISIBLE,
                    bounds.x,
                    bounds.y,
                    bounds.width as i32,
                    bounds.height as i32,
                    None,
                    None,
                    Some(instance.into()),
                    Some((&mut *state as *mut Picker).cast()),
                )?;
                SetLayeredWindowAttributes(hwnd, COLORREF(0), 125, LWA_ALPHA)?;
                let _ = SetForegroundWindow(hwnd);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                Ok(state.result)
            })()
        };
        let _ = tx.send(result);
    });
    rx
}

pub fn hotkeys() -> Receiver<Result<u32, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || unsafe {
        let mods = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
        for (id, key) in [(1, VK_F9), (2, VK_F10)] {
            if RegisterHotKey(None, id, mods, key.0 as u32).is_err() {
                let _ = tx.send(Err(format!(
                    "Không đăng ký được Ctrl+Shift+F{}. Phím đang được ứng dụng khác dùng.",
                    if id == 1 { 9 } else { 10 }
                )));
            }
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY && tx.send(Ok(msg.wParam.0 as u32)).is_err() {
                break;
            }
        }
        let _ = UnregisterHotKey(None, 1);
        let _ = UnregisterHotKey(None, 2);
    });
    rx
}

pub fn free_space(path: &std::path::Path) -> Result<u64> {
    use windows::core::PCWSTR;
    let p: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut available = 0;
    unsafe {
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            PCWSTR(p.as_ptr()),
            Some(&mut available),
            None,
            None,
        )?;
    }
    if available < 256 * 1024 * 1024 {
        bail!("Ổ đĩa còn dưới 256 MB. Hãy chọn thư mục lưu khác.");
    }
    Ok(available)
}

pub fn exclude_main_window(exclude: bool) {
    use std::sync::atomic::{AtomicI32, Ordering};
    static LAST: AtomicI32 = AtomicI32::new(-1);
    let exclude = exclude && !std::env::args().any(|a| a == "--inspect-ui");
    let wanted = i32::from(exclude);
    if LAST.load(Ordering::Relaxed) == wanted {
        return;
    }
    unsafe extern "system" fn find(hwnd: HWND, param: LPARAM) -> BOOL {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            let mut name = [0u16; 128];
            let len = GetWindowTextW(hwnd, &mut name);
            if String::from_utf16_lossy(&name[..len as usize]) == "Screen Recorder" {
                *(param.0 as *mut HWND) = hwnd;
                return BOOL(0);
            }
        }
        BOOL(1)
    }
    unsafe {
        let mut hwnd = HWND::default();
        let _ = EnumWindows(Some(find), LPARAM((&mut hwnd as *mut HWND) as isize));
        if !hwnd.0.is_null()
            && SetWindowDisplayAffinity(
                hwnd,
                if exclude {
                    WDA_EXCLUDEFROMCAPTURE
                } else {
                    WDA_NONE
                },
            )
            .is_ok()
        {
            LAST.store(wanted, Ordering::Relaxed);
        }
    }
}

pub fn window_area(handle: isize) -> Option<Area> {
    unsafe {
        let mut r = RECT::default();
        windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
            HWND(handle as *mut _),
            windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut r as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
        .ok()?;
        Some(Area {
            x: r.left,
            y: r.top,
            width: (r.right - r.left).max(0) as u32 & !1,
            height: (r.bottom - r.top).max(0) as u32 & !1,
        })
    }
}

fn keyboard_region(mut a: Area, width: u32, height: u32, key: u16, resize: bool) -> Area {
    let dx = if key == VK_LEFT.0 {
        -10
    } else if key == VK_RIGHT.0 {
        10
    } else {
        0
    };
    let dy = if key == VK_UP.0 {
        -10
    } else if key == VK_DOWN.0 {
        10
    } else {
        0
    };
    if resize {
        a.width = (a.width as i32 + dx).clamp(16, (width as i32 - a.x).max(16)) as u32 & !1;
        a.height = (a.height as i32 + dy).clamp(16, (height as i32 - a.y).max(16)) as u32 & !1;
    } else {
        a.x = (a.x + dx).clamp(0, width.saturating_sub(a.width) as i32);
        a.y = (a.y + dy).clamp(0, height.saturating_sub(a.height) as i32);
    }
    a
}
#[cfg(test)]
mod keyboard_tests {
    use super::*;
    #[test]
    fn region_keyboard_moves_resizes_and_stays_in_bounds() {
        let mut a = Area {
            x: 0,
            y: 0,
            width: 320,
            height: 180,
        };
        for _ in 0..100 {
            a = keyboard_region(a, 640, 360, VK_RIGHT.0, false);
        }
        assert_eq!(a.x, 320);
        a = keyboard_region(a, 640, 360, VK_RIGHT.0, true);
        assert_eq!(a.width, 320);
        a = keyboard_region(a, 640, 360, VK_LEFT.0, true);
        assert_eq!(a.width, 310);
        assert!(a.fits(Area {
            x: 0,
            y: 0,
            width: 640,
            height: 360
        }));
    }
}
