//! Puts a patch into a live instance of its plugin. Floe and Dexed take the
//! patch directly; for the rest the instance loads a state, built on the
//! state of a new instance when the plugin needs a template.
use super::{built, clap, clap::Plugin, dexed, floe, state, templated};
use crate::patch_reference::PatchReference;
use std::{
    ffi::{c_char, c_void},
    path::Path,
};

/// A patch that goes into the instance without a plugin state.
pub(super) enum Direct<'a> {
    Floe(&'a Path),
    Dexed(&'a str),
}

impl<'a> Direct<'a> {
    pub(super) fn of(reference: &'a PatchReference) -> Option<Self> {
        floe::preset(reference)
            .map(Self::Floe)
            .or_else(|| dexed::program(reference).map(Self::Dexed))
    }

    /// Fails as [`Self::load`] would on a patch that cannot be read, without an instance.
    pub(super) fn check(&self) -> Result<(), String> {
        match self {
            Self::Floe(path) => floe::check(path),
            Self::Dexed(path) => dexed::voice(path).map(|_| ()),
        }
    }

    pub(super) fn load(&self, plugin: Plugin) -> Result<(), String> {
        match self {
            Self::Floe(path) => floe::load(plugin, path),
            Self::Dexed(path) => plugin.send_sysex(&dexed::voice(path)?),
        }
    }
}

/// Fails as [`into`] would on a patch it cannot read, before an instance exists.
pub(super) fn check(reference: &PatchReference) -> Result<(), String> {
    if let Some(direct) = Direct::of(reference) {
        return direct.check();
    }
    if templated(reference) {
        return built::check(&reference.plugin.id, &reference.source);
    }
    Ok(())
}

/// A checked patch, with the state made ahead unless the patch goes into the
/// instance directly or is built on a template not kept yet.
pub(super) struct Prepared {
    pub(super) reference: PatchReference,
    pub(super) state: Option<Vec<u8>>,
}

/// `restore` asks the player for a state only it makes. Runs on any thread.
pub(super) fn prepare(
    reference: PatchReference,
    restore: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<Prepared, String> {
    let state = if let Some(direct) = Direct::of(&reference) {
        direct.check().map(|()| None)?
    } else if templated(&reference) {
        match built::kept(&reference.plugin.id) {
            Some(template) => Some(built::build(
                &template,
                &reference.plugin.id,
                &reference.source,
            )?),
            None => built::check(&reference.plugin.id, &reference.source).map(|()| None)?,
        }
    } else {
        Some(state(&reference, None, restore)?)
    };
    Ok(Prepared { reference, state })
}

impl Prepared {
    /// The reference's JSON after its length (u32, little-endian), then the
    /// state when there is one.
    pub(super) fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let json = serde_json::to_vec(&self.reference).map_err(|e| e.to_string())?;
        let length = u32::try_from(json.len()).map_err(|e| e.to_string())?;
        let state = self.state.as_deref().unwrap_or_default();
        Ok([&length.to_le_bytes(), json.as_slice(), state].concat())
    }

    pub(super) fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let malformed = || "Not a prepared patch".to_string();
        let (length, rest) = bytes.split_first_chunk::<4>().ok_or_else(malformed)?;
        let length = u32::from_le_bytes(*length) as usize;
        let json = rest.get(..length).ok_or_else(malformed)?;
        let reference = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        let state = &rest[length..];
        Ok(Self {
            reference,
            state: (!state.is_empty()).then(|| state.to_vec()),
        })
    }
}

/// The patch put into `plugin`: `None` when it went in directly, else the state
/// the caller loads into the instance. A plugin needing a template it lacks
/// gets the state of the instance when `fresh` tells the instance is new, else
/// of another new instance made for it. Runs on the control (CLAP main)
/// thread; `plugin` is called only when needed.
pub(super) fn into(
    plugin: impl FnOnce() -> Result<Plugin, String>,
    prepared: Prepared,
    fresh: bool,
) -> Result<Option<Vec<u8>>, String> {
    let reference = &prepared.reference;
    if let Some(direct) = Direct::of(reference) {
        return direct.load(plugin()?).map(|()| None);
    }
    if let Some(state) = prepared.state {
        return Ok(Some(state));
    }
    if !templated(reference) {
        return Err("The patch was prepared without its state".into());
    }
    let id = reference.plugin.id.as_str();
    if built::kept(id).is_none() {
        let template = match fresh {
            true => plugin()?.save()?,
            false => clap::fresh_state(&reference.plugin.format, id)?,
        };
        built::keep(id, template);
    }
    let template = built::kept(id).expect("kept above");
    built::build(&template, id, &reference.source).map(Some)
}

/// Receives the state the host loads into the instance next.
pub type StateOut = unsafe extern "C" fn(user: *mut c_void, state: *const u8, size: usize);

/// The bridge's `repl_set_patch_into`: [`into`] for an evolve host, called on
/// the control thread after it made the instance and before the instance
/// plays. `prepared` is a file [`Prepared::to_bytes`] wrote; `plugin` is the
/// instance's `clap_plugin*`, null when it is not CLAP. A state to load goes to
/// `state`; false fails with `error`.
///
/// # Safety
/// `plugin` is null or a live `clap_plugin*`, `prepared` holds `size` bytes and
/// `error` `capacity` bytes.
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn patch_into(
    plugin: *const c_void,
    prepared: *const u8,
    size: usize,
    fresh: bool,
    state: StateOut,
    user: *mut c_void,
    error: *mut c_char,
    capacity: usize,
) -> bool {
    let bytes = unsafe { std::slice::from_raw_parts(prepared, size) };
    let instance = || {
        (!plugin.is_null())
            .then(|| unsafe { Plugin::given(plugin) })
            .ok_or_else(|| "The instance is not a CLAP plugin".to_string())
    };
    let result = std::panic::catch_unwind(|| into(instance, Prepared::from_bytes(bytes)?, fresh))
        .unwrap_or_else(|_| Err("Putting the patch into the instance panicked".into()));
    match result {
        Ok(made) => {
            if let Some(made) = made {
                unsafe { state(user, made.as_ptr(), made.len()) };
            }
            true
        }
        Err(message) => {
            if capacity > 0 {
                let mut length = message.len().min(capacity - 1);
                while !message.is_char_boundary(length) {
                    length -= 1;
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), length);
                    *error.add(length) = 0;
                }
            }
            false
        }
    }
}
