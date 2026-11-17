use libloading::Library;
use serde_json::{json, Value};
use std::{
    ffi::{CStr, CString},
    path::Path,
};

pub struct Bridge {
    _library: Library,
    _sequence_library: Library,
    _control_thread: std::marker::PhantomData<std::rc::Rc<()>>,
    request: unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char,
    capture: Option<Box<crate::scope_capture::Capture>>,
    scope: Option<crate::scope::Scope>,
}

impl Bridge {
    pub fn load(path: &Path) -> Result<Self, String> {
        // The library stays loaded until all calls and session cleanup finish.
        unsafe {
            let library = Library::new(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let request = *library.get(b"repl_request\0").map_err(|e| e.to_string())?;
            type Callback =
                unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char;
            let set_api = library
                .get::<unsafe extern "C" fn(Callback)>(b"repl_set_script_api\0")
                .map_err(|e| e.to_string())?;
            set_api(crate::script_api::call);
            let register_notes = library
                .get::<unsafe extern "C" fn(
                    unsafe extern "C" fn(i32) -> *mut std::ffi::c_void,
                    unsafe extern "C" fn(*mut std::ffi::c_void),
                    unsafe extern "C" fn(*mut std::ffi::c_void, *const u32, usize),
                    unsafe extern "C" fn(*const i32, *const u32, usize) -> u64,
                )>(b"repl_set_note_counter\0")
                .map_err(|e| e.to_string())?;
            let sequence_path = crate::resources::extension("sequence")?;
            let sequence_library = Library::new(&sequence_path)
                .map_err(|error| format!("{}: {error}", sequence_path.display()))?;
            register_notes(
                *sequence_library
                    .get(b"repl_note_counter_create\0")
                    .map_err(|e| e.to_string())?,
                *sequence_library
                    .get(b"repl_note_counter_destroy\0")
                    .map_err(|e| e.to_string())?,
                *sequence_library
                    .get(b"repl_note_counter_process\0")
                    .map_err(|e| e.to_string())?,
                *sequence_library
                    .get(b"repl_note_counter_read\0")
                    .map_err(|e| e.to_string())?,
            );
            let api = sequence_library
                .get::<unsafe extern "C" fn() -> *const repl_extension_api::sequence::Api>(
                    b"repl_sequence_api\0",
                )
                .map_err(|e| e.to_string())?;
            let connect = library
                .get::<unsafe extern "C" fn(*const repl_extension_api::sequence::Api) -> bool>(
                    b"repl_set_sequence_api\0",
                )
                .map_err(|e| e.to_string())?;
            if !connect(api()) {
                return Err("Sequence DLL ABI mismatch".into());
            }
            let evolution = sequence_library
                .get::<unsafe extern "C" fn() -> *const repl_extension_api::evolution::Api>(
                    b"repl_evolution_api\0",
                )
                .map_err(|e| e.to_string())?;
            let connect_evolution = library
                .get::<unsafe extern "C" fn(*const repl_extension_api::evolution::Api) -> bool>(
                    b"repl_set_evolution_api\0",
                )
                .map_err(|e| e.to_string())?;
            if !connect_evolution(evolution()) {
                return Err("Evolution DLL ABI mismatch".into());
            }
            crate::script_api::set_sequence_choose(
                *sequence_library
                    .get::<repl_extension_api::sequence::Choose>(b"repl_sequence_choose\0")
                    .map_err(|e| e.to_string())?,
            );
            Ok(Self {
                _library: library,
                _sequence_library: sequence_library,
                _control_thread: std::marker::PhantomData,
                request,
                capture: None,
                scope: None,
            })
        }
    }
    pub fn enable_scope(&mut self, sample_rate: u32) -> Result<(), String> {
        let (capture, scope) = crate::scope::Scope::start(sample_rate)?;
        self.capture = Some(Box::new(capture));
        self.scope = Some(scope);
        // Registered before instrument creation. Close stops and joins native
        // processing before the boxed producer or library can be dropped.
        unsafe {
            let register = self
                ._library
                .get::<unsafe extern "C" fn(
                    crate::scope_capture::Callback,
                    *mut crate::scope_capture::Capture,
                )>(b"repl_set_scope\0")
                .map_err(|e| e.to_string())?;
            register(
                crate::scope_capture::record_native,
                self.capture.as_deref_mut().unwrap(),
            );
        }
        Ok(())
    }
    pub fn scope(&self) -> Option<&crate::scope::Scope> {
        self.scope.as_ref()
    }
    pub fn call(&self, args: Value) -> Result<Value, String> {
        let text = CString::new(args.to_string()).map_err(|e| e.to_string())?;
        let result = unsafe {
            let ptr = (self.request)(text.as_ptr());
            if ptr.is_null() {
                return Err(crate::i18n::t("m066").into());
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        };
        let result: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
        if result["ok"] == true {
            Ok(result)
        } else {
            Err(result["error"]
                .as_str()
                .unwrap_or(crate::i18n::t("m067"))
                .into())
        }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.call(json!({"command":"close"}));
        crate::script_api::clear_sequence_choose();
    }
}
