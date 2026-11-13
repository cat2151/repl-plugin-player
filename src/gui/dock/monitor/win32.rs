//! Monitors and window moves in physical pixels, which differing monitor DPI does not
//! change.
use super::Screen;
use eframe::egui;

#[cfg(windows)]
mod windows {
    use super::Screen;
    use std::{ffi::c_void, ptr::null_mut};

    type Handle = *mut c_void;
    /// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
    const PER_MONITOR_AWARE_V2: isize = -4;
    /// `SWP_NOZORDER | SWP_NOACTIVATE`.
    const KEEP_ORDER: u32 = 0x0014;

    #[derive(Clone, Copy, Default)]
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct Info {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
        device: [u16; 32],
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            dc: Handle,
            clip: *const Rect,
            callback: unsafe extern "system" fn(Handle, Handle, *mut Rect, isize) -> i32,
            data: isize,
        ) -> i32;
        fn GetMonitorInfoW(monitor: Handle, info: *mut Info) -> i32;
        fn SetThreadDpiAwarenessContext(context: Handle) -> Handle;
        fn FindWindowExW(
            parent: Handle,
            after: Handle,
            class: *const u16,
            title: *const u16,
        ) -> Handle;
        fn GetWindowThreadProcessId(window: Handle, process: *mut u32) -> u32;
        fn SetWindowPos(
            window: Handle,
            after: Handle,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
    }

    /// This thread sees physical pixels until dropped.
    struct Physical(Handle);

    impl Physical {
        fn new() -> Self {
            Self(unsafe { SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2 as Handle) })
        }
    }

    impl Drop for Physical {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { SetThreadDpiAwarenessContext(self.0) };
            }
        }
    }

    struct Survey {
        screens: Vec<Screen>,
        failed: bool,
    }

    unsafe extern "system" fn collect(
        monitor: Handle,
        _: Handle,
        _: *mut Rect,
        data: isize,
    ) -> i32 {
        let survey = &mut *(data as *mut Survey);
        let mut info = Info {
            size: std::mem::size_of::<Info>() as u32,
            monitor: Rect::default(),
            work: Rect::default(),
            flags: 0,
            device: [0; 32],
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            survey.failed = true;
            return 0;
        }
        let end = info.device.iter().position(|c| *c == 0).unwrap_or(32);
        let name = String::from_utf16_lossy(&info.device[..end]);
        let Some(number) = (name.strip_prefix(r"\\.\DISPLAY")).and_then(|n| n.parse().ok()) else {
            survey.failed = true;
            return 0;
        };
        let geometry = |r: Rect| [r.left, r.top, r.right - r.left, r.bottom - r.top];
        survey.screens.push(Screen {
            number,
            bounds: geometry(info.monitor),
            work: geometry(info.work),
        });
        1
    }

    pub fn screens() -> Option<Vec<Screen>> {
        let _physical = Physical::new();
        let mut survey = Survey {
            screens: vec![],
            failed: false,
        };
        let data = &mut survey as *mut Survey as isize;
        let ok = unsafe { EnumDisplayMonitors(null_mut(), std::ptr::null(), collect, data) };
        (ok != 0 && !survey.failed && !survey.screens.is_empty()).then_some(survey.screens)
    }

    /// This process's top-level window titled `title`; another application's window of
    /// the same title is skipped.
    pub fn window(title: &str) -> Option<isize> {
        let title: Vec<_> = title.encode_utf16().chain([0]).collect();
        let mut window = null_mut();
        loop {
            window = unsafe { FindWindowExW(null_mut(), window, std::ptr::null(), title.as_ptr()) };
            if window.is_null() {
                return None;
            }
            let mut process = 0;
            unsafe { GetWindowThreadProcessId(window, &mut process) };
            if process == std::process::id() {
                return Some(window as isize);
            }
        }
    }

    pub fn place(window: isize, [x, y, w, h]: [i32; 4]) -> bool {
        let _physical = Physical::new();
        unsafe { SetWindowPos(window as Handle, null_mut(), x, y, w, h, KEEP_ORDER) != 0 }
    }
}

pub(super) fn screens() -> Result<Vec<Screen>, String> {
    #[cfg(windows)]
    let screens = windows::screens();
    #[cfg(not(windows))]
    let screens = None;
    screens.ok_or_else(|| crate::i18n::t("layout_monitor_read").into())
}

/// Frames that place the window: the builder's size and `WM_DPICHANGED` can land after
/// the first one.
const PASSES: u8 = 3;

pub(super) fn apply(ctx: &egui::Context, title: &str, outer: [i32; 4]) {
    #[cfg(windows)]
    {
        let key = ctx.viewport_id().0.with("dock-monitor-placement");
        let Some(window) = windows::window(title) else {
            ctx.request_repaint();
            return;
        };
        let stamp = (window, outer);
        let left = match ctx.data(|d| d.get_temp::<((isize, [i32; 4]), u8)>(key)) {
            Some((old, left)) if old == stamp => left,
            _ => PASSES,
        };
        if left == 0 {
            return;
        }
        if windows::place(window, outer) {
            ctx.data_mut(|d| d.insert_temp(key, (stamp, left - 1)));
        }
        ctx.request_repaint();
    }
    #[cfg(not(windows))]
    let _ = (ctx, title, outer, PASSES);
}
