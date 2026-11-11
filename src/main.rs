#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bridge;
mod cli;
mod control;
mod demo_play_e2e;
mod e2e;
mod execution;
mod execution_e2e;
mod gui;
mod i18n;
mod install;
mod keyboard;
mod log;
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
mod surge_state;
mod syntax_highlight;
mod timeline;
mod tutorials_e2e;

fn main() {
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
