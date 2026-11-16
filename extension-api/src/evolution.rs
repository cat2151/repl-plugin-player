//! Dedicated A/B renderer ABI. Control owns a slot only while it is loading.
use super::sequence::{Emit, Plan};
use std::ffi::c_void;

#[repr(C)]
pub struct Api {
    pub version: u32,
    pub create: unsafe extern "C" fn(f64, f64) -> *mut c_void,
    pub destroy: unsafe extern "C" fn(*mut c_void),
    pub status: unsafe extern "C" fn(*mut c_void, u32) -> u32,
    pub gain: unsafe extern "C" fn(*mut c_void, u32) -> f32,
    pub claim: unsafe extern "C" fn(*mut c_void, u32) -> bool,
    pub ready: unsafe extern "C" fn(*mut c_void, u32, bool),
    pub publish: unsafe extern "C" fn(*mut c_void, *const Plan) -> u32,
    pub pending: unsafe extern "C" fn(*mut c_void) -> bool,
    pub begin: unsafe extern "C" fn(*mut c_void, u32, f64),
    pub render: unsafe extern "C" fn(*mut c_void, u32, u32, f64, Emit, *mut c_void) -> bool,
    pub mix: unsafe extern "C" fn(*mut c_void, u32, *mut f32, u32),
    pub mix64: unsafe extern "C" fn(*mut c_void, u32, *mut f64, u32),
    pub finish: unsafe extern "C" fn(*mut c_void, u32),
    pub pulse: unsafe extern "C" fn(*mut c_void),
    pub spinning: unsafe extern "C" fn(*mut c_void) -> bool,
    pub update_priority: unsafe extern "C" fn(),
    pub boosted_threads: unsafe extern "C" fn() -> u32,
}
