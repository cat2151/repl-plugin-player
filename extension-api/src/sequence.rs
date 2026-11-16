//! C ABI between the Rust sequence DLL and the UAPMD audio adapter.
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Step {
    pub notes: [u8; 8],
    pub count: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Plan {
    pub steps: [Step; 64],
    pub count: u32,
    pub track: i32,
    pub velocity: u16,
    pub seconds: f64,
    pub gate: f64,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            steps: [Step::default(); 64],
            count: 0,
            track: 0,
            velocity: 0,
            seconds: 0.0,
            gate: 0.0,
        }
    }
}

pub type Emit = unsafe extern "C" fn(*mut c_void, u32, u8, bool, u16);
pub type Choose = fn(&[Vec<Vec<u8>>], &[Vec<u8>], u8, Option<usize>) -> Option<Vec<usize>>;

#[repr(C)]
pub struct Api {
    pub version: u32,
    pub plan_size: usize,
    pub step_size: usize,
    pub create: unsafe extern "C" fn() -> *mut c_void,
    pub destroy: unsafe extern "C" fn(*mut c_void),
    /// 0: accepted, 1: invalid, 2: queue full.
    pub publish: unsafe extern "C" fn(*mut c_void, *const Plan) -> u32,
    pub pending: unsafe extern "C" fn(*mut c_void) -> bool,
    /// reset/rewind/held/release require exclusion of audio callbacks.
    pub reset: unsafe extern "C" fn(*mut c_void),
    pub rewind: unsafe extern "C" fn(*mut c_void),
    pub held: unsafe extern "C" fn(*mut c_void, *mut Step),
    pub render: unsafe extern "C" fn(*mut c_void, i32, u32, f64, bool, Emit, *mut c_void),
    pub release: unsafe extern "C" fn(*mut c_void),
}
