use super::super::harness::{keyboard, with_keyboard};
use super::*;
use eframe::egui;

fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("rpp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// Lets the next poll run without waiting.
fn poll(file: &mut LayoutFile, layout: &mut Layout) {
    file.last -= POLL;
    file.poll(layout);
}

#[test]
fn the_file_is_written_read_back_and_kept_while_broken() {
    let path = directory("layout-file").join("layout-auto.toml");
    let (mut file, mut layout) = LayoutFile::open(path.parent().unwrap(), true, &with_keyboard());
    assert!(layout.floating.is_empty());
    poll(&mut file, &mut layout);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("view = \"console\""), "{written}");

    // An edit is applied; a moved window is written.
    let floated = written.replace(
        "{ height = 126, view = \"builtin/keyboard\" },
",
        "",
    ) + "
[[window]]
view = \"builtin/keyboard\"
";
    std::fs::write(&path, floated).unwrap();
    poll(&mut file, &mut layout);
    assert!(file.error.is_empty(), "{}", file.error);
    assert_eq!(layout.floating_panes(), [keyboard()]);
    assert!(layout.tree.tiles.find_pane(&Pane::Console).is_some());
    let moved = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, 100.0));
    layout.seen.insert(keyboard().viewport(), moved);
    poll(&mut file, &mut layout);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("x = 10\ny = 20\nwidth = 300\nheight = 100"),
        "{written}"
    );

    // A broken edit keeps the layout and the user's text.
    std::fs::write(&path, "[root\n").unwrap();
    poll(&mut file, &mut layout);
    assert!(!file.error.is_empty());
    assert_eq!(layout.floating_panes(), [keyboard()]);
    layout.seen.clear();
    poll(&mut file, &mut layout);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[root\n");

    // Reopened, the fixed text is the layout.
    std::fs::write(&path, written).unwrap();
    let (file, layout) = LayoutFile::open(path.parent().unwrap(), true, &with_keyboard());
    assert!(file.error.is_empty(), "{}", file.error);
    assert_eq!(layout.placed[&keyboard().viewport()], moved);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn config_layout_is_read_and_never_written_and_switching_keeps_both() {
    let directory = directory("layout-config");
    std::fs::create_dir_all(&directory).unwrap();
    let config = directory.join("config.toml");
    let text = "# mine\n[window]\nwidth = 900\n\n[layout.root]\nrow = [{ view = \"log\" }, { view = \"editor\" }]\n";
    std::fs::write(&config, text).unwrap();
    let (mut file, mut layout) = LayoutFile::open(&directory, false, &with_keyboard());
    assert!(file.error.is_empty(), "{}", file.error);
    layout.float(layout.tree.tiles.find_pane(&Pane::Log).unwrap());
    poll(&mut file, &mut layout);
    file.save(&layout);
    assert_eq!(std::fs::read_to_string(&config).unwrap(), text);
    assert!(!directory.join("layout-auto.toml").exists());

    // An edit of the file is still applied.
    std::fs::write(&config, "[[layout.window]]\nview = \"scope\"\n").unwrap();
    poll(&mut file, &mut layout);
    assert!(file.error.is_empty(), "{}", file.error);
    assert_eq!(layout.floating_panes(), [Pane::Scope]);

    // To auto: no file yet, so the first layout, which is then written; back to the
    // config's layout; to auto again: the written layout.
    file.switch(&mut layout);
    assert!(layout.floating.is_empty());
    let auto = directory.join("layout-auto.toml");
    layout.float(layout.tree.tiles.find_pane(&keyboard()).unwrap());
    file.switch(&mut layout);
    assert_eq!(layout.floating_panes(), [Pane::Scope]);
    assert!(std::fs::read_to_string(&auto)
        .unwrap()
        .contains("view = \"builtin/keyboard\""));
    file.switch(&mut layout);
    assert_eq!(layout.floating_panes(), [keyboard()]);
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn switching_takes_every_window_place_from_the_new_source_only() {
    let directory = directory("layout-switch");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("config.toml"),
        "[window]\nx = 50\ny = 60\nwidth = 900\nheight = 700\n\n[[layout.window]]\nview = \"scope\"\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("layout-auto.toml"),
        "[main]\nx = 300\ny = 200\nwidth = 1100\nheight = 650\n\n\
         [[window]]\nview = \"scope\"\nx = 900\ny = 400\nwidth = 500\nheight = 250\n",
    )
    .unwrap();
    let (mut file, mut layout) = LayoutFile::open(&directory, true, &with_keyboard());
    assert!(file.error.is_empty(), "{}", file.error);
    assert_eq!(layout.main.position, Some([300.0, 200.0]));
    let scope = Pane::Scope.viewport();
    // Where the user moved things while auto-saving.
    let main = egui::Rect::from_min_size(egui::pos2(310.0, 210.0), egui::vec2(1000.0, 600.0));
    layout.seen.insert(egui::ViewportId::ROOT, main);
    let moved = egui::Rect::from_min_size(egui::pos2(950.0, 450.0), egui::vec2(400.0, 200.0));
    layout.seen.insert(scope, moved);

    file.switch(&mut layout);
    assert!(layout.switched);
    assert_eq!(layout.main.position, Some([50.0, 60.0]));
    assert_eq!(layout.main.size, [900.0, 700.0]);
    assert!(layout.seen.is_empty() && layout.placed.is_empty());
    let auto = std::fs::read_to_string(directory.join("layout-auto.toml")).unwrap();
    assert!(
        auto.contains("[main]\nx = 310\ny = 210\nwidth = 1000\nheight = 600"),
        "{auto}"
    );

    layout.switched = false;
    file.switch(&mut layout);
    assert!(layout.switched);
    assert_eq!(layout.main.position, Some([310.0, 210.0]));
    assert_eq!(layout.main.size, [1000.0, 600.0]);
    assert_eq!(layout.placed[&scope], moved);
    std::fs::remove_dir_all(&directory).unwrap();
}

/// The `[layout]` the author has in `config.toml`, read with keyboard off.
#[test]
fn the_config_layout_reads_with_keyboard_off() {
    let directory = directory("layout-config-off");
    std::fs::create_dir_all(&directory).unwrap();
    let text =
        "[[layout.window]]\ncolumn = [{ view = \"builtin/keyboard\" }, { view = \"scope\" }]\n";
    std::fs::write(directory.join("config.toml"), text).unwrap();
    let (file, layout) = LayoutFile::open(&directory, false, &[]);
    assert!(file.error.is_empty(), "{}", file.error);
    assert_eq!(layout.floating_panes(), [Pane::Scope]);
    assert!(layout.tree.tiles.find_pane(&keyboard()).is_none());
    std::fs::remove_dir_all(&directory).unwrap();
}
