//! Connects the exe, `uapmd_host.dll` and `evolve.dll` to each other by
//! registering each side's callbacks with the others.
use libloading::Library;

/// Loads `evolve.dll` and registers the callbacks among the three. The returned
/// library must outlive every call into `host`; [`disconnect`] undoes the exe's side.
///
/// # Safety
/// `host` is a loaded `uapmd_host.dll`, called on the control thread only.
pub unsafe fn connect(host: &Library) -> Result<Library, String> {
    type Callback = unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char;
    let set_api = host
        .get::<unsafe extern "C" fn(Callback)>(b"repl_set_script_api\0")
        .map_err(|e| e.to_string())?;
    set_api(crate::script_api::call);
    let evolve_path = crate::resources::extension("evolve")?;
    let evolve = Library::new(&evolve_path)
        .map_err(|error| format!("{}: {error}", evolve_path.display()))?;
    let sequence = evolve
        .get::<unsafe extern "C" fn() -> *const repl_extension_api::sequence::Api>(
            b"repl_sequence_api\0",
        )
        .map_err(|e| e.to_string())?;
    let connect_sequence = host
        .get::<unsafe extern "C" fn(*const repl_extension_api::sequence::Api) -> bool>(
            b"repl_set_sequence_api\0",
        )
        .map_err(|e| e.to_string())?;
    if !connect_sequence(sequence()) {
        return Err("sequence API ABI mismatch".into());
    }
    let evolve_api = evolve
        .get::<unsafe extern "C" fn() -> *const repl_extension_api::evolve::Api>(
            b"repl_evolve_api\0",
        )
        .map_err(|e| e.to_string())?;
    let connect_evolve = host
        .get::<unsafe extern "C" fn(*const repl_extension_api::evolve::Api) -> bool>(
            b"repl_set_evolve_api\0",
        )
        .map_err(|e| e.to_string())?;
    if !connect_evolve(evolve_api()) {
        return Err("evolve DLL ABI mismatch".into());
    }
    type PadGain = unsafe extern "C" fn(
        i32,
        *const *mut std::ffi::c_void,
        usize,
        usize,
        f64,
        bool,
        *const u32,
        usize,
    );
    let register_gain = host
        .get::<unsafe extern "C" fn(PadGain)>(b"repl_set_pad_gain_callback\0")
        .map_err(|e| e.to_string())?;
    let process_gain = *evolve
        .get(b"repl_pad_gain_process\0")
        .map_err(|e| e.to_string())?;
    let set_gain = *evolve
        .get(b"repl_pad_gain_set\0")
        .map_err(|e| e.to_string())?;
    let read_gain = *evolve
        .get(b"repl_pad_gain_read\0")
        .map_err(|e| e.to_string())?;
    use crate::patch_state::clap;
    clap::connect(Some(clap::HostCalls {
        plugin: *host
            .get::<clap::ClapPlugin>(b"repl_clap_plugin\0")
            .map_err(|e| e.to_string())?,
        sysex: *host
            .get::<clap::ClapSysEx>(b"repl_clap_sysex\0")
            .map_err(|e| e.to_string())?,
        fresh: *host
            .get::<clap::FreshPlugin>(b"repl_fresh_plugin\0")
            .map_err(|e| e.to_string())?,
    }));
    type Into = unsafe extern "C" fn(
        *const std::ffi::c_void,
        *const u8,
        usize,
        bool,
        crate::patch_state::StateOut,
        *mut std::ffi::c_void,
        *mut std::ffi::c_char,
        usize,
    ) -> bool;
    host.get::<unsafe extern "C" fn(Into)>(b"repl_set_patch_into\0")
        .map_err(|e| e.to_string())?(crate::patch_state::patch_into);
    crate::script_api::set_sequence_choose(
        *evolve
            .get::<repl_extension_api::sequence::Choose>(b"repl_sequence_choose\0")
            .map_err(|e| e.to_string())?,
    );
    register_gain(process_gain);
    crate::script_api::connect_pad_gain(Some(crate::script_api::PadGain {
        set: set_gain,
        read: read_gain,
    }));
    Ok(evolve)
}

/// Forgets the exe's pointers into both DLLs, after the host session is closed.
pub fn disconnect() {
    crate::script_api::connect_pad_gain(None);
    crate::script_api::clear_sequence_choose();
    crate::patch_state::clap::connect(None);
}
