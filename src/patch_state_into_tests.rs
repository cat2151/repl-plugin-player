use super::tests::{catalog, effect, Dir};
use super::*;

fn dexed_cartridge(dir: &Dir) -> PathBuf {
    let file = dir.file("Set/Bank.syx");
    let mut cartridge = vec![0u8; cmrt_core::dx7::DX7_BULK_DUMP_LEN];
    cartridge[..6].copy_from_slice(&[0xF0, 0x43, 0x00, 0x09, 0x20, 0x00]);
    *cartridge.last_mut().unwrap() = 0xF7;
    std::fs::write(&file, cartridge).unwrap();
    file
}

fn asked(_: &[u8]) -> Result<Vec<u8>, String> {
    panic!("asked the player")
}

#[test]
fn floe_and_dexed_patches_go_to_the_instance_without_the_player() {
    let dir = Dir::new("direct");
    let floe = catalog(
        cmrt_server_config::FLOE_PLUGIN_ID,
        dir.file("p.floe-preset"),
    );
    let dexed = catalog(
        cmrt_server_config::DEXED_PLUGIN_ID,
        dexed_cartridge(&dir).join("01 Voice"),
    );
    for reference in [&floe, &dexed] {
        assert_eq!(into::check(reference), Ok(()));
        let prepared = into::prepare(reference.clone(), asked).unwrap();
        assert!(prepared.state.is_none());
        // No uapmd_host.dll on the test thread: the instance is not found, and nothing else is tried.
        let missing = || Err::<clap::Plugin, _>("instance 3".to_string());
        let loaded = into::into(missing, prepared, true);
        assert_eq!(loaded, Err("instance 3".into()));
    }
    let missing = catalog(
        cmrt_server_config::FLOE_PLUGIN_ID,
        dir.0.join("none.floe-preset"),
    );
    assert!(into::check(&missing).is_err());
    let broken = catalog(
        cmrt_server_config::DEXED_PLUGIN_ID,
        dir.file("Set/Broken.syx").join("01 Voice"),
    );
    assert!(into::check(&broken).is_err());
    assert!(into::prepare(broken, asked).is_err());
}

/// A CLAP plugin whose `clap.state` saves `TEMPLATE`.
mod saving {
    use clap_sys::{
        ext::state::{clap_plugin_state, CLAP_EXT_STATE},
        plugin::{clap_plugin, clap_plugin_descriptor},
        stream::{clap_istream, clap_ostream},
    };
    use std::ffi::{c_char, c_void, CStr};

    pub const TEMPLATE: [u8; 35 * 8] = [0; 35 * 8];

    unsafe extern "C" fn save(_: *const clap_plugin, stream: *const clap_ostream) -> bool {
        let stream = unsafe { &*stream };
        let written = unsafe { stream.write.unwrap()(stream, TEMPLATE.as_ptr().cast(), 280) };
        written == 280
    }
    unsafe extern "C" fn load(_: *const clap_plugin, _: *const clap_istream) -> bool {
        false
    }
    static STATE: clap_plugin_state = clap_plugin_state {
        save: Some(save),
        load: Some(load),
    };
    unsafe extern "C" fn extension(_: *const clap_plugin, id: *const c_char) -> *const c_void {
        if unsafe { CStr::from_ptr(id) } == CLAP_EXT_STATE {
            (&STATE as *const clap_plugin_state).cast()
        } else {
            std::ptr::null()
        }
    }

    pub fn plugin() -> clap_plugin {
        clap_plugin {
            desc: std::ptr::null::<clap_plugin_descriptor>(),
            plugin_data: std::ptr::null_mut(),
            init: None,
            destroy: None,
            activate: None,
            deactivate: None,
            start_processing: None,
            stop_processing: None,
            reset: None,
            process: None,
            get_extension: Some(extension),
            on_main_thread: None,
        }
    }
}

#[test]
fn a_new_instance_gives_the_template_without_the_player() {
    let dir = Dir::new("into-template");
    let shu = effect("audio.mikey.Shu", dir.file("Shu.clap"), "Ether");
    assert_eq!(into::check(&shu), Ok(()));
    let prepared = into::prepare(shu.clone(), asked).unwrap();
    assert!(prepared.state.is_none(), "no template is kept yet");
    let plugin = saving::plugin();
    // SAFETY: the plugin above lives through the call, on this thread.
    let given =
        || {
            Ok(unsafe {
                clap::Plugin::given((&plugin as *const clap_sys::plugin::clap_plugin).cast())
            })
        };
    let made = into::into(given, prepared, true).unwrap().unwrap();
    assert_eq!(made.len(), saving::TEMPLATE.len());
    assert_ne!(made, saving::TEMPLATE);
    assert_eq!(built::kept("audio.mikey.Shu").unwrap(), saving::TEMPLATE);
    let again = into::prepare(shu, asked).unwrap();
    assert_eq!(again.state, Some(made));
    let missing = effect("audio.mikey.Shu", dir.file("Shu.clap"), "Missing");
    assert!(into::check(&missing).is_err());
}

#[test]
fn a_state_only_the_player_makes_is_asked_without_the_instance() {
    let dir = Dir::new("into-player");
    let vst3 = PatchReference {
        plugin: PluginKey {
            format: "VST3".into(),
            ..catalog("id", PathBuf::new()).plugin
        },
        ..catalog("id", dir.file("p.fxp"))
    };
    assert_eq!(into::check(&vst3), Ok(()));
    let prepared = into::prepare(vst3, |_| Ok(vec![1])).unwrap();
    assert_eq!(prepared.state, Some(vec![1]));
    let unused = || -> Result<clap::Plugin, String> { panic!("took the instance") };
    let made = into::into(unused, prepared, true);
    assert_eq!(made, Ok(Some(vec![1])));
}

#[test]
fn a_reused_instance_gets_the_template_from_a_new_instance_without_the_player() {
    unsafe extern "C" fn unused(_: i32) -> *const std::ffi::c_void {
        panic!("took a track instance")
    }
    unsafe extern "C" fn no_sysex(_: *const std::ffi::c_void, _: *const u8, _: usize) {}
    unsafe extern "C" fn refuse(
        format: *const std::ffi::c_char,
        plugin: *const std::ffi::c_char,
        _: clap::FreshUse,
        _: *mut std::ffi::c_void,
        error: *mut std::ffi::c_char,
        _: usize,
    ) -> bool {
        let text = |s| {
            unsafe { std::ffi::CStr::from_ptr(s) }
                .to_string_lossy()
                .into_owned()
        };
        let message = std::ffi::CString::new(format!("made no {} {}", text(format), text(plugin)));
        let message = message.unwrap();
        let bytes = message.as_bytes_with_nul();
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), error, bytes.len()) };
        false
    }
    clap::connect(Some(clap::HostCalls {
        plugin: unused,
        sysex: no_sysex,
        fresh: refuse,
    }));
    let dir = Dir::new("into-reused");
    let tone = effect(
        cmrt_core::tone3000_preset::TONE3000_PLUGIN_ID,
        dir.file("p.t3k"),
        "p",
    );
    let prepared = into::Prepared {
        reference: tone,
        state: None,
    };
    let unused = || -> Result<clap::Plugin, String> { panic!("took the reused instance") };
    let made = into::into(unused, prepared, false);
    clap::connect(None);
    let expected = format!(
        "made no CLAP {}",
        cmrt_core::tone3000_preset::TONE3000_PLUGIN_ID
    );
    assert_eq!(made, Err(expected));
    assert!(built::kept(cmrt_core::tone3000_preset::TONE3000_PLUGIN_ID).is_none());
}

#[test]
fn prepared_patches_carry_their_reference_and_state_to_the_host() {
    let dir = Dir::new("prepared");
    let dexed = catalog(
        cmrt_server_config::DEXED_PLUGIN_ID,
        dexed_cartridge(&dir).join("01 Voice"),
    );
    let prepared = into::prepare(dexed.clone(), asked).unwrap();
    assert!(prepared.state.is_none());
    let read = into::Prepared::from_bytes(&prepared.to_bytes().unwrap()).unwrap();
    assert!(read.reference == dexed && read.state.is_none());
    let vst3 = PatchReference {
        plugin: PluginKey {
            format: "VST3".into(),
            ..catalog("id", PathBuf::new()).plugin
        },
        ..catalog("id", dir.file("p.fxp"))
    };
    let ahead = into::prepare(vst3.clone(), |_| Ok(vec![7, 8])).unwrap();
    let read = into::Prepared::from_bytes(&ahead.to_bytes().unwrap()).unwrap();
    assert!(read.reference == vst3 && read.state == Some(vec![7, 8]));
    assert!(into::Prepared::from_bytes(b"a plugin state").is_err());
    assert!(into::Prepared::from_bytes(&[]).is_err());
}

#[test]
fn the_host_gets_the_state_to_load_or_the_error() {
    unsafe extern "C" fn keep(user: *mut std::ffi::c_void, state: *const u8, size: usize) {
        let kept = unsafe { &mut *user.cast::<Vec<u8>>() };
        kept.extend_from_slice(unsafe { std::slice::from_raw_parts(state, size) });
    }
    let call = |bytes: &[u8]| {
        let mut state: Vec<u8> = Vec::new();
        let mut error = [0 as std::ffi::c_char; 64];
        // SAFETY: every pointer outlives the call; no plugin is given.
        let ok = unsafe {
            patch_into(
                std::ptr::null(),
                bytes.as_ptr(),
                bytes.len(),
                true,
                keep,
                (&mut state as *mut Vec<u8>).cast(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        let error = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) };
        (ok, state, error.to_string_lossy().into_owned())
    };
    let dir = Dir::new("patch-into");
    let surge = into::Prepared {
        reference: catalog("org.surge-synth-team.surge-xt", dir.file("p.fxp")),
        state: Some(vec![1, 2, 3]),
    };
    assert_eq!(
        call(&surge.to_bytes().unwrap()),
        (true, vec![1, 2, 3], String::new())
    );
    let floe = into::Prepared {
        reference: catalog(
            cmrt_server_config::FLOE_PLUGIN_ID,
            dir.file("p.floe-preset"),
        ),
        state: None,
    };
    let (ok, state, error) = call(&floe.to_bytes().unwrap());
    assert!(!ok && state.is_empty() && error.contains("CLAP"), "{error}");
    let (ok, _, error) = call(b"a plugin state");
    assert!(!ok && !error.is_empty() && error.len() < 64);
}
