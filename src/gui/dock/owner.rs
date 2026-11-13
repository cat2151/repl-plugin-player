//! Makes the main window the Win32 owner of each pane window, so a pane window stays in
//! front of the main window when the main window is clicked. egui has no owner setting
//! for a viewport, so the windows are found by winit's window class and their titles.
#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;
    const GW_OWNER: u32 = 4;
    const GWLP_HWNDPARENT: i32 = -8;
    /// The class winit registers for every window it creates.
    const WINIT_CLASS: &str = "Window Class";

    #[link(name = "user32")]
    extern "system" {
        fn EnumThreadWindows(
            thread: u32,
            callback: unsafe extern "system" fn(Hwnd, isize) -> i32,
            param: isize,
        ) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        fn GetWindow(hwnd: Hwnd, cmd: u32) -> Hwnd;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    fn text(hwnd: Hwnd, get: unsafe extern "system" fn(Hwnd, *mut u16, i32) -> i32) -> String {
        let mut buffer = [0u16; 256];
        let len = unsafe { get(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        String::from_utf16_lossy(&buffer[..len.max(0) as usize])
    }

    unsafe extern "system" fn collect(hwnd: Hwnd, param: isize) -> i32 {
        let found = &mut *(param as *mut Vec<(Hwnd, String)>);
        if text(hwnd, GetClassNameW) == WINIT_CLASS {
            found.push((hwnd, text(hwnd, GetWindowTextW)));
        }
        1
    }

    /// Owns the windows titled `titles` by the window titled `main`; all are winit
    /// windows of the calling (UI) thread.
    pub fn own(main: &str, titles: &[String]) {
        let mut found: Vec<(Hwnd, String)> = vec![];
        unsafe {
            EnumThreadWindows(
                GetCurrentThreadId(),
                collect,
                &mut found as *mut Vec<(Hwnd, String)> as isize,
            )
        };
        let Some(&(owner, _)) = found.iter().find(|(_, title)| title == main) else {
            return;
        };
        for (hwnd, title) in &found {
            if *hwnd != owner
                && titles.contains(title)
                && unsafe { GetWindow(*hwnd, GW_OWNER) } != owner
            {
                unsafe { SetWindowLongPtrW(*hwnd, GWLP_HWNDPARENT, owner as isize) };
            }
        }
    }
}

#[cfg(windows)]
pub(super) use win32::own;

#[cfg(not(windows))]
pub(super) fn own(_: &str, _: &[String]) {}
