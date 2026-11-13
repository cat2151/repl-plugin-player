use super::*;

fn panes(layout: &Layout) -> Vec<Pane> {
    let mut panes: Vec<_> = (layout.tree.tiles.iter())
        .filter_map(|(_, tile)| match tile {
            Tile::Pane(pane) => Some(pane.clone()),
            _ => None,
        })
        .collect();
    panes.sort_by_key(|pane| format!("{pane:?}"));
    panes
}

#[test]
fn the_first_layout_reads_back_as_written() {
    let first = super::super::initial();
    let text = write(&first);
    println!("{text}");
    let read = read(&text).unwrap();
    assert_eq!(write(&read), text);
    assert!(read.floating.is_empty());
    assert_eq!(panes(&read), panes(&first));
}

#[test]
fn tabs_shares_and_window_places_are_kept() {
    let text = r#"
[root]
row = [
  { width = 2, view = "editor" },
  { width = 1, column = [
    { height = 3, tabs = [{ view = "main" }, { active = true, view = "log" }] },
    { height = 1, view = "scope" },
  ] },
]

[[window]]
view = "console"
x = -1200
y = 40
width = 500
height = 180

[[window]]
view = "keyboard"
"#;
    let layout = read(text).unwrap();
    assert_eq!(layout.floating_panes(), [Pane::Console, Pane::Keyboard]);
    let console = layout.placed[&Pane::Console.viewport()];
    assert_eq!((console.min.x, console.min.y), (-1200.0, 40.0));
    assert_eq!(console.size(), egui::vec2(500.0, 180.0));
    let tiles = &layout.tree.tiles;
    let log = tiles.find_pane(&Pane::Log).unwrap();
    let Some(Tile::Container(Container::Tabs(tabs))) = tiles.get(tiles.parent_of(log).unwrap())
    else {
        panic!("log is not in tabs");
    };
    assert_eq!(tabs.active, Some(log));
    assert!(
        tiles.find_pane(&Pane::Reference).is_some(),
        "left out, joins the tree"
    );
    let again = write(&layout);
    println!("{again}");
    assert_eq!(write(&read(&again).unwrap()), again);
    assert!(again.contains("width = 2, view = \"editor\""));
    assert!(again.contains("active = true, view = \"log\""));
}

#[test]
fn wrong_text_is_refused() {
    for text in [
        "[root\n",
        "[root]\nview = \"nothing\"\n",
        "[root]\nrow = [{ view = \"main\" }, { view = \"main\" }]\n",
        "[root]\nrow = [{ view = \"main\", tabs = [] }]\n",
        "[root]\ncolumn = [{ width = 1, view = \"main\" }]\n",
        "[[window]]\nview = \"console\"\nx = 1\n",
        "[[window]]\nview = \"console\"\nx = 1\ny = 1\nwidth = 0\nheight = 1\n",
        "[panes]\n",
        "[[window]]\nrow = []\n",
        "[[window]]\nview = \"console\"\nactive = true\n",
        "[[window]]\nview = \"console\"\nx = 1\ny = 1\nwidth = 1\nheight = 1\n\
         [window.monitor_placement]\nmonitor = 1\nleft_percent = 0\ntop_percent = 0\n\
         width_percent = 100\nheight_percent = 100\n",
        "[[window]]\nview = \"console\"\n[window.monitor_placement]\nmonitor = 1\n",
        "[root]\nview = \"main\"\nmonitor_placement = 1\n",
        "[main]\nx = 1\ny = 1\n",
        "[main]\nx = 1\ny = 1\nwidth = 1\nheight = 1\nview = \"main\"\n",
        "main = 1\n",
    ] {
        assert!(read(text).is_err(), "{text}");
    }
}

#[test]
fn a_monitor_placement_is_kept_over_where_the_window_was_drawn() {
    let text = r#"
[[window]]
view = "scope"
[window.monitor_placement]
monitor = "right"
left_percent = 0
top_percent = 0
width_percent = 100
height_percent = 100
"#;
    let mut layout = read(text).unwrap();
    let id = Pane::Scope.viewport();
    assert_eq!(
        layout.monitor[&id].monitor,
        super::super::monitor::Monitor::Position(super::super::monitor::Position::Right)
    );
    assert!(!layout.placed.contains_key(&id));
    let moved = egui::Rect::from_min_size(egui::pos2(5.0, 6.0), egui::vec2(300.0, 200.0));
    layout.seen.insert(id, moved);
    let again = write(&layout);
    println!("{again}");
    assert!(
        again.contains("monitor = \"right\"") && !again.contains("x = "),
        "{again}"
    );
    assert_eq!(write(&read(&again).unwrap()), again);
}

#[test]
fn script_windows_are_not_written() {
    let mut layout = super::super::initial();
    layout
        .floating
        .push(Float::single(Pane::Script("w".into())));
    layout.dock(Pane::Script("tab".into()));
    let text = write(&layout);
    assert!(!text.contains("\"w\"") && !text.contains("tab\""), "{text}");
    assert!(read(&text).is_ok());
}

#[test]
fn a_window_tree_is_kept_and_named_by_its_first_view() {
    let text = r#"
[[window]]
row = [
  { width = 2, tabs = [{ view = "scope" }, { active = true, view = "log" }] },
  { width = 1, view = "keyboard" },
]
x = 10
y = 20
width = 700
height = 400
"#;
    let layout = read(text).unwrap();
    assert_eq!(
        layout.floating_panes(),
        [Pane::Scope, Pane::Log, Pane::Keyboard]
    );
    let id = layout.floating[0].id;
    assert_eq!(id, Pane::Scope.viewport());
    assert_eq!(layout.placed[&id].size(), egui::vec2(700.0, 400.0));
    assert!(layout.tree.tiles.find_pane(&Pane::Log).is_none());
    let again = write(&layout);
    println!("{again}");
    assert!(again.contains("active = true, view = \"log\""), "{again}");
    assert!(again.contains("width = 1, view = \"keyboard\""), "{again}");
    assert_eq!(write(&read(&again).unwrap()), again);
}
