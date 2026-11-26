//! `uapmd_host.dll`'s generic entry points to a live CLAP instance.
use crate::i18n::render;
use std::{
    cell::Cell,
    ffi::{c_char, c_void, CStr, CString},
};

/// `repl_clap_plugin`: a track instance's `clap_plugin*`, or null.
pub type ClapPlugin = unsafe extern "C" fn(i32) -> *const c_void;
/// `repl_clap_sysex`: queues a MIDI SysEx message (copied) for the next
/// `process()` of a live `clap_plugin*`.
pub type ClapSysEx = unsafe extern "C" fn(*const c_void, *const u8, usize);
/// Given the `clap_plugin*` of a new instance, null when it is not CLAP.
pub type FreshUse = unsafe extern "C" fn(user: *mut c_void, plugin: *const c_void);
/// `repl_fresh_plugin(format, pluginId, use, user, error, capacity)`: calls
/// `use` with a new instance of the plugin that no track holds, then destroys
/// the instance; false fails with a NUL-terminated `error`.
pub type FreshPlugin = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    FreshUse,
    *mut c_void,
    *mut c_char,
    usize,
) -> bool;

#[derive(Clone, Copy)]
pub struct HostCalls {
    pub plugin: ClapPlugin,
    pub sysex: ClapSysEx,
    pub fresh: FreshPlugin,
}

thread_local! {
    /// Set on the control thread while `uapmd_host.dll` is loaded; plugins are called there only.
    static HOST: Cell<Option<HostCalls>> = const { Cell::new(None) };
}

pub fn connect(host: Option<HostCalls>) {
    HOST.set(host);
}

/// The state a new instance of the plugin saves, from an instance made and
/// destroyed for this call. Runs on the control (CLAP main) thread.
pub(super) fn fresh_state(format: &str, plugin_id: &str) -> Result<Vec<u8>, String> {
    type Saved = Option<Result<Vec<u8>, String>>;
    unsafe extern "C" fn save(user: *mut c_void, plugin: *const c_void) {
        // SAFETY: `user` is `saved` below, alive for the whole call.
        let saved = unsafe { &mut *user.cast::<Saved>() };
        *saved = Some(match plugin.is_null() {
            true => Err("The new instance is not a CLAP plugin".into()),
            false => Plugin(plugin).save(),
        });
    }
    let host = HOST
        .get()
        .ok_or("uapmd_host.dll is not loaded on this thread")?;
    let text = |s: &str| CString::new(s).map_err(|e| e.to_string());
    let (format, plugin_id) = (text(format)?, text(plugin_id)?);
    let mut saved: Saved = None;
    let mut error = [0 as c_char; 256];
    // SAFETY: every pointer outlives the call, made on the control thread.
    let made = unsafe {
        (host.fresh)(
            format.as_ptr(),
            plugin_id.as_ptr(),
            save,
            (&mut saved as *mut Saved).cast(),
            error.as_mut_ptr(),
            error.len(),
        )
    };
    if !made {
        // SAFETY: the host NUL-terminates `error` when it fails.
        return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .into_owned());
    }
    saved.unwrap_or_else(|| Err("The new instance was not given".into()))
}

/// A live CLAP plugin, used on the control (CLAP main) thread within the call
/// that gave it.
#[derive(Clone, Copy)]
pub(super) struct Plugin(*const c_void);

impl Plugin {
    /// The plugin of the track instance `instance`.
    pub(super) fn of(instance: i32) -> Result<Self, String> {
        HOST.get()
            .map(|host| unsafe { (host.plugin)(instance) })
            .filter(|plugin| !plugin.is_null())
            .map(Self)
            .ok_or_else(|| render("patch_state_04", &[("instance", instance.to_string())]))
    }

    /// # Safety
    /// `plugin` is a live `clap_plugin*`, used on the control thread.
    pub(super) unsafe fn given(plugin: *const c_void) -> Self {
        Self(plugin)
    }

    pub(super) fn raw(self) -> *const c_void {
        self.0
    }

    /// Queues `sysex` for the plugin's next `process()`; the plugin takes it on
    /// its audio thread, after this returns, once it processes.
    pub(super) fn send_sysex(self, sysex: &[u8]) -> Result<(), String> {
        let host = HOST
            .get()
            .ok_or("uapmd_host.dll is not loaded on this thread")?;
        unsafe { (host.sysex)(self.0, sysex.as_ptr(), sysex.len()) };
        Ok(())
    }

    /// The plugin's whole state, saved through its `clap.state` extension.
    pub(super) fn save(self) -> Result<Vec<u8>, String> {
        use clap_sys::{
            ext::state::{clap_plugin_state, CLAP_EXT_STATE},
            plugin::clap_plugin,
            stream::clap_ostream,
        };
        unsafe extern "C" fn write(
            stream: *const clap_ostream,
            buffer: *const c_void,
            size: u64,
        ) -> i64 {
            // SAFETY: `ctx` is the vector below, alive for the whole save; `buffer` holds `size` bytes.
            unsafe {
                let saved = &mut *(*stream).ctx.cast::<Vec<u8>>();
                saved.extend_from_slice(std::slice::from_raw_parts(
                    buffer.cast::<u8>(),
                    size as usize,
                ));
            }
            size as i64
        }
        let plugin = self.0.cast::<clap_plugin>();
        let missing = || "The plugin has no clap.state extension".to_string();
        // SAFETY: `Plugin` is a live instance, used on the control (CLAP main) thread.
        unsafe {
            let get_extension = (*plugin).get_extension.ok_or_else(missing)?;
            let state = get_extension(plugin, CLAP_EXT_STATE.as_ptr()).cast::<clap_plugin_state>();
            let save = state
                .as_ref()
                .and_then(|state| state.save)
                .ok_or_else(missing)?;
            let mut saved = Vec::new();
            let stream = clap_ostream {
                ctx: (&mut saved as *mut Vec<u8>).cast(),
                write: Some(write),
            };
            if !save(plugin, &stream) {
                return Err("The plugin failed to save its state".into());
            }
            Ok(saved)
        }
    }
}
