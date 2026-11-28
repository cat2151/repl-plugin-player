//! Synchronous bindings to existing Rust APIs. Composition belongs to JavaScript.
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    ffi::{c_char, CStr, CString},
    io::Read,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

type SetPadGain = unsafe extern "C" fn(*const i32, usize) -> bool;
type ReadPadGain = unsafe extern "C" fn(i32) -> f32;

#[derive(Clone, Copy)]
pub struct PadGain {
    pub set: SetPadGain,
    pub read: ReadPadGain,
}

thread_local! {
    static REPLY: RefCell<CString> = RefCell::new(CString::default());
    static SEQUENCE_CHOOSE: std::cell::Cell<Option<repl_extension_api::sequence::Choose>> = const { std::cell::Cell::new(None) };
    static PAD_GAIN: std::cell::Cell<Option<PadGain>> = const { std::cell::Cell::new(None) };
}

pub fn connect_pad_gain(gain: Option<PadGain>) {
    if let Some(previous) = PAD_GAIN.replace(gain) {
        unsafe { (previous.set)(std::ptr::null(), 0) };
    }
}

pub fn pad_gain_meter(instance: Option<i32>) -> (Option<f32>, Option<f32>) {
    let Some(gain) = PAD_GAIN.get() else {
        return (None, None);
    };
    let read = |id| {
        let value = unsafe { (gain.read)(id) };
        value.is_finite().then_some(value)
    };
    (instance.and_then(read), read(-2))
}
pub fn set_sequence_choose(choose: repl_extension_api::sequence::Choose) {
    SEQUENCE_CHOOSE.set(Some(choose));
}
pub fn clear_sequence_choose() {
    SEQUENCE_CHOOSE.set(None);
}

// The native caller copies this pointer before another call. No panic crosses FFI.
pub unsafe extern "C" fn call(request: *const c_char) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        if request.is_null() {
            return Err(crate::i18n::t("m068").to_owned());
        }
        let input = unsafe { CStr::from_ptr(request) }
            .to_str()
            .map_err(|e| e.to_string())?;
        dispatch(&serde_json::from_str(input).map_err(|e| e.to_string())?)
    });
    let reply = match result {
        Ok(Ok(value)) => json!({"ok":true,"value":value}),
        Ok(Err(error)) => json!({"ok":false,"error":error}),
        Err(_) => json!({"ok":false,"error":crate::i18n::t("m069")}),
    };
    REPLY.with(|slot| {
        *slot.borrow_mut() = CString::new(reply.to_string()).expect("JSON escapes NUL");
        slot.borrow().as_ptr()
    })
}

/// The catalog patch `patch`, or else the effect preset `preset` with `value`.
fn named(input: &Value) -> Result<crate::patch_reference::PatchReference, String> {
    let (format, plugin_id) = (argument(input, "format")?, argument(input, "pluginId")?);
    match input["patch"].as_str() {
        Some(patch) => crate::patch_state::patch_reference(format, plugin_id, patch),
        None => crate::patch_state::effect_reference(
            &crate::patch_browser::player()?,
            format,
            plugin_id,
            argument(input, "preset")?,
            argument(input, "value")?,
        ),
    }
}

/// `patchState` and `effectState`: the patch put into the track instance
/// `instance` when given, else a state file made without one.
fn patch_state(input: &Value) -> Result<Value, String> {
    let reference = named(input)?;
    Ok(json!(match input["instance"].as_i64() {
        Some(instance) =>
            crate::patch_state::into_instance(reference, instance as i32, input["created"] == true)?,
        None => Some(crate::patch_state::state_file(&reference)?),
    }))
}

fn argument<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input[key]
        .as_str()
        .ok_or_else(|| crate::i18n::render("f025", &[("key", key.to_string())]))
}

fn dispatch(input: &Value) -> Result<Value, String> {
    match argument(input, "api")? {
        api if api.starts_with("patchJob.") => crate::patch_jobs::dispatch(api, input),
        "patchCatalog" => {
            crate::patch_catalog::list(argument(input, "role")?, argument(input, "kind")?)
        }
        "patchCatalog.reload" => {
            crate::patch_catalog::reload();
            Ok(Value::Null)
        }
        "patchCatalog.instruments" => crate::patch_catalog::instruments(),
        "audio.setPadAutoGain" => {
            let instances: Vec<i32> =
                serde_json::from_value(input["instances"].clone()).map_err(|e| e.to_string())?;
            let gain = PAD_GAIN.get().ok_or("Pad auto gain is not loaded")?;
            if unsafe { (gain.set)(instances.as_ptr(), instances.len()) } {
                Ok(Value::Null)
            } else {
                Err("Expected up to 16 distinct pad instance IDs".into())
            }
        }
        // A script that blocks the control thread for long (an E2E cell) marks its own passes.
        "frameMonitor.beat" => {
            crate::frame_monitor::beat();
            Ok(Value::Null)
        }
        "power.setMinProcessorState" => {
            let percent = input["percent"].as_u64().filter(|&p| p <= 100);
            let percent = percent.ok_or(crate::i18n::t("min_processor_state_range"))?;
            crate::min_processor_state::set(percent as u32).map(|()| Value::Null)
        }
        "snippet.source" => Ok(json!(
            crate::snippets::Library::default().source(argument(input, "name")?)?
        )),
        api if api.starts_with("ui.") => crate::script_ui::dispatch(api, input),
        api if api.starts_with("keys.") => crate::keybinds::dispatch(api, input),
        api if api.starts_with("patchBrowser.") => crate::patch_browse::dispatch(api, input),
        "text" => {
            let key = argument(input, "key")?;
            if let Some(values) = input.get("values") {
                Ok(json!(crate::i18n::formatted(key, values)?))
            } else {
                Ok(json!(crate::i18n::text(key)?))
            }
        }
        "cmrt_chord.resolve_chord_or_mml" => {
            let resolved = cmrt_chord::resolve_chord_or_mml(argument(input, "input")?);
            Ok(json!({"mml":resolved.mml,"from_chord":resolved.from_chord}))
        }
        "cmrt_chord.note_progression" => Ok(json!(cmrt_chord::note_progression(argument(
            input, "input"
        )?)?)),
        "sequence.choose" => {
            let candidates =
                serde_json::from_value::<Vec<Vec<Vec<u8>>>>(input["candidates"].clone())
                    .map_err(|e| e.to_string())?;
            let previous = serde_json::from_value::<Vec<Vec<u8>>>(input["previous"].clone())
                .map_err(|e| e.to_string())?;
            let max = serde_json::from_value::<u8>(input["maxLeap"].clone())
                .map_err(|e| e.to_string())?;
            let pin = serde_json::from_value::<Option<usize>>(input["pin"].clone())
                .map_err(|e| e.to_string())?;
            SEQUENCE_CHOOSE
                .get()
                .ok_or("Rust sequence DLL is not loaded")?(
                &candidates, &previous, max, pin
            )
            .map(|path| json!(path))
            .ok_or_else(|| "No voicing satisfies the maximum leap".into())
        }
        "mmlabc_to_smf.mml_to_smf_bytes" => {
            let bytes = mmlabc_to_smf::mml_to_smf_bytes(argument(input, "mml")?)
                .map_err(|e| e.to_string())?;
            Ok(json!(bytes))
        }
        "mmlabc_to_smf.mml_preprocessor.extract_embedded_json" => {
            let result =
                mmlabc_to_smf::mml_preprocessor::extract_embedded_json(argument(input, "mml")?);
            Ok(
                json!({"embedded_json": result.embedded_json, "remaining_mml": result.remaining_mml,
                "remaining_offset": result.remaining_offset}),
            )
        }
        "mmlabc_to_smf.pass1_parser.parse_mml" => Ok(json!(
            mmlabc_to_smf::pass1_parser::parse_mml(argument(input, "mml")?)
        )),
        "mmlabc_to_smf.pass2_ast.tokens_to_ast" => {
            let tokens: Vec<mmlabc_to_smf::types::Token> =
                serde_json::from_value(input["tokens"].clone()).map_err(|e| e.to_string())?;
            Ok(json!(mmlabc_to_smf::pass2_ast::tokens_to_ast(&tokens)))
        }
        "mmlabc_to_smf.pass3_events.ast_to_events" => {
            let ast = serde_json::from_value(input["ast"].clone()).map_err(|e| e.to_string())?;
            let drums = input["use_drum_channel_for_128"]
                .as_bool()
                .ok_or("Expected drum mapping flag")?;
            Ok(json!(mmlabc_to_smf::pass3_events::ast_to_events(
                &ast, drums
            )))
        }
        "mmlabc_to_smf.pass4_midi.save_midi_file" => {
            let bytes: Vec<u8> = serde_json::from_value(input["bytes"].clone())
                .map_err(|e| crate::i18n::render("f026", &[("e", format!("{}", e))]))?;
            mmlabc_to_smf::pass4_midi::save_midi_file(&bytes, argument(input, "filepath")?)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "generatedMidiPath" => Ok(json!(generated_midi_path()?)),
        "projectsDirectory" => {
            let directory = crate::resources::user_directory()?.join("projects");
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            Ok(json!(directory))
        }
        "patchCheck" => crate::patch_state::check(&named(input)?).map(|()| Value::Null),
        "patchState" | "effectState" => patch_state(input),
        "sourceRoot" => Ok(json!(crate::resources::source_root())),
        "file.read" => {
            let path = argument(input, "filepath")?;
            Ok(json!(
                std::fs::read(path).map_err(|e| format!("{path}: {e}"))?
            ))
        }
        "file.list" => list_directory(argument(input, "directory")?),
        "zlib.decompress" => {
            let bytes: Vec<u8> =
                serde_json::from_value(input["bytes"].clone()).map_err(|e| e.to_string())?;
            let mut inflated = Vec::new();
            flate2::read::ZlibDecoder::new(bytes.as_slice())
                .read_to_end(&mut inflated)
                .map_err(|e| e.to_string())?;
            Ok(json!(inflated))
        }
        other => Err(crate::i18n::render("f027", &[("other", other.to_string())])),
    }
}

/// The directory's entries in name order, without descending into subdirectories.
fn list_directory(directory: &str) -> Result<Value, String> {
    let error = |e: std::io::Error| format!("{directory}: {e}");
    let mut entries = std::fs::read_dir(directory)
        .map_err(error)?
        .map(|entry| {
            let entry = entry.map_err(error)?;
            Ok((
                entry.file_name().to_string_lossy().into_owned(),
                entry.file_type().map_err(error)?.is_dir(),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort();
    Ok(json!(entries
        .into_iter()
        .map(|(name, directory)| json!({"name":name,"directory":directory}))
        .collect::<Vec<_>>()))
}

fn generated_midi_path() -> Result<String, String> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let directory = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(crate::i18n::t("m064"))?)
        .join("repl-plugin-player/generated-midi");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    loop {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!("{timestamp}-{}-{sequence}.mid", std::process::id()));
        if !path.try_exists().map_err(|e| e.to_string())? {
            return path
                .into_os_string()
                .into_string()
                .map_err(|_| crate::i18n::t("m070").into());
        }
    }
}
