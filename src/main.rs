#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bridge;
mod catalog_refresh;
mod cli;
mod config;
mod control;
mod e2e;
mod e2e_script;
mod execution;
mod execution_e2e;
mod external_editor;
mod gui;
mod host;
mod i18n;
mod install;
mod keyboard;
mod log;
mod patch_browse;
mod patch_browse_e2e;
mod patch_browser;
mod patch_cell;
mod patch_state;
mod player_download;
mod preparation;
mod resources;
mod scope;
mod scope_boundary;
mod scope_capture;
mod scope_drift;
mod scope_e2e;
mod scope_stream;
mod scope_trigger;
mod scope_ui;
mod script_api;
mod script_editor;
mod script_ui;
mod snippet_insert;
mod snippets;
mod snippets_e2e;
mod snippets_ui;
mod status;
mod surge_home;
mod syntax_highlight;
mod timeline;
mod tutorials_e2e;

// cat-plugin-player's resident protocol and the types it names, from its source
// of truth. This app uses only part of them.
#[path = "../../cat-plugin-player/shared/drum_kit.rs"]
#[allow(dead_code)]
mod drum_kit;
#[path = "../../cat-plugin-player/shared/patch_identity.rs"]
mod patch_identity;
#[path = "../../cat-plugin-player/shared/patch_reference.rs"]
#[allow(dead_code)]
mod patch_reference;
#[path = "../../cat-plugin-player/shared/resident_ipc.rs"]
#[allow(dead_code)]
mod resident_ipc;

fn main() {
    surge_home::prepare();
    let args: Vec<String> = std::env::args().collect();
    let args = match i18n::initialize(&args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let result = if args.get(1).is_some_and(|arg| arg == "--e2e") {
        e2e::run(args.get(2).map_or("sample", String::as_str))
    } else if args.len() > 1 {
        cli::run(&args[1..])
    } else {
        gui::run()
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
