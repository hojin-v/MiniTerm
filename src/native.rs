use crate::geometry::{configured_bounds, Rect, WindowPreferences};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::mem::{size_of, size_of_val, zeroed};
use windows_sys::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFOEXW,
        MONITOR_DEFAULTTONEAREST,
    },
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
    UI::WindowsAndMessaging::{
        GetForegroundWindow, IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow,
        MONITORINFOF_PRIMARY, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE,
    },
};

pub fn hwnd(handle: &impl HasWindowHandle) -> Result<isize, String> {
    match handle.window_handle().map_err(|e| e.to_string())?.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get()),
        _ => Err("MiniTerm은 Windows 전용입니다.".into()),
    }
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub name: String,
    pub primary: bool,
    pub work: Rect,
    handle: isize,
}

unsafe extern "system" fn collect_monitor(
    handle: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let mut info: MONITORINFOEXW = zeroed();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(handle, &mut info.monitorInfo) != 0 {
        let end = info
            .szDevice
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(info.szDevice.len());
        let work = info.monitorInfo.rcWork;
        (*(data as *mut Vec<Monitor>)).push(Monitor {
            name: String::from_utf16_lossy(&info.szDevice[..end]),
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            work: Rect {
                x: work.left,
                y: work.top,
                width: (work.right - work.left).max(1) as u32,
                height: (work.bottom - work.top).max(1) as u32,
            },
            handle: handle as isize,
        });
    }
    1
}

pub fn monitors() -> Result<Vec<Monitor>, String> {
    let mut monitors: Vec<Monitor> = Vec::new();
    if unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect_monitor),
            &mut monitors as *mut _ as LPARAM,
        )
    } == 0
        || monitors.is_empty()
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    monitors.sort_by_key(|monitor| (!monitor.primary, monitor.name.clone()));
    Ok(monitors)
}

pub fn target_bounds(
    hwnd: isize,
    scale: f64,
    expanded: bool,
    preferences: &WindowPreferences,
) -> Result<Rect, String> {
    let monitors = monitors()?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.name == preferences.monitor)
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .unwrap_or(&monitors[0]);
    // Moving onto another display updates the HWND DPI before its final DIP size is applied.
    // This runs only for an explicit monitor change or a disconnected display.
    unsafe {
        if MonitorFromWindow(hwnd as HWND, MONITOR_DEFAULTTONEAREST) as isize != monitor.handle
            && SetWindowPos(
                hwnd as HWND,
                std::ptr::null_mut(),
                monitor.work.x,
                monitor.work.y,
                0,
                0,
                SWP_NOACTIVATE
                    | SWP_NOZORDER
                    | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
            ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd as HWND);
        Ok(configured_bounds(
            monitor.work,
            if dpi == 0 { scale } else { dpi as f64 / 96.0 },
            expanded,
            preferences,
        ))
    }
}

pub fn window_bounds(hwnd: isize) -> Result<Rect, String> {
    let mut bounds: RECT = unsafe { zeroed() };
    if unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd as HWND, &mut bounds)
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(Rect {
        x: bounds.left,
        y: bounds.top,
        width: (bounds.right - bounds.left).max(1) as u32,
        height: (bounds.bottom - bounds.top).max(1) as u32,
    })
}

pub fn window_scale(hwnd: isize) -> f64 {
    unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd as HWND).max(96) as f64 / 96.0 }
}

pub fn dragged_position(hwnd: isize) -> Result<(String, crate::geometry::WindowAnchor), String> {
    let handle = unsafe { MonitorFromWindow(hwnd as HWND, MONITOR_DEFAULTTONEAREST) } as isize;
    let monitor = monitors()?
        .into_iter()
        .find(|m| m.handle == handle)
        .ok_or("이동한 모니터를 찾지 못했습니다.")?;
    Ok((
        monitor.name,
        crate::geometry::WindowAnchor::from_bounds(monitor.work, window_bounds(hwnd)?),
    ))
}

pub fn position(
    hwnd: isize,
    scale: f64,
    expanded: bool,
    preferences: &WindowPreferences,
) -> Result<(), String> {
    set_bounds(hwnd, target_bounds(hwnd, scale, expanded, preferences)?)
}

pub fn set_bounds(hwnd: isize, bounds: Rect) -> Result<(), String> {
    unsafe {
        if SetWindowPos(
            hwnd as HWND,
            std::ptr::null_mut(),
            bounds.x,
            bounds.y,
            bounds.width as i32,
            bounds.height as i32,
            SWP_NOACTIVATE | SWP_NOZORDER,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

pub fn animations_enabled() -> bool {
    let mut enabled: BOOL = 0;
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut _ as _,
            0,
        ) != 0
            && enabled != 0
    }
}

/// Windows 11's compositor supplies a fixed small antialiased corner. A binary
/// window region would prevent compositor rounding and leave staircase pixels.
#[derive(Default)]
pub struct WindowAppearance {
    rounded: bool,
    transparency: Option<u16>,
}
impl WindowAppearance {
    pub fn apply(
        &mut self,
        watch: &DisplayWatch,
        preferences: &WindowPreferences,
    ) -> Result<(), String> {
        let hwnd = watch.hwnd;
        watch.data.layered.store(
            preferences.transparency != 0,
            std::sync::atomic::Ordering::Release,
        );
        use windows_sys::Win32::{
            Foundation::{GetLastError, SetLastError},
            Graphics::Dwm::{
                DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUNDSMALL,
            },
            UI::WindowsAndMessaging::{
                GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE,
                LWA_ALPHA, WS_EX_LAYERED,
            },
        };
        unsafe {
            if self.transparency != Some(preferences.transparency) {
                let previous = GetWindowLongPtrW(hwnd as HWND, GWL_EXSTYLE);
                let next = if preferences.transparency == 0 {
                    previous & !(WS_EX_LAYERED as isize)
                } else {
                    previous | WS_EX_LAYERED as isize
                };
                if previous != next {
                    SetLastError(0);
                    if SetWindowLongPtrW(hwnd as HWND, GWL_EXSTYLE, next) == 0
                        && GetLastError() != 0
                    {
                        return Err(std::io::Error::last_os_error().to_string());
                    }
                }
                if preferences.transparency != 0
                    && SetLayeredWindowAttributes(
                        hwnd as HWND,
                        0,
                        ((100 - preferences.transparency) as u32 * 255 / 100) as u8,
                        LWA_ALPHA,
                    ) == 0
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                self.transparency = Some(preferences.transparency);
            }
            if !self.rounded {
                let preference = DWMWCP_ROUNDSMALL;
                let result = DwmSetWindowAttribute(
                    hwnd as HWND,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    &preference as *const _ as _,
                    size_of_val(&preference) as u32,
                );
                if result < 0 {
                    return Err(format!("창 모서리를 적용하지 못했습니다: {result:#x}"));
                }
                self.rounded = true;
            }
        }
        Ok(())
    }
}

pub fn show(hwnd: isize, focus: bool) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_SHOWNOACTIVATE);
        if focus {
            SetForegroundWindow(hwnd as HWND);
        }
    }
}
pub fn hide(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_HIDE);
    }
}
pub fn visible(hwnd: isize) -> bool {
    unsafe { IsWindowVisible(hwnd as HWND) != 0 }
}
pub fn focused(hwnd: isize) -> bool {
    unsafe { GetForegroundWindow() == hwnd as HWND }
}
pub fn foreground() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

struct DisplayWatchData {
    changed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ctx: egui::Context,
    shortcut: std::sync::atomic::AtomicBool,
    header_regions: std::sync::Mutex<Vec<egui::Rect>>,
    body_region: std::sync::Mutex<Option<egui::Rect>>,
    suppress_release: std::sync::atomic::AtomicBool,
    last_header_press: std::sync::Mutex<Option<(std::time::Instant, egui::Pos2)>>,
    expand: std::sync::atomic::AtomicBool,
    layered: std::sync::atomic::AtomicBool,
    moving: std::sync::atomic::AtomicBool,
    moved: std::sync::atomic::AtomicBool,
    expanded: std::sync::atomic::AtomicBool,
}
pub struct DisplayWatch {
    hwnd: isize,
    data: Box<DisplayWatchData>,
    registered: std::cell::RefCell<Option<Shortcut>>,
}
impl DisplayWatch {
    pub fn context(&self) -> &egui::Context {
        &self.data.ctx
    }
    pub fn request_repaint(&self) {
        self.data.ctx.request_repaint();
    }
    pub fn new(hwnd: isize, ctx: egui::Context) -> Result<Self, String> {
        let data = Box::new(DisplayWatchData {
            changed: Default::default(),
            ctx,
            shortcut: Default::default(),
            header_regions: Default::default(),
            body_region: Default::default(),
            suppress_release: Default::default(),
            last_header_press: Default::default(),
            expand: Default::default(),
            layered: Default::default(),
            moving: Default::default(),
            moved: Default::default(),
            expanded: Default::default(),
        });
        unsafe {
            if windows_sys::Win32::UI::Shell::SetWindowSubclass(
                hwnd as HWND,
                Some(display_callback),
                1,
                &*data as *const _ as usize,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(Self {
            hwnd,
            data,
            registered: Default::default(),
        })
    }
    pub fn take_changed(&self) -> bool {
        self.data
            .changed
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
    pub fn moving(&self) -> bool {
        self.data.moving.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn set_resize_mode(&self, expanded: bool) {
        self.data
            .expanded
            .store(expanded, std::sync::atomic::Ordering::Release);
    }
    pub fn take_moved(&self) -> bool {
        self.data
            .moved
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
    pub fn register_shortcut(&self, value: &str) -> Result<(), String> {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT,
        };
        let next = parse_shortcut(value)?;
        let previous = *self.registered.borrow();
        if next == previous {
            return Ok(());
        }
        unsafe {
            UnregisterHotKey(self.hwnd as HWND, 1);
        }
        if next.is_none() {
            *self.registered.borrow_mut() = None;
            return Ok(());
        }
        let shortcut = next.unwrap();
        if unsafe {
            RegisterHotKey(
                self.hwnd as HWND,
                1,
                shortcut.modifiers | MOD_NOREPEAT,
                shortcut.key,
            )
        } == 0
        {
            let error = std::io::Error::last_os_error();
            if let Some(previous) = previous {
                let restored = unsafe {
                    RegisterHotKey(
                        self.hwnd as HWND,
                        1,
                        previous.modifiers | MOD_NOREPEAT,
                        previous.key,
                    )
                };
                if restored == 0 {
                    *self.registered.borrow_mut() = None;
                    return Err(format!("{value} 등록 실패: {error}. 이전 단축키도 복원하지 못했습니다: {}. 트레이에서 열 수 있습니다.", std::io::Error::last_os_error()));
                }
            }
            return Err(format!(
                "{value} 등록 실패: {error}. 다른 단축키를 지정하거나 트레이에서 열 수 있습니다."
            ));
        }
        *self.registered.borrow_mut() = next;
        Ok(())
    }
    pub fn take_shortcut(&self) -> bool {
        self.data
            .shortcut
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
    pub fn set_header_regions(&self, regions: &[egui::Rect], scale: f64) {
        *self
            .data
            .header_regions
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = regions
            .iter()
            .map(|r| egui::Rect::from_min_max(r.min * scale as f32, r.max * scale as f32))
            .collect();
    }
    pub fn set_body_region(&self, rect: Option<egui::Rect>, scale: f64) {
        *self
            .data
            .body_region
            .lock()
            .unwrap_or_else(|p| p.into_inner()) =
            rect.map(|r| egui::Rect::from_min_max(r.min * scale as f32, r.max * scale as f32));
    }
    pub fn take_expansion(&self) -> bool {
        self.data
            .expand
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: u32,
    pub key: u32,
}
pub fn parse_shortcut(text: &str) -> Result<Option<Shortcut>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<_> = text
        .split('+')
        .map(str::trim)
        .map(str::to_ascii_uppercase)
        .collect();
    if parts.len() < 2 {
        return Err(
            "단축키는 Ctrl/Alt/Shift/Win과 키를 조합하세요. 비우면 사용하지 않습니다.".into(),
        );
    }
    let mut modifiers = 0;
    for part in &parts[..parts.len() - 1] {
        let flag = match part.as_str() {
            "ALT" => 1,
            "CTRL" | "CONTROL" => 2,
            "SHIFT" => 4,
            "WIN" => 8,
            _ => return Err("지원하지 않는 단축키 조합입니다.".into()),
        };
        if modifiers & flag != 0 {
            return Err("단축키 수정자가 중복되었습니다.".into());
        }
        modifiers |= flag;
    }
    if modifiers & (1 | 2 | 8) == 0 {
        return Err("전역 단축키에 Ctrl, Alt 또는 Win이 필요합니다.".into());
    }
    let key = parts.last().unwrap();
    let key = if key == "SPACE" {
        0x20
    } else if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() {
        key.as_bytes()[0] as u32
    } else if let Some(number) = key
        .strip_prefix('F')
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| (1..=11).contains(n))
    {
        0x70 + number - 1
    } else {
        return Err("키는 Space, A–Z, 0–9, F1–F11을 사용할 수 있습니다.".into());
    };
    Ok(Some(Shortcut { modifiers, key }))
}

pub fn autostart(helper: &std::path::Path, enabled: bool) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| e.to_string())?;
    let owned = format!("\"{}\" --autostart", helper.display());
    let existing: Option<String> = key.get_value("MiniTerm").ok();
    if existing.as_ref().is_some_and(|value| value != &owned) {
        return Err("다른 MiniTerm 시작 항목이 있습니다. 기존 항목을 보존했습니다.".into());
    }
    if enabled {
        key.set_value("MiniTerm", &owned).map_err(|e| e.to_string())
    } else if existing.is_some() {
        key.delete_value("MiniTerm").map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

impl Drop for DisplayWatch {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(self.hwnd as HWND, 1);
            windows_sys::Win32::UI::Shell::RemoveWindowSubclass(
                self.hwnd as HWND,
                Some(display_callback),
                1,
            );
        }
    }
}
unsafe extern "system" fn display_callback(
    hwnd: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    data: usize,
) -> isize {
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_NCHITTEST {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
        };
        if let Ok(bounds) = window_bounds(hwnd as isize) {
            let x = lparam as i16 as i32 - bounds.x;
            let y = (lparam >> 16) as i16 as i32 - bounds.y;
            if x >= 0 && y >= 0 && x < bounds.width as i32 && y < bounds.height as i32 {
                let edge = (4.0 * window_scale(hwnd as isize)).ceil() as i32;
                let left = x < edge;
                let right = x >= bounds.width as i32 - edge;
                let top = y < edge;
                let bottom = y >= bounds.height as i32 - edge;
                let hit = match (left, right, top, bottom) {
                    (true, _, true, _) => HTTOPLEFT,
                    (_, true, true, _) => HTTOPRIGHT,
                    (true, _, _, true) => HTBOTTOMLEFT,
                    (_, true, _, true) => HTBOTTOMRIGHT,
                    (true, _, _, _) => HTLEFT,
                    (_, true, _, _) => HTRIGHT,
                    (_, _, true, _) => HTTOP,
                    (_, _, _, true) => HTBOTTOM,
                    _ => 0,
                };
                if hit != 0 {
                    return hit as isize;
                }
            }
        }
    }
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_GETMINMAXINFO && lparam != 0 {
        let result = windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, message, wparam, lparam);
        let watch = &*(data as *const DisplayWatchData);
        let limits = &mut *(lparam as *mut windows_sys::Win32::UI::WindowsAndMessaging::MINMAXINFO);
        let scale = window_scale(hwnd as isize);
        limits.ptMinTrackSize.x = (280.0 * scale).round() as i32;
        limits.ptMinTrackSize.y = (96.0 * scale).round() as i32;
        limits.ptMaxTrackSize.x = (1000.0 * scale).round() as i32;
        if !watch.expanded.load(std::sync::atomic::Ordering::Acquire) {
            limits.ptMaxTrackSize.y = (320.0 * scale).round() as i32;
        }
        return result;
    }
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_ENTERSIZEMOVE {
        let watch = &*(data as *const DisplayWatchData);
        watch
            .moving
            .store(true, std::sync::atomic::Ordering::Release);
        *watch
            .last_header_press
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = None;
    }
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_EXITSIZEMOVE {
        let watch = &*(data as *const DisplayWatchData);
        watch
            .moving
            .store(false, std::sync::atomic::Ordering::Release);
        watch
            .moved
            .store(true, std::sync::atomic::Ordering::Release);
        watch.ctx.request_repaint();
    }
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_STYLECHANGING
        && wparam as i32 == windows_sys::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE
        && lparam != 0
    {
        // winit rewrites extended styles when focus/visibility/window flags change. Keep our
        // alpha style in that update, so opacity survives showing, focusing and expanding.
        let watch = &*(data as *const DisplayWatchData);
        let styles =
            &mut *(lparam as *mut windows_sys::Win32::UI::WindowsAndMessaging::STYLESTRUCT);
        if watch.layered.load(std::sync::atomic::Ordering::Acquire) {
            styles.styleNew |= windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED;
        } else {
            styles.styleNew &= !windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED;
        }
    }
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SETTINGCHANGE,
    };
    if matches!(message, WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DPICHANGED) {
        // The subclass is installed and removed on the window's thread; the boxed data stays pinned.
        let watch = &*(data as *const DisplayWatchData);
        watch
            .changed
            .store(true, std::sync::atomic::Ordering::Release);
        watch.ctx.request_repaint();
    }
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_HOTKEY && wparam == 1 {
        let watch = &*(data as *const DisplayWatchData);
        show(hwnd as isize, true);
        watch
            .shortcut
            .store(true, std::sync::atomic::Ordering::Release);
        watch.ctx.request_repaint();
        return 0;
    }
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    };
    if message == WM_LBUTTONUP {
        let watch = &*(data as *const DisplayWatchData);
        if watch
            .suppress_release
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            return 0;
        }
    }
    if matches!(message, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK) {
        let watch = &*(data as *const DisplayWatchData);
        let point = egui::pos2(lparam as i16 as f32, (lparam >> 16) as i16 as f32);
        let header = watch
            .header_regions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .any(|rect| rect.contains(point));
        let body = watch
            .body_region
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some_and(|rect| wparam & 4 == 0 && rect.contains(point));
        let mut last = watch
            .last_header_press
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if header || body {
            let now = std::time::Instant::now();
            let double = last.is_some_and(|(time, previous)| {
                now.duration_since(time).as_millis() <= GetDoubleClickTime() as u128
                    && previous.distance(point) <= 4.0
            });
            if double {
                watch
                    .expand
                    .fetch_xor(true, std::sync::atomic::Ordering::AcqRel);
                *last = None;
                if body {
                    watch
                        .suppress_release
                        .store(true, std::sync::atomic::Ordering::Release);
                    watch.ctx.request_repaint();
                    return 0;
                }
            } else {
                *last = Some((now, point));
            }
            if header {
                show(hwnd as isize, true);
                watch
                    .shortcut
                    .store(true, std::sync::atomic::Ordering::Release);
            }
            watch.ctx.request_repaint();
        } else {
            *last = None;
        }
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, message, wparam, lparam)
}

/// A job owns the shell's child tree. Its handle is never exposed or duplicated outside this type.
pub struct ProcessJob(HANDLE);
// Windows kernel handles may be used from any thread; ownership remains with this wrapper.
unsafe impl Send for ProcessJob {}
unsafe impl Sync for ProcessJob {}

impl ProcessJob {
    pub fn assign(process: isize) -> Result<Self, String> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
                || AssignProcessToJobObject(handle, process as HANDLE) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(job)
        }
    }
    pub fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    #[test]
    fn validates_shortcuts_without_hijacking_unmodified_keys() {
        assert_eq!(
            parse_shortcut("Ctrl+Alt+Space").unwrap(),
            Some(Shortcut {
                modifiers: 3,
                key: 0x20
            })
        );
        assert_eq!(parse_shortcut("Ctrl+Shift+F10").unwrap().unwrap().key, 0x79);
        assert_eq!(parse_shortcut("").unwrap(), None);
        for shortcut in ["C", "Shift+Space", "Ctrl+Ctrl+A", "Ctrl+F12", "Ctrl+Alt+"] {
            assert!(parse_shortcut(shortcut).is_err());
        }
    }
}
