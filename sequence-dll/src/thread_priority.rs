//! Evolve-scoped rendering priority, restored on the same host thread.
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static BOOSTED: AtomicU32 = AtomicU32::new(0);
pub fn set_active(value: bool) {
    ACTIVE.store(value, Ordering::Release);
}
pub fn boosted() -> u32 {
    BOOSTED.load(Ordering::Acquire)
}
pub fn update() {
    apply(ACTIVE.load(Ordering::Acquire));
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{cell::RefCell, ffi::c_void};
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn GetThreadPriority(thread: *mut c_void) -> i32;
        fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
    }
    struct Saved(Option<i32>);
    impl Drop for Saved {
        fn drop(&mut self) {
            if self.0.is_some() {
                BOOSTED.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
    thread_local! { static SAVED: RefCell<Saved> = const { RefCell::new(Saved(None)) }; }
    fn eligible(original: i32) -> bool {
        // MMCSS returns values such as 17 (base priority 25). Do not lower
        // those threads or later feed that value to SetThreadPriority.
        original == -15 || (-2..=2).contains(&original)
    }
    pub(super) fn apply(active: bool) {
        SAVED.with(|saved| {
            let mut saved = saved.borrow_mut();
            match (active, saved.0) {
                (true, None) => unsafe {
                    let thread = GetCurrentThread();
                    let original = GetThreadPriority(thread);
                    if eligible(original) && SetThreadPriority(thread, 15) != 0 {
                        saved.0 = Some(original);
                        BOOSTED.fetch_add(1, Ordering::AcqRel);
                    }
                },
                (false, Some(original)) => unsafe {
                    if SetThreadPriority(GetCurrentThread(), original) != 0 {
                        saved.0 = None;
                        BOOSTED.fetch_sub(1, Ordering::AcqRel);
                    }
                },
                _ => {}
            }
        });
    }
    #[cfg(test)]
    #[test]
    fn host_thread_priority_is_restored_after_evolve() {
        let original = unsafe { GetThreadPriority(GetCurrentThread()) };
        apply(true);
        assert_eq!(
            unsafe { GetThreadPriority(GetCurrentThread()) },
            if eligible(original) { 15 } else { original }
        );
        apply(false);
        assert_eq!(unsafe { GetThreadPriority(GetCurrentThread()) }, original);
    }
    #[cfg(test)]
    #[test]
    fn mmcss_priority_is_preserved_instead_of_lowered() {
        assert!(!eligible(17));
        assert!(!eligible(15));
        assert!(!eligible(i32::MAX));
        assert!(eligible(2));
    }
}
#[cfg(windows)]
use platform::apply;
#[cfg(not(windows))]
fn apply(_: bool) {}
