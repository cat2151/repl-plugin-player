//! `uapmd_host.dll`: the C++ DLL that hosts UAPMD (audio device, plugins,
//! timeline, JS engine) and takes JSON requests on the control thread.
use libloading::Library;
use serde_json::{json, Value};
use std::{
    ffi::{CStr, CString},
    path::Path,
};

pub struct UapmdHost {
    library: Library,
    _evolve: Library,
    _control_thread: std::marker::PhantomData<std::rc::Rc<()>>,
    request: unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char,
    capture: Option<Box<crate::scope_capture::Capture>>,
    scope: Option<crate::scope::Scope>,
}

impl UapmdHost {
    pub fn load(path: &Path) -> Result<Self, String> {
        // The library stays loaded until all calls and session cleanup finish.
        unsafe {
            let library = Library::new(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let request = *library.get(b"repl_request\0").map_err(|e| e.to_string())?;
            let evolve = crate::dll_wiring::connect(&library)?;
            Ok(Self {
                library,
                _evolve: evolve,
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
        // Registered before instrument creation. Close stops and joins the host's
        // processing before the boxed producer or library can be dropped.
        unsafe {
            let register = self
                .library
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
impl Drop for UapmdHost {
    fn drop(&mut self) {
        crate::recording::close();
        let _ = self.call(json!({"command":"close"}));
        crate::dll_wiring::disconnect();
    }
}
