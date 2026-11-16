//! The types the app passes to an extension DLL. The app and every extension are built
//! in the same cargo workspace, so these Rust types are shared as they are.
//!
//! # Adding an extension from another repo
//!
//! 1. Put the extension's repo next to this repo and point to it with a junction:
//!    `mklink /J extensions\<dll> ..\<extension repo>`.
//! 2. In the extension's `Cargo.toml`, name the package `<dll>`, make it a `cdylib`
//!    (`[lib]` with `test = false` and `doctest = false`), and depend on
//!    `eframe.workspace = true` and `repl-extension-api = { path = "../../extension-api" }`.
//!    The path counts from the junction (`extensions/<dll>/`), not from the repo's real place.
//! 3. Export [`LIST`] as a [`List`]: `#[no_mangle] pub fn repl_extensions() -> &'static [Extension]`,
//!    one [`Extension`] per pane. Panes of one DLL can share its statics.
//! 4. Turn each pane `<dll>/<pane>` on in the settings (`Ctrl+,`), then restart the app.
//! 5. Build and test in this repo without `-p` (`cargo build --release`, `cargo test --release`).
//!    The extension's repo cannot build on its own. The tests here load every DLL under
//!    `extensions/` and draw each of its panes.
//! 6. When they pass, commit and push the extension's repo only. Do not commit what it
//!    added to this repo's `Cargo.lock`.
//! 7. Before removing the junction, turn its panes off in the settings and restart the app
//!    once, so `layout-auto.toml` no longer names them; a view whose DLL dir
//!    is gone is an unknown view, and the saved layout is not read.
//! 8. Remove the junction with `rmdir extensions\<dll>`. A delete that removes contents
//!    can delete the extension repo's files through the junction.
use eframe::egui;
pub mod sequence;

/// What the app knows when it draws an extension's pane.
pub struct Frame<'a> {
    /// The MIDI notes sounding now.
    pub active_notes: &'a [u8],
    /// The lowest and highest notes in the timeline, when there are any.
    pub range: Option<(u8, u8)>,
    /// The app's language, such as `"ja"` or `"en"`.
    pub language: &'a str,
}

/// Draws a pane into `ui`.
pub type Show = fn(&mut egui::Ui, &Frame);

/// One pane of a DLL. `name` is the part after `/` in `config.toml`, and the tab title.
pub struct Extension {
    pub name: &'static str,
    pub show: Show,
}

/// The function every extension DLL exports as [`LIST`]: its panes.
pub type List = fn() -> &'static [Extension];

/// The symbol name of the [`List`] function.
pub const LIST: &[u8] = b"repl_extensions\0";

/// The `TypeId` of `egui::Context` in the build that links this crate. Every DLL exports
/// it through this crate, so the app can compare it with its own before calling the DLL.
/// They differ when the DLL was built with `-p`: then egui's state stored by one side is
/// not found by the other, and the DLL's panic aborts the app.
#[no_mangle]
pub fn repl_egui_type_id() -> std::any::TypeId {
    std::any::TypeId::of::<egui::Context>()
}

/// The symbol name of [`repl_egui_type_id`].
pub const EGUI_TYPE_ID: &[u8] = b"repl_egui_type_id\0";
pub mod evolution;
