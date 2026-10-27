//! Native topmost controls. Both windows are excluded BEFORE being shown.
use crate::{
    model::Area,
    studio::{self, Action, Mark, Shared, Status, Tool},
};
use anyhow::Context;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::Duration,
};
use windows::{
    core::w,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
const PAD: i32 = 6;
pub struct Overlay {
    stop: Arc<AtomicBool>,
    pub actions: Receiver<Action>,
}
impl Drop for Overlay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
struct State {
    live: Shared,
    tx: Sender<Action>,
    frame: HWND,
    bar: HWND,
    bounds: Area,
    expanded: bool,
    compact: bool,
    detached: bool,
    typing: bool,
    ratio: f32,
    buttons: Vec<(RECT, i32)>,
    scale: f32,
    last_area: Area,
    last_revision: u64,
    last_status: Status,
    last_tool: Tool,
    last_desktop_capture: bool,
    dragging: bool,
    last_mouse: POINT,
    mouse_down: bool,
    hover: Option<i32>,
    focus: usize,
    adjust: Option<(u32, POINT, Area)>,
    bitmap: Vec<u8>,
    frame_content: Vec<u8>,
    bitmap_size: (u32, u32),
    snapshots: u8,
}
pub fn spawn(live: Shared, bounds: Area) -> Overlay {
    let (stop, flag) = (Arc::new(AtomicBool::new(false)), live.clone());
    let ending = stop.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Err(e) = unsafe { run(flag.clone(), bounds, tx, ending) } {
            flag.lock().unwrap().fault = Some(format!("Không mở được thanh điều khiển: {e:#}"));
        }
    });
    Overlay { stop, actions: rx }
}
unsafe fn run(
    live: Shared,
    bounds: Area,
    tx: Sender<Action>,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let module = GetModuleHandleW(None)?;
    for (class, proc) in [
        (
            w!("RecorderStudioFrame"),
            frame_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
        ),
        (w!("RecorderStudioToolbar"), bar_proc),
    ] {
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
    }
    let a = live.lock().unwrap().area;
    let mut s = Box::new(State {
        live,
        tx,
        frame: HWND::default(),
        bar: HWND::default(),
        bounds,
        expanded: false,
        compact: bounds.width < 1100,
        detached: false,
        typing: false,
        ratio: 0.0,
        buttons: vec![],
        scale: 1.0,
        last_area: Area::default(),
        last_revision: 0,
        last_status: Status::Busy,
        last_tool: Tool::Pointer,
        last_desktop_capture: false,
        dragging: false,
        last_mouse: POINT::default(),
        mouse_down: false,
        hover: None,
        focus: 0,
        adjust: None,
        bitmap: Vec::new(),
        frame_content: Vec::new(),
        bitmap_size: (0, 0),
        snapshots: 0,
    });
    let ptr = (&mut *s as *mut State).cast();
    s.frame = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        w!("RecorderStudioFrame"),
        w!("Khung ghi hình"),
        WS_POPUP,
        a.x - PAD,
        a.y - PAD,
        a.width as i32 + PAD * 2,
        a.height as i32 + PAD * 2,
        None,
        None,
        Some(module.into()),
        Some(ptr),
    )?;
    s.bar = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        w!("RecorderStudioToolbar"),
        w!("Screen Recorder • Điều khiển"),
        WS_POPUP,
        0,
        0,
        820,
        52,
        None,
        None,
        Some(module.into()),
        Some(ptr),
    )?;
    SetLayeredWindowAttributes(s.frame, COLORREF(0x00010101), 255, LWA_COLORKEY)?;
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(s.bar);
    let inspection_scale = if std::env::args().any(|a| a == "--inspect-ui") {
        std::env::var("SCREENRECORDER_UI_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(1.)
            .clamp(1., 1.5)
    } else {
        1.
    };
    s.scale = (dpi as f32 / 96.0 * inspection_scale)
        .min(bounds.width as f32 / 886.0)
        .max(0.7);
    // DWM needs an allocated surface for display affinity. Initialize off-screen,
    // then exclude both windows before moving either into the visible desktop.
    paint_frame(&mut s).context("Initial overlay bitmap")?;
    for hwnd in [s.frame, s.bar] {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            bounds.x + bounds.width as i32 + 10000,
            bounds.y + 10000,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    let _ = windows::Win32::Graphics::Dwm::DwmFlush();
    if let Err(e) = SetWindowDisplayAffinity(s.frame, WDA_EXCLUDEFROMCAPTURE)
        .context("Exclude annotation window")
        .and_then(|_| {
            SetWindowDisplayAffinity(s.bar, WDA_EXCLUDEFROMCAPTURE).context("Exclude toolbar")
        })
    {
        let _ = DestroyWindow(s.bar);
        let _ = DestroyWindow(s.frame);
        return Err(e);
    }
    arrange(&mut s, true);
    paint_frame(&mut s)?;
    SetTimer(Some(s.bar), 1, 33, None);
    let inspected = std::env::args().any(|a| a == "--inspect-ui");
    let mut msg = MSG::default();
    while !stop.load(Ordering::Relaxed) {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if inspected && s.snapshots == 1 {
            activate(&mut s, 5);
        } else if inspected && s.snapshots == 3 {
            activate(&mut s, 7);
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    let _ = KillTimer(Some(s.bar), 1);
    let _ = DestroyWindow(s.bar);
    let _ = DestroyWindow(s.frame);
    Ok(())
}
unsafe fn state(hwnd: HWND, msg: u32, lp: LPARAM) -> *mut State {
    if msg == WM_NCCREATE {
        let cs = &*(lp.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State
}
fn point(lp: LPARAM) -> (i32, i32) {
    (
        (lp.0 & 0xffff) as u16 as i16 as i32,
        ((lp.0 >> 16) & 0xffff) as u16 as i16 as i32,
    )
}
unsafe fn arrange(s: &mut State, force: bool) {
    let a = s.live.lock().unwrap().area;
    if force || a != s.last_area {
        let _ = SetWindowPos(
            s.frame,
            Some(HWND_TOPMOST),
            a.x - PAD,
            a.y - PAD,
            a.width as i32 + PAD * 2,
            a.height as i32 + PAD * 2,
            SWP_NOACTIVATE,
        );
        let w = ((if s.compact { 524.0 } else { 870.0 }) * s.scale) as i32;
        let h = ((if s.expanded && !s.compact {
            164.0
        } else {
            70.0
        }) * s.scale) as i32;
        let mut old = RECT::default();
        let _ = GetWindowRect(s.bar, &mut old);
        let x = if s.detached { old.left } else { a.x };
        let y = if s.detached {
            old.top
        } else if a.y - h - 8 >= s.bounds.y {
            a.y - h - 8
        } else {
            a.y + a.height as i32 + 8
        };
        let _ = SetWindowPos(
            s.bar,
            Some(HWND_TOPMOST),
            x.clamp(
                s.bounds.x,
                (s.bounds.x + s.bounds.width as i32 - w).max(s.bounds.x),
            ),
            y.clamp(
                s.bounds.y,
                (s.bounds.y + s.bounds.height as i32 - h).max(s.bounds.y),
            ),
            w,
            h,
            SWP_NOACTIVATE,
        );
        let rounded = CreateRoundRectRgn(
            0,
            0,
            w + 1,
            h + 1,
            (12.0 * s.scale) as i32,
            (12.0 * s.scale) as i32,
        );
        SetWindowRgn(s.bar, Some(rounded), true);
        s.last_area = a;
    }
}
unsafe extern "system" fn bar_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let ptr = state(hwnd, msg, lp);
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let s = &mut *ptr;
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEMOVE => {
            let (x, y) = point(lp);
            s.hover = s
                .buttons
                .iter()
                .find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
                .map(|(_, id)| *id);
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let (x, y) = point(lp);
            let mut r = RECT::default();
            let _ = GetWindowRect(hwnd, &mut r);
            if (((x - r.left) as f32) < 28.0 * s.scale || ((y - r.top) as f32) >= 48.0 * s.scale)
                && ((y - r.top) as f32) < 70.0 * s.scale
            {
                LRESULT(HTCAPTION as isize)
            } else {
                LRESULT(HTCLIENT as isize)
            }
        }
        WM_ENTERSIZEMOVE => {
            s.detached = true;
            LRESULT(0)
        }
        WM_EXITSIZEMOVE => {
            arrange(s, true);
            LRESULT(0)
        }
        WM_TIMER => {
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            let down = GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0
                || GetAsyncKeyState(VK_RBUTTON.0 as i32) < 0;
            let (mut dirty, mut interactive, mut affinity) = (false, false, None);
            {
                let mut l = s.live.lock().unwrap();
                if l.cursor_highlight || l.click_effect {
                    let local = (p.x - l.area.x, p.y - l.area.y);
                    l.pointer = if local.0 >= 0
                        && local.1 >= 0
                        && local.0 < l.area.width as i32
                        && local.1 < l.area.height as i32
                    {
                        Some((local.0 as f32, local.1 as f32))
                    } else {
                        None
                    };
                    if down && !s.mouse_down {
                        l.click_at = l.pointer.map(|v| (v, std::time::Instant::now()));
                    }
                    dirty = p.x != s.last_mouse.x
                        || p.y != s.last_mouse.y
                        || l.click_at
                            .is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 0.7);
                }
                dirty |= !l.marks.is_empty() || l.tool != Tool::Pointer;
                dirty |= l.revision != s.last_revision
                    || l.status != s.last_status
                    || l.tool != s.last_tool
                    || l.area != s.last_area;
                if l.tool != s.last_tool || l.status != s.last_status {
                    interactive = true;
                }
                if l.desktop_capture != s.last_desktop_capture {
                    affinity = Some(l.desktop_capture);
                    dirty = true;
                }
                s.last_revision = l.revision;
                s.last_status = l.status;
                s.last_tool = l.tool;
                s.last_desktop_capture = l.desktop_capture;
            }
            if let Some(include_annotations) = affinity {
                if let Err(error) = SetWindowDisplayAffinity(
                    s.frame,
                    if include_annotations {
                        WDA_NONE
                    } else {
                        WDA_EXCLUDEFROMCAPTURE
                    },
                ) {
                    s.live.lock().unwrap().fault =
                        Some(format!("Không đổi được chế độ ghép chú thích GPU: {error}"));
                }
            }
            s.last_mouse = p;
            s.mouse_down = down;
            arrange(s, false);
            if interactive {
                let l = s.live.lock().unwrap();
                let mut ex = GetWindowLongPtrW(s.frame, GWL_EXSTYLE) as u32;
                if l.tool == Tool::Pointer && l.status != Status::Ready {
                    ex |= WS_EX_TRANSPARENT.0
                } else {
                    ex &= !WS_EX_TRANSPARENT.0
                }
                SetWindowLongPtrW(s.frame, GWL_EXSTYLE, ex as isize);
            }
            if dirty {
                if let Err(e) = paint_frame(s) {
                    s.live.lock().unwrap().fault = Some(e.to_string());
                }
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lp);
            let id = s
                .buttons
                .iter()
                .find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
                .map(|(_, id)| *id);
            if let Some(id) = id {
                activate(s, id);
            }
            let _ = SetFocus(Some(hwnd));
            LRESULT(0)
        }
        WM_SYSKEYDOWN if [VK_LEFT.0, VK_RIGHT.0, VK_UP.0, VK_DOWN.0].contains(&(wp.0 as u16)) => {
            bar_proc(hwnd, WM_KEYDOWN, wp, lp)
        }
        WM_KEYDOWN => {
            if GetKeyState(VK_MENU.0 as i32) < 0
                && [VK_LEFT.0, VK_RIGHT.0, VK_UP.0, VK_DOWN.0].contains(&(wp.0 as u16))
            {
                let mut r = RECT::default();
                let _ = GetWindowRect(hwnd, &mut r);
                let dx = if wp.0 == VK_LEFT.0 as usize {
                    -16
                } else if wp.0 == VK_RIGHT.0 as usize {
                    16
                } else {
                    0
                };
                let dy = if wp.0 == VK_UP.0 as usize {
                    -16
                } else if wp.0 == VK_DOWN.0 as usize {
                    16
                } else {
                    0
                };
                s.detached = true;
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left + dx,
                    r.top + dy,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER,
                );
                arrange(s, true);
                return LRESULT(0);
            }
            if !s.typing && wp.0 == VK_TAB.0 as usize {
                if !s.buttons.is_empty() {
                    s.focus = if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        (s.focus + s.buttons.len() - 1) % s.buttons.len()
                    } else {
                        (s.focus + 1) % s.buttons.len()
                    };
                    s.hover = Some(s.buttons[s.focus].1);
                }
                return LRESULT(0);
            }
            if !s.typing && (wp.0 == VK_SPACE.0 as usize || wp.0 == VK_RETURN.0 as usize) {
                if let Some((_, id)) = s.buttons.get(s.focus).copied() {
                    activate(s, id);
                }
                return LRESULT(0);
            }
            if wp.0 == VK_ESCAPE.0 as usize {
                let mut live = s.live.lock().unwrap();
                let dismiss =
                    live.status == Status::Ready && live.tool == Tool::Pointer && !s.typing;
                live.tool = Tool::Pointer;
                live.draft = None;
                live.changed();
                s.typing = false;
                drop(live);
                if dismiss {
                    let _ = s.tx.send(Action::Dismiss);
                }
            }
            if GetKeyState(VK_CONTROL.0 as i32) < 0 {
                if wp.0 == b'Z' as usize {
                    s.live.lock().unwrap().undo();
                }
                if wp.0 == b'Y' as usize {
                    s.live.lock().unwrap().redo();
                }
            }
            LRESULT(0)
        }
        WM_CHAR if s.typing => {
            let mut l = s.live.lock().unwrap();
            if wp.0 == 8 {
                l.text.pop();
            } else if wp.0 == 13 {
                s.typing = false;
            } else if let Some(ch) = char::from_u32(wp.0 as u32) {
                if !ch.is_control() && l.text.chars().count() < 160 {
                    l.text.push(ch);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            paint_bar(s);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = s.tx.send(Action::ShowMain);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
unsafe fn activate(s: &mut State, id: i32) {
    match id {
        1 => {
            let _ = s.tx.send(Action::StartStop);
        }
        2 => {
            let _ = s.tx.send(Action::Pause);
        }
        3 => {
            let mut l = s.live.lock().unwrap();
            if l.audio_available[1] {
                l.mic_on = !l.mic_on;
            }
        }
        4 => {
            let mut l = s.live.lock().unwrap();
            if l.audio_available[0] {
                l.system_on = !l.system_on;
            }
        }
        5 => {
            s.expanded = !s.expanded;
            s.compact = false;
            if !s.expanded {
                s.live.lock().unwrap().tool = Tool::Pointer;
            }
            arrange(s, true);
        }
        6 => {
            let _ = s.tx.send(Action::Screenshot);
        }
        7 => {
            s.compact = !s.compact;
            if s.compact {
                s.expanded = false;
                s.live.lock().unwrap().tool = Tool::Pointer;
            }
            arrange(s, true);
        }
        8 => {
            let _ = s.tx.send(Action::ShowMain);
        }
        9 => {
            let _ = s.tx.send(Action::Dismiss);
        }
        10..=17 => {
            s.live.lock().unwrap().tool = Tool::ALL[(id - 10) as usize].0;
            s.typing = false;
        }
        20 => s.live.lock().unwrap().undo(),
        21 => s.live.lock().unwrap().redo(),
        22 => s.live.lock().unwrap().clear(),
        23 => {
            s.typing = !s.typing;
            if s.typing {
                s.live.lock().unwrap().text.clear();
            }
        }
        24 => {
            let mut l = s.live.lock().unwrap();
            l.width = if l.width >= 8.0 { 2.0 } else { l.width + 2.0 };
        }
        25 => {
            s.ratio = if s.ratio == 0.0 {
                16.0 / 9.0
            } else if s.ratio > 1.5 {
                4.0 / 3.0
            } else {
                0.0
            };
        }
        30..=34 => {
            s.live.lock().unwrap().color = COLORS[(id - 30) as usize];
        }
        _ => {}
    }
}
const COLORS: [[u8; 3]; 5] = [
    [255, 193, 64],
    [255, 82, 102],
    [88, 192, 255],
    [95, 225, 167],
    [255, 255, 255],
];
unsafe fn fill(dc: HDC, r: RECT, c: u32) {
    let b = CreateSolidBrush(COLORREF(c));
    FillRect(dc, &r, b);
    let _ = DeleteObject(b.into());
}
unsafe fn text(dc: HDC, r: RECT, t: &str, c: u32) {
    SetTextColor(dc, COLORREF(c));
    let mut r = r;
    let mut v = t.encode_utf16().collect::<Vec<_>>();
    DrawTextW(
        dc,
        &mut v,
        &mut r,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
}
fn rect(s: f32, x: f32, y: f32, w: f32, h: f32) -> RECT {
    RECT {
        left: (x * s) as i32,
        top: (y * s) as i32,
        right: ((x + w) * s) as i32,
        bottom: ((y + h) * s) as i32,
    }
}
unsafe fn button(s: &mut State, dc: HDC, id: i32, r: RECT, t: &str, active: bool, danger: bool) {
    let color = if danger {
        0x004e3eda
    } else if active || s.hover == Some(id) {
        0x00694e2d
    } else {
        0x003b2b1f
    };
    let brush = CreateSolidBrush(COLORREF(color));
    let old_brush = SelectObject(dc, brush.into());
    let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    let _ = RoundRect(
        dc,
        r.left,
        r.top,
        r.right,
        r.bottom,
        (7.0 * s.scale) as i32,
        (7.0 * s.scale) as i32,
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    let _ = DeleteObject(brush.into());
    text(dc, r, t, if active { 0x00ffda9a } else { 0x00f5f0eb });
    s.buttons.push((r, id));
}
unsafe fn icon_button(
    s: &mut State,
    dc: HDC,
    id: i32,
    r: RECT,
    glyph: &str,
    active: bool,
    danger: bool,
) {
    let color = if danger {
        0x004e3eda
    } else if active || s.hover == Some(id) {
        0x00694e2d
    } else {
        0x003b2b1f
    };
    let brush = CreateSolidBrush(COLORREF(color));
    let old_brush = SelectObject(dc, brush.into());
    let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    let _ = RoundRect(
        dc,
        r.left,
        r.top,
        r.right,
        r.bottom,
        (8.0 * s.scale) as i32,
        (8.0 * s.scale) as i32,
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    let _ = DeleteObject(brush.into());
    let icon_font = CreateFontW(
        -(18.0 * s.scale) as i32,
        0,
        0,
        0,
        500,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe MDL2 Assets"),
    );
    let old_font = SelectObject(dc, icon_font.into());
    text(dc, r, glyph, if active { 0x00ffda9a } else { 0x00f5f0eb });
    SelectObject(dc, old_font);
    let _ = DeleteObject(icon_font.into());
    s.buttons.push((r, id));
}

fn tooltip(id: i32, lang: crate::model::Language, status: Status) -> &'static str {
    match id {
        1 if status == Status::Ready => lang.text("Bắt đầu ghi", "Start recording"),
        1 => lang.text("Dừng và lưu", "Stop and save"),
        2 if status == Status::Paused => lang.text("Tiếp tục", "Resume"),
        2 => lang.text("Tạm dừng", "Pause"),
        3 => lang.text("Bật / tắt micro", "Toggle microphone"),
        4 => lang.text("Bật / tắt âm thanh máy", "Toggle system audio"),
        5 => lang.text("Công cụ chú thích", "Annotation tools"),
        6 => lang.text("Chụp ảnh PNG", "Take PNG screenshot"),
        7 => lang.text(
            "Thu gọn / mở rộng · kéo dải dưới để di chuyển",
            "Compact / expand · drag the bottom strip to move",
        ),
        8 => lang.text("Mở ứng dụng", "Open application"),
        9 => lang.text("Đóng khung và toolbar", "Close frame and toolbar"),
        10 => lang.text("Thao tác phần mềm", "Interact with software"),
        11 => lang.text("Bút vẽ", "Pen"),
        12 => lang.text("Bút đánh dấu", "Highlighter"),
        13 => lang.text("Mũi tên", "Arrow"),
        14 => lang.text("Hình chữ nhật", "Rectangle"),
        15 => lang.text("Hình tròn", "Ellipse"),
        16 => lang.text("Thêm chữ", "Text"),
        17 => lang.text("Đánh số", "Number"),
        20 => lang.text("Hoàn tác", "Undo"),
        21 => lang.text("Làm lại", "Redo"),
        22 => lang.text("Xóa chú thích", "Clear annotations"),
        23 => lang.text("Nội dung chữ", "Text content"),
        24 => lang.text("Độ dày nét", "Stroke width"),
        25 => lang.text("Khóa tỷ lệ vùng", "Lock region ratio"),
        _ => "",
    }
}

unsafe fn annotation_button(s: &mut State, dc: HDC, id: i32, r: RECT, tool: Tool, active: bool) {
    let background = if active || s.hover == Some(id) {
        0x00694e2d
    } else {
        0x003b2b1f
    };
    let brush = CreateSolidBrush(COLORREF(background));
    let old_brush = SelectObject(dc, brush.into());
    let old_null_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    let _ = RoundRect(
        dc,
        r.left,
        r.top,
        r.right,
        r.bottom,
        (8.0 * s.scale) as i32,
        (8.0 * s.scale) as i32,
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_null_pen);
    let _ = DeleteObject(brush.into());

    let color = COLORREF(if active { 0x00ffda9a } else { 0x00f5f0eb });
    let pen = CreatePen(PS_SOLID, (2.0 * s.scale).round().max(1.0) as i32, color);
    let old_pen = SelectObject(dc, pen.into());
    let old_shape_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
    let z = s.scale;
    let cx = (r.left + r.right) / 2;
    let cy = (r.top + r.bottom) / 2;
    let segment = |x1: f32, y1: f32, x2: f32, y2: f32| {
        let _ = MoveToEx(
            dc,
            cx + (x1 * z).round() as i32,
            cy + (y1 * z).round() as i32,
            None,
        );
        let _ = LineTo(
            dc,
            cx + (x2 * z).round() as i32,
            cy + (y2 * z).round() as i32,
        );
    };
    match tool {
        Tool::Pointer => {
            for (a, b) in [
                ((-7., -9.), (-7., 8.)),
                ((-7., -9.), (6., 2.)),
                ((6., 2.), (0., 3.)),
                ((0., 3.), (4., 9.)),
                ((4., 9.), (0., 11.)),
                ((0., 11.), (-3., 5.)),
                ((-3., 5.), (-7., 8.)),
            ] {
                segment(a.0, a.1, b.0, b.1);
            }
        }
        Tool::Pen => {
            segment(-8., 7., 5., -6.);
            segment(-6., 9., 7., -4.);
            segment(5., -6., 7., -4.);
            segment(-8., 7., -6., 9.);
            segment(-9., 10., -3., 9.);
        }
        Tool::Highlight => {
            let wide = CreatePen(PS_SOLID, (5.0 * z).round().max(2.0) as i32, color);
            SelectObject(dc, wide.into());
            segment(-6., 5., 5., -6.);
            SelectObject(dc, pen.into());
            let _ = DeleteObject(wide.into());
            segment(-9., 9., 7., 9.);
            segment(5., -6., 8., -3.);
        }
        Tool::Arrow => {
            segment(-8., 8., 8., -8.);
            segment(1., -8., 8., -8.);
            segment(8., -8., 8., -1.);
        }
        Tool::Rectangle => {
            let _ = Rectangle(
                dc,
                cx - (9.0 * z) as i32,
                cy - (7.0 * z) as i32,
                cx + (9.0 * z) as i32,
                cy + (7.0 * z) as i32,
            );
        }
        Tool::Ellipse => {
            let _ = Ellipse(
                dc,
                cx - (9.0 * z) as i32,
                cy - (7.0 * z) as i32,
                cx + (9.0 * z) as i32,
                cy + (7.0 * z) as i32,
            );
        }
        Tool::Text | Tool::Number => {
            SelectObject(dc, old_pen);
            let font = CreateFontW(
                -(18.0 * z) as i32,
                0,
                0,
                0,
                600,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            );
            let old_font = SelectObject(dc, font.into());
            text(dc, r, if tool == Tool::Text { "T" } else { "1" }, color.0);
            SelectObject(dc, old_font);
            let _ = DeleteObject(font.into());
            let selected = SelectObject(dc, pen.into());
            let _ = selected;
        }
    }
    SelectObject(dc, old_shape_brush);
    SelectObject(dc, old_pen);
    let _ = DeleteObject(pen.into());
    s.buttons.push((r, id));
}
unsafe fn paint_bar(s: &mut State) {
    let mut ps = PAINTSTRUCT::default();
    let target = BeginPaint(s.bar, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(s.bar, &mut client);
    let dc = CreateCompatibleDC(Some(target));
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: client.right,
            biHeight: -client.bottom,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let Ok(bmp) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
        let _ = DeleteDC(dc);
        let _ = EndPaint(s.bar, &ps);
        return;
    };
    let old = SelectObject(dc, bmp.into());
    fill(dc, client, 0x001e140e);
    SetBkMode(dc, TRANSPARENT);
    let font = CreateFontW(
        -(13.0 * s.scale) as i32,
        0,
        0,
        0,
        500,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    );
    let old_font = SelectObject(dc, font.into());
    s.buttons.clear();
    let z = s.scale;
    let (status, seconds, countdown, mic, sys, tool, color, width, txt, levels, lang) = {
        let l = s.live.lock().unwrap();
        (
            l.status,
            l.seconds,
            l.countdown,
            l.mic_on,
            l.system_on,
            l.tool,
            l.color,
            l.width,
            l.text.clone(),
            l.levels,
            l.language,
        )
    };
    for x in [10., 17.] {
        for y in [19., 25., 31.] {
            fill(dc, rect(z, x, y, 2., 2.), 0x00948a80);
        }
    }
    let a = s.live.lock().unwrap().area;
    let title = match status {
        Status::Ready => format!("{} × {}", a.width, a.height),
        Status::Recording => format!("{} · {}×{}", lang.text("GHI", "REC"), a.width, a.height),
        Status::Paused => lang.text("Đã tạm dừng", "Paused").into(),
        Status::Countdown => lang.text("Đếm ngược", "Countdown").into(),
        Status::Starting => lang.text("Chuẩn bị", "Preparing").into(),
        Status::Busy => lang.text("Đang lưu", "Saving").into(),
    };
    let timer = if status == Status::Countdown {
        format!("{countdown}…")
    } else {
        let t = seconds as u64;
        format!("{:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
    };
    if s.compact {
        text(dc, rect(z, 28., 6., 112., 40.), &timer, 0x00f5f0eb);
        icon_button(
            s,
            dc,
            2,
            rect(z, 144., 8., 60., 36.),
            if status == Status::Paused {
                "\u{E768}"
            } else {
                "\u{E769}"
            },
            false,
            false,
        );
        icon_button(
            s,
            dc,
            1,
            rect(z, 210., 8., 64., 36.),
            if status == Status::Ready {
                "●"
            } else {
                "■"
            },
            false,
            true,
        );
        icon_button(
            s,
            dc,
            7,
            rect(z, 412., 8., 52., 36.),
            "\u{E96D}",
            false,
            false,
        );
        icon_button(
            s,
            dc,
            9,
            rect(z, 470., 8., 46., 36.),
            "\u{E8BB}",
            false,
            false,
        );
        icon_button(
            s,
            dc,
            3,
            rect(z, 280., 8., 60., 36.),
            if mic { "\u{E720}" } else { "\u{E74F}" },
            mic,
            false,
        );
        icon_button(
            s,
            dc,
            4,
            rect(z, 346., 8., 60., 36.),
            if sys { "\u{E767}" } else { "\u{E74F}" },
            sys,
            false,
        );
        if s.hover.is_none() {
            text(
                dc,
                rect(z, 28., 48., 488., 18.),
                &audio_status(&s.live),
                0x00c3b8aa,
            );
        }
        if let Some(id) = s.hover {
            text(
                dc,
                rect(z, 28., 48., 488., 18.),
                tooltip(id, lang, status),
                0x00c3b8aa,
            );
        }
    } else {
        text(
            dc,
            rect(z, 28., 3., 116., 21.),
            &title,
            if status == Status::Recording {
                0x00817bff
            } else {
                0x00b1a99f
            },
        );
        text(dc, rect(z, 28., 24., 116., 24.), &timer, 0x00f5f0eb);
        icon_button(
            s,
            dc,
            2,
            rect(z, 150., 8., 84., 36.),
            if status == Status::Paused {
                "\u{E768}"
            } else {
                "\u{E769}"
            },
            false,
            false,
        );
        icon_button(
            s,
            dc,
            1,
            rect(z, 240., 8., 88., 36.),
            if status == Status::Ready {
                "●"
            } else if status == Status::Countdown {
                "\u{E711}"
            } else {
                "■"
            },
            false,
            true,
        );
        icon_button(
            s,
            dc,
            3,
            rect(z, 342., 8., 84., 36.),
            "\u{E720}",
            mic,
            false,
        );
        icon_button(
            s,
            dc,
            4,
            rect(z, 432., 8., 84., 36.),
            "\u{E767}",
            sys,
            false,
        );
        for (x, level) in [
            (344.0, if mic { levels[1] } else { 0. }),
            (434.0, if sys { levels[0] } else { 0. }),
        ] {
            fill(
                dc,
                rect(z, x, 46., 80. * level.clamp(0., 1.), 2.),
                0x009fe15f,
            );
        }
        icon_button(
            s,
            dc,
            5,
            rect(z, 530., 8., 60., 36.),
            "\u{E70F}",
            s.expanded,
            false,
        );
        icon_button(
            s,
            dc,
            6,
            rect(z, 596., 8., 60., 36.),
            "\u{E722}",
            false,
            false,
        );
        icon_button(
            s,
            dc,
            8,
            rect(z, 662., 8., 80., 36.),
            "\u{E8A7}",
            false,
            false,
        );
        icon_button(
            s,
            dc,
            7,
            rect(z, 748., 8., 64., 36.),
            "\u{E96D}",
            false,
            false,
        );
        icon_button(
            s,
            dc,
            9,
            rect(z, 818., 8., 44., 36.),
            "\u{E8BB}",
            false,
            false,
        );
        if let Some(id) = s.hover {
            text(
                dc,
                rect(z, 150., 48., 712., 18.),
                tooltip(id, lang, status),
                0x00c3b8aa,
            );
        } else {
            text(
                dc,
                rect(z, 150., 48., 712., 18.),
                &audio_status(&s.live),
                0x008d8278,
            );
        }
        if s.expanded {
            for (i, (t, _)) in Tool::ALL.iter().enumerate() {
                annotation_button(
                    s,
                    dc,
                    10 + i as i32,
                    rect(z, 8. + i as f32 * 78., 72., 72., 32.),
                    *t,
                    *t == tool,
                );
            }
            icon_button(
                s,
                dc,
                20,
                rect(z, 638., 72., 52., 32.),
                "\u{E7A7}",
                false,
                false,
            );
            icon_button(
                s,
                dc,
                21,
                rect(z, 696., 72., 52., 32.),
                "\u{E7A6}",
                false,
                false,
            );
            icon_button(
                s,
                dc,
                22,
                rect(z, 754., 72., 58., 32.),
                "\u{E74D}",
                false,
                false,
            );
            for (i, c) in COLORS.iter().enumerate() {
                let r = rect(z, 8. + i as f32 * 28., 116., 22., 24.);
                fill(
                    dc,
                    r,
                    (c[0] as u32) | ((c[1] as u32) << 8) | ((c[2] as u32) << 16),
                );
                if color == *c {
                    text(dc, r, "✓", 0x00221b16);
                }
                s.buttons.push((r, 30 + i as i32));
            }
            button(
                s,
                dc,
                24,
                rect(z, 154., 114., 70., 30.),
                &format!("{width:.0} px"),
                false,
                false,
            );
            button(
                s,
                dc,
                23,
                rect(z, 232., 114., 340., 30.),
                &format!(
                    "{} {}",
                    if s.typing {
                        lang.text("Nhập chữ:", "Type:")
                    } else {
                        lang.text("Chữ:", "Text:")
                    },
                    txt
                ),
                s.typing,
                false,
            );
            button(
                s,
                dc,
                25,
                rect(z, 580., 114., 108., 30.),
                if s.ratio == 0.0 {
                    lang.text("Tỷ lệ: tự do", "Ratio: free")
                } else if s.ratio > 1.5 {
                    "16:9"
                } else {
                    "4:3"
                },
                false,
                false,
            );
            text(
                dc,
                rect(z, 696., 114., 116., 30.),
                lang.text("Esc: thao tác", "Esc: interact"),
                0x00b1a99f,
            );
        }
    }
    if std::env::args().any(|a| a == "--inspect-ui") {
        let key = if s.compact {
            4
        } else if s.expanded {
            2
        } else {
            1
        };
        if s.snapshots & key == 0 {
            s.snapshots |= key;
            let bytes = std::slice::from_raw_parts(
                bits.cast::<u8>(),
                client.right as usize * client.bottom as usize * 4,
            )
            .to_vec();
            let image =
                crate::capture::Image::from_bgra(client.right as u32, client.bottom as u32, bytes);
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("test-output/toolbar-{key}.png"));
            let _ = studio::save_png(&image, &path);
        }
    }
    let _ = BitBlt(
        target,
        0,
        0,
        client.right,
        client.bottom,
        Some(dc),
        0,
        0,
        SRCCOPY,
    );
    SelectObject(dc, old_font);
    let _ = DeleteObject(font.into());
    SelectObject(dc, old);
    let _ = DeleteObject(bmp.into());
    let _ = DeleteDC(dc);
    let _ = EndPaint(s.bar, &ps);
}
unsafe extern "system" fn frame_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let ptr = state(hwnd, msg, lp);
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let s = &mut *ptr;
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            draw_bitmap(s, dc);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_NCLBUTTONDOWN => {
            let l = s.live.lock().unwrap();
            if l.status == Status::Ready && l.allow_resize {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                s.adjust = Some((wp.0 as u32, p, l.area));
                SetCapture(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_MOUSEMOVE if s.adjust.is_some() => {
            let (edge, start, a) = s.adjust.unwrap();
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            let result = adjust_area(a, (p.x - start.x, p.y - start.y), edge, s.bounds, s.ratio);
            {
                let mut l = s.live.lock().unwrap();
                if l.area != result {
                    l.area = result;
                    l.clear();
                }
            }
            arrange(s, true);
            let _ = paint_frame(s);
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let l = s.live.lock().unwrap();
            if l.tool != Tool::Pointer {
                return LRESULT(HTCLIENT as isize);
            }
            if l.status != Status::Ready || !l.allow_resize {
                return LRESULT(HTTRANSPARENT as isize);
            }
            let (x, y) = point(lp);
            let a = l.area;
            let (left, right, top, bottom) = (
                x < a.x + 6,
                x >= a.x + a.width as i32 - 6,
                y < a.y + 6,
                y >= a.y + a.height as i32 - 6,
            );
            LRESULT(match (left, right, top, bottom) {
                (true, _, true, _) => HTTOPLEFT,
                (_, true, true, _) => HTTOPRIGHT,
                (true, _, _, true) => HTBOTTOMLEFT,
                (_, true, _, true) => HTBOTTOMRIGHT,
                (true, _, _, _) => HTLEFT,
                (_, true, _, _) => HTRIGHT,
                (_, _, true, _) => {
                    if (x - a.x - a.width as i32 / 2).abs() < 8 {
                        HTTOP
                    } else {
                        HTCAPTION
                    }
                }
                (_, _, _, true) => HTBOTTOM,
                _ => HTTRANSPARENT as u32,
            } as isize)
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lp.0 as *mut MINMAXINFO);
            info.ptMinTrackSize = POINT { x: 44, y: 44 };
            LRESULT(0)
        }
        WM_SIZING => {
            let r = &mut *(lp.0 as *mut RECT);
            if s.ratio > 0.0 {
                let h = ((r.right - r.left - PAD * 2) as f32 / s.ratio) as i32 + PAD * 2;
                if wp.0 == WMSZ_TOP as usize
                    || wp.0 == WMSZ_TOPLEFT as usize
                    || wp.0 == WMSZ_TOPRIGHT as usize
                {
                    r.top = r.bottom - h;
                } else {
                    r.bottom = r.top + h;
                }
            }
            LRESULT(1)
        }
        WM_EXITSIZEMOVE => {
            let mut r = RECT::default();
            let _ = GetWindowRect(hwnd, &mut r);
            let a = Area {
                x: (r.left + PAD).clamp(s.bounds.x, s.bounds.x + s.bounds.width as i32 - 32),
                y: (r.top + PAD).clamp(s.bounds.y, s.bounds.y + s.bounds.height as i32 - 32),
                width: (r.right - r.left - PAD * 2).max(32) as u32 & !1,
                height: (r.bottom - r.top - PAD * 2).max(32) as u32 & !1,
            };
            let mut a = a;
            a.width = a
                .width
                .min((s.bounds.x + s.bounds.width as i32 - a.x) as u32)
                & !1;
            a.height = a
                .height
                .min((s.bounds.y + s.bounds.height as i32 - a.y) as u32)
                & !1;
            {
                let mut l = s.live.lock().unwrap();
                l.area = a;
                l.clear();
            }
            arrange(s, true);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let mut l = s.live.lock().unwrap();
            if l.tool == Tool::Pointer {
                return DefWindowProcW(hwnd, msg, wp, lp);
            }
            let (x, y) = point(lp);
            let p = (
                (x - PAD).clamp(0, l.area.width as i32) as f32,
                (y - PAD).clamp(0, l.area.height as i32) as f32,
            );
            let txt = if l.tool == Tool::Number {
                (l.marks.iter().filter(|m| m.tool == Tool::Number).count() + 1).to_string()
            } else {
                l.text.clone()
            };
            l.draft = Some(Mark {
                tool: l.tool,
                points: vec![p, p],
                color: l.color,
                width: l.width,
                text: txt,
            });
            l.changed();
            s.dragging = true;
            SetCapture(hwnd);
            let _ = SetWindowPos(
                s.bar,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            LRESULT(0)
        }
        WM_MOUSEMOVE if s.dragging => {
            let mut l = s.live.lock().unwrap();
            let (x, y) = point(lp);
            let p = (
                (x - PAD).clamp(0, l.area.width as i32) as f32,
                (y - PAD).clamp(0, l.area.height as i32) as f32,
            );
            if let Some(m) = &mut l.draft {
                if matches!(m.tool, Tool::Pen | Tool::Highlight) {
                    if m.points.len() < 20000 {
                        m.points.push(p);
                    }
                } else if !matches!(m.tool, Tool::Text | Tool::Number) {
                    *m.points.last_mut().unwrap() = p;
                }
            }
            l.changed();
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            s.adjust = None;
            s.dragging = false;
            let _ = ReleaseCapture();
            s.live.lock().unwrap().finish_mark();
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            s.adjust = None;
            if s.dragging {
                s.dragging = false;
                let mut l = s.live.lock().unwrap();
                l.draft = None;
                l.changed();
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wp.0 == VK_ESCAPE.0 as usize {
                let mut l = s.live.lock().unwrap();
                let dismiss = l.status == Status::Ready && l.tool == Tool::Pointer;
                l.tool = Tool::Pointer;
                l.draft = None;
                l.changed();
                drop(l);
                if dismiss {
                    let _ = s.tx.send(Action::Dismiss);
                }
            }
            if GetKeyState(VK_CONTROL.0 as i32) < 0 {
                if wp.0 == b'Z' as usize {
                    s.live.lock().unwrap().undo();
                }
                if wp.0 == b'Y' as usize {
                    s.live.lock().unwrap().redo();
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
fn adjust_area(a: Area, delta: (i32, i32), edge: u32, b: Area, ratio: f32) -> Area {
    if edge == HTCAPTION {
        return Area {
            x: (a.x + delta.0).clamp(b.x, b.x + b.width as i32 - a.width as i32),
            y: (a.y + delta.1).clamp(b.y, b.y + b.height as i32 - a.height as i32),
            ..a
        };
    }
    let (mut x, mut y, mut right, mut bottom) =
        (a.x, a.y, a.x + a.width as i32, a.y + a.height as i32);
    if matches!(edge, HTLEFT | HTTOPLEFT | HTBOTTOMLEFT) {
        x = (x + delta.0).clamp(b.x, right - 32);
    }
    if matches!(edge, HTRIGHT | HTTOPRIGHT | HTBOTTOMRIGHT) {
        right = (right + delta.0).clamp(x + 32, b.x + b.width as i32);
    }
    if matches!(edge, HTTOP | HTTOPLEFT | HTTOPRIGHT) {
        y = (y + delta.1).clamp(b.y, bottom - 32);
    }
    if matches!(edge, HTBOTTOM | HTBOTTOMLEFT | HTBOTTOMRIGHT) {
        bottom = (bottom + delta.1).clamp(y + 32, b.y + b.height as i32);
    }
    if ratio > 0.0 {
        let mut w = (right - x) as f32;
        let mut h = (bottom - y) as f32;
        if edge == HTTOP || edge == HTBOTTOM {
            w = h * ratio;
        } else {
            h = w / ratio;
        }
        let factor = ((b.x + b.width as i32 - x) as f32 / w)
            .min((b.y + b.height as i32 - y) as f32 / h)
            .min(1.0);
        right = x + (w * factor) as i32;
        bottom = y + (h * factor) as i32;
    }
    Area {
        x,
        y,
        width: (right - x).max(32) as u32 & !1,
        height: (bottom - y).max(32) as u32 & !1,
    }
}
unsafe fn paint_frame(s: &mut State) -> anyhow::Result<()> {
    let mut l = s.live.lock().unwrap();
    let a = l.area;
    let w = a.width + PAD as u32 * 2;
    let h = a.height + PAD as u32 * 2;
    if a.width < 16 || a.height < 16 {
        return Ok(());
    }
    let ann = l.raster(a.width, a.height);
    s.frame_content.clear();
    s.frame_content.extend_from_slice(&ann);
    studio::effects_reuse(&mut s.frame_content, a.width, a.height, &mut l);
    s.bitmap.clear();
    s.bitmap.resize(w as usize * h as usize * 4, 0);
    let pixels = &mut s.bitmap;
    for y in 0..a.height as usize {
        let src = &s.frame_content[y * a.width as usize * 4..(y + 1) * a.width as usize * 4];
        let i = ((y + PAD as usize) * w as usize + PAD as usize) * 4;
        pixels[i..i + src.len()].copy_from_slice(src);
    }
    if l.tool != Tool::Pointer {
        for p in pixels.chunks_exact_mut(4) {
            if p[3] == 0 {
                p[3] = 1;
            }
        }
    }
    let c = match l.status {
        Status::Recording => [89, 80, 244, 255],
        Status::Paused => [64, 193, 255, 255],
        _ => [240, 185, 87, 255],
    };
    for y in 0..h as usize {
        for x in 0..w as usize {
            if x < 2 || y < 2 || x >= w as usize - 2 || y >= h as usize - 2 {
                let i = (y * w as usize + x) * 4;
                pixels[i..i + 4].copy_from_slice(&c);
            }
        }
    }
    if l.status == Status::Ready && l.allow_resize {
        for (cx, cy) in [
            (3, 3),
            (w as i32 / 2, 3),
            (w as i32 - 4, 3),
            (3, h as i32 / 2),
            (w as i32 - 4, h as i32 / 2),
            (3, h as i32 - 4),
            (w as i32 / 2, h as i32 - 4),
            (w as i32 - 4, h as i32 - 4),
        ] {
            for y in (cy - 3).max(0)..(cy + 4).min(h as i32) {
                for x in (cx - 3).max(0)..(cx + 4).min(w as i32) {
                    let i = (y as usize * w as usize + x as usize) * 4;
                    pixels[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    // In the GPU path Desktop Duplication captures this color-key surface. Alpha
    // is represented with a stable 4x4 dither, so annotations blend with the
    // live desktop without a BGRA readback or a stale background copy.
    let desktop_capture = l.desktop_capture;
    let raw = if desktop_capture {
        None
    } else {
        l.latest.clone()
    };
    let drawing = l.tool != Tool::Pointer;
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 4;
            let inside = x >= PAD as usize
                && y >= PAD as usize
                && x < (w - PAD as u32) as usize
                && y < (h - PAD as u32) as usize;
            let alpha = pixels[i + 3] as u32;
            if desktop_capture && inside {
                const BAYER: [[u32; 4]; 4] = [
                    [0, 128, 32, 160],
                    [192, 64, 224, 96],
                    [48, 176, 16, 144],
                    [240, 112, 208, 80],
                ];
                if alpha == 0 || alpha <= BAYER[y & 3][x & 3] {
                    pixels[i..i + 3].copy_from_slice(&[1, 1, 1]);
                }
            } else if inside && (alpha > 0 || drawing) {
                let bg = raw
                    .as_ref()
                    .filter(|im| im.width == a.width && im.height == a.height)
                    .map(|im| {
                        let j = ((y - PAD as usize) * a.width as usize + x - PAD as usize) * 4;
                        [im.pixels[j], im.pixels[j + 1], im.pixels[j + 2]]
                    })
                    .unwrap_or([30, 24, 18]);
                let alpha = if alpha == 1 { 0 } else { alpha };
                for j in 0..3 {
                    pixels[i + j] =
                        ((pixels[i + j] as u32 * alpha + bg[j] as u32 * (255 - alpha)) / 255) as u8;
                }
            } else if alpha == 0 {
                pixels[i..i + 3].copy_from_slice(&[1, 1, 1]);
            }
            pixels[i + 3] = 255;
        }
    }
    drop(l);
    s.bitmap_size = (w, h);
    let _ = InvalidateRect(Some(s.frame), None, false);
    Ok(())
}
unsafe fn draw_bitmap(s: &State, dc: HDC) {
    let (w, h) = s.bitmap_size;
    if w == 0 || h == 0 {
        return;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    SetDIBitsToDevice(
        dc,
        0,
        0,
        w,
        h,
        0,
        0,
        0,
        h,
        s.bitmap.as_ptr().cast(),
        &info,
        DIB_RGB_COLORS,
    );
}

fn audio_status(live: &Shared) -> String {
    let l = live.lock().unwrap();
    let lang = l.language;
    let state = |enabled: bool, available: bool| {
        if !available {
            lang.text("Không khả dụng", "Unavailable")
        } else if enabled {
            lang.text("Bật", "On")
        } else {
            lang.text("Tắt", "Off")
        }
    };
    format!(
        "Micro: {} · {}: {}",
        state(l.mic_on, l.audio_available[1]),
        lang.text("Tiếng máy", "System"),
        state(l.system_on, l.audio_available[0])
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_resize_stays_on_negative_origin_monitor() {
        let b = Area {
            x: -1280,
            y: 0,
            width: 1280,
            height: 720,
        };
        let a = Area {
            x: -1100,
            y: 100,
            width: 640,
            height: 360,
        };
        for edge in [
            HTLEFT,
            HTTOP,
            HTRIGHT,
            HTBOTTOM,
            HTTOPLEFT,
            HTTOPRIGHT,
            HTBOTTOMLEFT,
            HTBOTTOMRIGHT,
            HTCAPTION,
        ] {
            assert!(adjust_area(a, (3000, -3000), edge, b, 0.0).fits(b));
        }
        let r = adjust_area(a, (100, 20), HTBOTTOMRIGHT, b, 16.0 / 9.0);
        assert!(r.fits(b));
        assert!((r.width as f32 / r.height as f32 - 16.0 / 9.0).abs() < 0.02);
    }
}
