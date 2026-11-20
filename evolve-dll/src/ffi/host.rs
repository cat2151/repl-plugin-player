//! Wraps the C [`HostApi`] as the [`Host`] trait. Each `done` closure is boxed
//! and passed as the callback's `user` pointer; the trampoline takes it back.
use std::ffi::{c_char, c_void, CStr, CString};

use repl_extension_api::evolve::HostApi;

use crate::host::{CreateDone, Host, InstanceHandle, LoadDone, PluginKind};

pub struct FfiHost {
    api: HostApi,
}

// The host promises its functions may be called from any thread.
unsafe impl Send for FfiHost {}
unsafe impl Sync for FfiHost {}

impl FfiHost {
    /// `None` when `api` is null or smaller than this build's [`HostApi`].
    ///
    /// # Safety
    /// A non-null `api` points to a readable `HostApi` prefix of `size` bytes.
    pub unsafe fn new(api: *const HostApi) -> Option<Self> {
        let api = unsafe { api.as_ref()? };
        (api.size >= size_of::<HostApi>()).then_some(Self { api: *api })
    }
}

pub(crate) unsafe fn text(ptr: *const c_char) -> Option<String> {
    (!ptr.is_null()).then(|| {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    })
}

unsafe extern "C" fn created(user: *mut c_void, handle: u64, error: *const c_char) {
    let done = unsafe { Box::from_raw(user.cast::<CreateDone>()) };
    done(match unsafe { text(error) } {
        Some(error) => Err(error),
        None if handle == 0 => Err("the host returned no instance".to_string()),
        None => Ok(handle),
    });
}

unsafe extern "C" fn loaded(user: *mut c_void, ok: bool, error: *const c_char) {
    let done = unsafe { Box::from_raw(user.cast::<LoadDone>()) };
    done(if ok {
        Ok(())
    } else {
        Err(unsafe { text(error) }.unwrap_or_else(|| "the host could not load the state".into()))
    });
}

impl Host for FfiHost {
    fn create_instance(&self, kind: &PluginKind, done: CreateDone) {
        let (Ok(format), Ok(plugin_id)) = (
            CString::new(kind.format.as_str()),
            CString::new(kind.plugin_id.as_str()),
        ) else {
            done(Err("the plugin name contains NUL".to_string()));
            return;
        };
        let user = Box::into_raw(Box::new(done)).cast();
        unsafe {
            (self.api.create_instance)(
                self.api.ctx,
                format.as_ptr(),
                plugin_id.as_ptr(),
                created,
                user,
            )
        };
    }

    fn destroy_instance(&self, handle: InstanceHandle) {
        unsafe { (self.api.destroy_instance)(self.api.ctx, handle) };
    }

    fn load_state(&self, handle: InstanceHandle, data: &[u8], done: LoadDone) {
        let user = Box::into_raw(Box::new(done)).cast();
        unsafe {
            (self.api.load_state)(
                self.api.ctx,
                handle,
                data.as_ptr(),
                data.len(),
                loaded,
                user,
            )
        };
    }

    fn attach_to_slot(&self, slot: u8, handle: InstanceHandle) -> bool {
        unsafe { (self.api.attach_to_slot)(self.api.ctx, u32::from(slot), handle) }
    }

    fn detach_from_slot(&self, slot: u8) -> Option<InstanceHandle> {
        let handle = unsafe { (self.api.detach_from_slot)(self.api.ctx, u32::from(slot)) };
        (handle != 0).then_some(handle)
    }
}
