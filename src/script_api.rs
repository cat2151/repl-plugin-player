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

thread_local! {
    static REPLY: RefCell<CString> = RefCell::new(CString::default());
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

fn argument<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input[key]
        .as_str()
        .ok_or_else(|| crate::i18n::render("f025", &[("key", key.to_string())]))
}

fn dispatch(input: &Value) -> Result<Value, String> {
    match argument(input, "api")? {
        "snippet.source" => Ok(json!(
            crate::snippets::Library::default().source(argument(input, "name")?)?
        )),
        api @ ("ui.show" | "ui.close") => crate::script_ui::dispatch(api, input),
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
        "patchState" => Ok(json!(crate::patch_state::file(
            argument(input, "format")?,
            argument(input, "pluginId")?,
            argument(input, "patch")?,
        )?)),
        "effectState" => Ok(json!(crate::patch_state::effect_file(
            argument(input, "format")?,
            argument(input, "pluginId")?,
            argument(input, "preset")?,
            argument(input, "value")?,
        )?)),
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
