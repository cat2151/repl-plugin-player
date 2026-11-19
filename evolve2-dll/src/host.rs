//! What evolve2 needs from the plugin host.
//!
//! Creating an instance and loading a state are callback-shaped in UAPMD, and
//! the host may finish them on the main thread. Blocking a worker on a host
//! call would therefore stall that worker until the next UI frame, so both
//! report through `done` instead of returning.
use std::sync::Arc;

/// Opaque id of one physical instance, chosen by the host.
pub type InstanceHandle = u64;

/// One plugin type; instances of the same kind can take each other's state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginKind {
    pub format: String,
    pub plugin_id: String,
}

pub type CreateDone = Box<dyn FnOnce(Result<InstanceHandle, String>) + Send>;
pub type LoadDone = Box<dyn FnOnce(Result<(), String>) + Send>;

/// Every `done` must be called exactly once, from any thread. The workers wait
/// for it without a timeout.
pub trait Host: Send + Sync {
    fn create_instance(&self, kind: &PluginKind, done: CreateDone);
    fn destroy_instance(&self, handle: InstanceHandle);
    /// The host copies `data` before returning.
    fn load_state(&self, handle: InstanceHandle, data: &[u8], done: LoadDone);
    /// Makes `slot` render `handle`. A detached instance is never processed.
    fn attach_to_slot(&self, slot: u8, handle: InstanceHandle) -> bool;
    fn detach_from_slot(&self, slot: u8) -> Option<InstanceHandle>;
}

pub type SharedHost = Arc<dyn Host>;
