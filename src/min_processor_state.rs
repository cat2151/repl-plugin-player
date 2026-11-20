//! The power setting "minimum processor state" (AC and DC) of the active
//! scheme. What to write back, and when, belongs to the script.
use windows_sys::{
    core::GUID,
    Win32::{
        Foundation::LocalFree,
        System::{
            Power::{
                PowerGetActiveScheme, PowerSetActiveScheme, PowerWriteACValueIndex,
                PowerWriteDCValueIndex,
            },
            SystemServices::{GUID_PROCESSOR_SETTINGS_SUBGROUP, GUID_PROCESSOR_THROTTLE_MINIMUM},
        },
    },
};

/// `percent` must be at most 100.
pub fn set(percent: u32) -> Result<(), String> {
    let check = |call: &str, code: u32| {
        if code == 0 {
            Ok(())
        } else {
            Err(crate::i18n::render(
                "min_processor_state_error",
                &[("call", call.into()), ("code", code.to_string())],
            ))
        }
    };
    let (subgroup, setting) = (
        &GUID_PROCESSOR_SETTINGS_SUBGROUP,
        &GUID_PROCESSOR_THROTTLE_MINIMUM,
    );
    let root = std::ptr::null_mut();
    let mut scheme: *mut GUID = std::ptr::null_mut();
    unsafe {
        check(
            "PowerGetActiveScheme",
            PowerGetActiveScheme(root, &mut scheme),
        )?;
        let result = check(
            "PowerWriteACValueIndex",
            PowerWriteACValueIndex(root, scheme, subgroup, setting, percent),
        )
        .and_then(|()| {
            check(
                "PowerWriteDCValueIndex",
                PowerWriteDCValueIndex(root, scheme, subgroup, setting, percent),
            )
        })
        .and_then(|()| check("PowerSetActiveScheme", PowerSetActiveScheme(root, scheme)));
        LocalFree(scheme.cast());
        result
    }
}
