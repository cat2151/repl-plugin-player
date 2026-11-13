//! Surge XT in this process starts from a minimal data home, without scanning
//! its thousands of patches; cat-plugin-player children get the original one.
use std::sync::OnceLock;

#[allow(dead_code)]
#[path = "../../cat-plugin-player/shared/surge_data_home.rs"]
mod data_home;

/// Why the minimal data home was not used; Surge then starts normally.
static ERROR: OnceLock<String> = OnceLock::new();

/// Call first in `main`, before any thread or plugin library starts.
pub fn prepare() {
    let result = crate::resources::user_directory().and_then(|root| {
        // SAFETY: called first in `main`, before threads and native libraries.
        unsafe { data_home::before_threads(&root.join("cache")) }
    });
    if let Err(error) = result {
        let _ = ERROR.set(error);
    }
}

pub fn error() -> Option<&'static str> {
    ERROR.get().map(String::as_str)
}

pub fn restore_for_child(command: &mut std::process::Command) {
    data_home::restore_for_child(command);
}
