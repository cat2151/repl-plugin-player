use std::{fs::File, io::Write, os::windows::io::FromRawHandle};

// The native bridge redirects CRT stdout/stderr. Keep a separate OS handle
// before opening it so diagnostics cannot swallow or corrupt protocol replies.
pub fn stdout() -> Result<File, String> {
    unsafe extern "system" {
        fn GetStdHandle(kind: u32) -> *mut std::ffi::c_void;
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn DuplicateHandle(
            process: *mut std::ffi::c_void,
            source: *mut std::ffi::c_void,
            target: *mut std::ffi::c_void,
            duplicate: *mut *mut std::ffi::c_void,
            access: u32,
            inherit: i32,
            options: u32,
        ) -> i32;
    }
    let mut handle = std::ptr::null_mut();
    unsafe {
        let process = GetCurrentProcess();
        if DuplicateHandle(
            process,
            GetStdHandle(-11_i32 as u32),
            process,
            &mut handle,
            0,
            0,
            2,
        ) == 0
        {
            return Err(crate::i18n::render(
                "f024",
                &[("arg0", format!("{}", std::io::Error::last_os_error()))],
            ));
        }
        Ok(File::from_raw_handle(handle))
    }
}

pub fn emit(output: &mut File, value: serde_json::Value) -> Result<(), String> {
    writeln!(output, "{value}")
        .and_then(|_| output.flush())
        .map_err(|error| error.to_string())
}
