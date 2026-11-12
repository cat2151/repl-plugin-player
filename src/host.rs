//! The plugin identity that cat-plugin-player's resident protocol names, without
//! that player's native host library.
#[path = "../../cat-plugin-player/shared/identity.rs"]
#[allow(dead_code)] // Shared with the player; this app only reads `PluginKey`.
mod identity;
pub use identity::PluginKey;

/// The catalog fields `identity.rs` copies into a [`PluginKey`].
#[allow(dead_code)]
pub struct PluginInfo {
    pub format: String,
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub bundle_path: String,
}
