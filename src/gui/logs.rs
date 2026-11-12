use eframe::egui;
use std::{os::windows::ffi::OsStrExt, path::Path};

pub(super) fn show(ui: &mut egui::Ui, path: &Path) -> Result<(), ()> {
    let directory = path.parent().filter(|dir| !dir.as_os_str().is_empty());
    let mut result = Ok(());
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::i18n::t("m008"));
        if ui
            .add_enabled(
                directory.is_some(),
                egui::Button::new(crate::i18n::t("open_log_directory")),
            )
            .clicked()
        {
            if let Some(directory) = directory {
                result = open_directory(directory);
            }
        }
    });
    if let Some(filename) = path.file_name() {
        ui.label(filename.to_string_lossy());
        let native = Path::new(filename).with_extension("native");
        ui.label(format!("{}.stdout.log", native.display()));
        ui.label(format!("{}.stderr.log", native.display()));
    }
    result
}

fn open_directory(directory: &Path) -> Result<(), ()> {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            window: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    // ShellExecute opens the directory with the OS-associated file manager.
    // The UTF-16 path is NUL-terminated and remains alive throughout the call.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result > 32 {
        Ok(())
    } else {
        Err(())
    }
}
