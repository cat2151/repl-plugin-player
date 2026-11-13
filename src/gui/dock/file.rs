//! `layout-auto.toml` and the `[layout]` of `config.toml`: the main tree and each OS
//! window's tree as text, in the `row`/`column`/`tabs`/`view` form of
//! `../cat-smf-editor`'s `layout.toml`. Script windows are not written; a fixed pane
//! or an enabled extension the text leaves out joins the active tabs, and the view of
//! an extension that is not enabled is left out.
use super::monitor::{MonitorPlacement, KEY as MONITOR_KEY};
use super::{Float, Layout, Pane};
use eframe::egui;
use egui_tiles::{Container, LinearDir, Tile, TileId, Tiles, Tree};

/// The panes every layout has; an extension's view is its `<dll>/<pane>` name.
const VIEWS: [(&str, Pane); 6] = [
    ("editor", Pane::Editor),
    ("main", Pane::Main),
    ("console", Pane::Console),
    ("scope", Pane::Scope),
    ("log", Pane::Log),
    ("reference", Pane::Reference),
];

fn name(pane: &Pane) -> Option<&str> {
    match pane {
        Pane::Extension(name) => Some(name),
        _ => VIEWS.iter().find(|(_, p)| p == pane).map(|(name, _)| *name),
    }
}

/// The layout as the file's text; `[main]` is the main window, which only
/// `layout-auto.toml` has (`config.toml` has it in `[window]`).
pub(super) fn write(layout: &Layout) -> String {
    let fixed = VIEWS.iter().map(|(name, _)| *name);
    let views: Vec<_> = fixed
        .chain(layout.extensions.iter().map(String::as_str))
        .collect();
    let mut text = format!("# view: {}\n", views.join(", "));
    let main = layout
        .seen
        .get(&egui::ViewportId::ROOT)
        .copied()
        .or_else(|| {
            let [x, y] = layout.main.position?;
            let [width, height] = layout.main.size;
            Some(egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(width, height),
            ))
        });
    if let Some(rect) = main {
        text += &format!("\n[main]\n{}", place(rect));
    }
    let tiles = &layout.tree.tiles;
    if let Some(root) = layout.tree.root.and_then(|root| node(tiles, root)) {
        text += &format!(
            "
[root]
{root}
"
        );
    }
    for float in &layout.floating {
        let tiles = &float.tree.tiles;
        let Some(body) = float.tree.root.and_then(|root| node(tiles, root)) else {
            continue;
        };
        text += &format!(
            "
[[window]]
{body}
"
        );
        if let Some(placement) = layout.monitor.get(&float.id) {
            text += &placement.write();
        } else if let Some(rect) = layout.seen.get(&float.id).or(layout.placed.get(&float.id)) {
            text += &place(*rect);
        }
    }
    text
}

fn place(rect: egui::Rect) -> String {
    format!(
        "x = {}
y = {}
width = {}
height = {}
",
        rect.min.x.round(),
        rect.min.y.round(),
        rect.width().round(),
        rect.height().round()
    )
}

/// One tile as the keys of a table (`view = ...`, `row = [...]`); tabs of one pane are
/// written as the pane.
fn node(tiles: &Tiles<Pane>, id: TileId) -> Option<String> {
    let entry = |child: TileId, extra: String| {
        node(tiles, child).map(|body| format!("{{ {extra}{body} }}"))
    };
    Some(match tiles.get(id)? {
        Tile::Pane(pane) => format!("view = \"{}\"", name(pane)?),
        Tile::Container(Container::Tabs(tabs)) => {
            let shown: Vec<_> = (tabs.children.iter())
                .filter(|&&child| node(tiles, child).is_some())
                .copied()
                .collect();
            if let [only] = shown[..] {
                return node(tiles, only);
            }
            let children: Vec<_> = (shown.iter())
                .filter_map(|&child| {
                    let active = tabs.active == Some(child);
                    entry(child, if active { "active = true, " } else { "" }.into())
                })
                .collect();
            if children.is_empty() {
                return None;
            }
            format!("tabs = {}", list(&children))
        }
        Tile::Container(Container::Linear(linear)) => {
            let (key, share) = match linear.dir {
                LinearDir::Horizontal => ("row", "width"),
                LinearDir::Vertical => ("column", "height"),
            };
            let children: Vec<_> = (linear.children.iter())
                .filter_map(|&child| {
                    let value = (linear.shares[child] * 100.0).round() / 100.0;
                    entry(child, format!("{share} = {value}, "))
                })
                .collect();
            if children.is_empty() {
                return None;
            }
            format!("{key} = {}", list(&children))
        }
        Tile::Container(Container::Grid(grid)) => {
            let children: Vec<_> = (grid.children())
                .filter_map(|&child| entry(child, String::new()))
                .collect();
            if children.is_empty() {
                return None;
            }
            format!("grid = {}", list(&children))
        }
    })
}

fn list(children: &[String]) -> String {
    let lines: Vec<_> = children
        .iter()
        .map(|child| format!("  {},\n", child.replace('\n', "\n  ")))
        .collect();
    format!("[\n{}]", lines.concat())
}

/// The layout the text describes with the enabled `extensions`, script panes aside.
pub(super) fn read(text: &str, extensions: &[String]) -> Result<Layout, String> {
    let mut root: toml::Table = toml::from_str(text).map_err(|error| error.to_string())?;
    let main = match root.remove("main") {
        None => None,
        Some(toml::Value::Table(main)) => {
            if let Some(key) = main
                .keys()
                .find(|key| !WINDOW_KEYS[..4].contains(&key.as_str()))
            {
                return Err(crate::i18n::render("layout_key", &[("key", key.clone())]));
            }
            rect(&main)?
        }
        Some(_) => return Err(crate::i18n::render("layout_key", &[("key", "main".into())])),
    };
    let mut layout = read_table(&root, extensions)?;
    if let Some(rect) = main {
        layout.main.position = Some(rect.min.into());
        layout.main.size = rect.size().into();
    }
    Ok(layout)
}

/// The layout a table of `root` and `window` describes with the enabled `extensions`,
/// script panes aside.
pub(super) fn read_table(root: &toml::Table, extensions: &[String]) -> Result<Layout, String> {
    let mut tiles = Tiles::default();
    let mut seen = Seen {
        panes: vec![],
        extensions,
    };
    let mut layout = Layout {
        extensions: extensions.to_vec(),
        ..Layout::default()
    };
    for (key, value) in root {
        match (key.as_str(), value) {
            ("root", toml::Value::Table(table)) => {
                if let Some(root) = build(&mut tiles, table, &[], &mut seen)? {
                    layout.tree = Tree::new(egui::Id::unique("dock"), root, tiles);
                }
                tiles = Tiles::default();
            }
            ("window", toml::Value::Array(windows)) => {
                for window in windows {
                    let window = window.as_table().ok_or("[[window]]")?;
                    let first = seen.panes.len();
                    let mut tiles = Tiles::default();
                    let root = build(&mut tiles, window, &WINDOW_KEYS, &mut seen)?;
                    let rect = rect(window)?;
                    let Some(root) = root else {
                        continue;
                    };
                    let pane = seen
                        .panes
                        .get(first)
                        .ok_or_else(|| crate::i18n::t("layout_node"))?;
                    let id = pane.viewport();
                    let float = Float {
                        id,
                        tree: Tree::new(Float::tree_id(id), root, tiles),
                    };
                    match window.get(MONITOR_KEY).map(MonitorPlacement::read) {
                        Some(_) if rect.is_some() => {
                            return Err(crate::i18n::t("layout_monitor").into())
                        }
                        Some(placement) => {
                            layout.monitor.insert(float.id, placement?);
                        }
                        None => {}
                    }
                    if let Some(rect) = rect {
                        layout.placed.insert(float.id, rect);
                    }
                    layout.floating.push(float);
                }
            }
            _ => return Err(crate::i18n::render("layout_key", &[("key", key.clone())])),
        }
    }
    let target = layout.target();
    let active = |layout: &Layout| match target.and_then(|t| layout.tree.tiles.get(t)) {
        Some(Tile::Container(Container::Tabs(tabs))) => tabs.active,
        _ => None,
    };
    let chosen = active(&layout);
    let fixed = VIEWS.into_iter().map(|(_, pane)| pane);
    let enabled = extensions.iter().map(|name| Pane::Extension(name.clone()));
    for pane in fixed.chain(enabled) {
        if !seen.panes.contains(&pane) {
            layout.dock(pane);
        }
    }
    if let (Some(chosen), Some(Tile::Container(Container::Tabs(tabs)))) =
        (chosen, target.and_then(|t| layout.tree.tiles.get_mut(t)))
    {
        tabs.set_active(chosen);
    }
    Ok(layout)
}

/// The panes read so far, and the extensions that have one.
struct Seen<'a> {
    panes: Vec<Pane>,
    extensions: &'a [String],
}

/// The pane of view `name`; `None` for an extension that is not enabled.
fn view(name: &str, seen: &mut Seen) -> Result<Option<Pane>, String> {
    let fixed = VIEWS.iter().find(|(view, _)| *view == name);
    let pane = match fixed {
        Some((_, pane)) => pane.clone(),
        None if seen.extensions.iter().any(|enabled| enabled == name) => {
            Pane::Extension(name.into())
        }
        None if crate::extension::split(name).is_some_and(|(dll, _)| {
            crate::resources::extension_names()
                .iter()
                .any(|known| known == dll)
        }) =>
        {
            return Ok(None)
        }
        None => return Err(crate::i18n::render("layout_view", &[("view", name.into())])),
    };
    if seen.panes.contains(&pane) {
        return Err(crate::i18n::render(
            "layout_twice",
            &[("view", name.into())],
        ));
    }
    seen.panes.push(pane.clone());
    Ok(Some(pane))
}

/// The keys of a window's place, read beside its tree: `x`, `y`, `width`, `height`, or
/// the `monitor_placement` table instead.
const WINDOW_KEYS: [&str; 5] = ["x", "y", "width", "height", MONITOR_KEY];

/// `x`, `y`, `width`, `height` together, or none of them.
fn rect(window: &toml::Table) -> Result<Option<egui::Rect>, String> {
    let keys = &WINDOW_KEYS[..4];
    let values: Vec<_> = keys
        .iter()
        .filter_map(|key| number(window.get(*key)?))
        .collect();
    match values[..] {
        [] if keys.iter().all(|key| !window.contains_key(*key)) => Ok(None),
        [x, y, width, height] if width > 0.0 && height > 0.0 => Ok(Some(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height)),
        )),
        _ => Err(crate::i18n::t("layout_rect").into()),
    }
}

fn number(value: &toml::Value) -> Option<f32> {
    match value {
        toml::Value::Integer(value) => Some(*value as f32),
        toml::Value::Float(value) if value.is_finite() => Some(*value as f32),
        _ => None,
    }
}

/// The keys a tabs container reads from its children.
const TAB_KEYS: &[&str] = &["active"];

/// One node table into `tiles`; `allowed` are the keys its parent reads from it. A view
/// outside tabs gets tabs of its own, as `all_panes_must_have_tabs` wants. `None` when
/// nothing is left: the view of an extension that is not enabled, or a container of
/// only such views.
fn build(
    tiles: &mut Tiles<Pane>,
    table: &toml::Table,
    allowed: &[&str],
    seen: &mut Seen,
) -> Result<Option<TileId>, String> {
    let kinds = ["view", "row", "column", "tabs", "grid"];
    for key in table.keys() {
        if !kinds.contains(&key.as_str()) && !allowed.contains(&key.as_str()) {
            return Err(crate::i18n::render("layout_key", &[("key", key.clone())]));
        }
    }
    let mut found = kinds
        .iter()
        .filter_map(|kind| Some((*kind, table.get(*kind)?)));
    let (Some((kind, value)), None) = (found.next(), found.next()) else {
        return Err(crate::i18n::t("layout_node").into());
    };
    if kind == "view" {
        let Some(pane) = view(value.as_str().unwrap_or(""), seen)? else {
            return Ok(None);
        };
        let pane = tiles.insert_pane(pane);
        return Ok(Some(if allowed == TAB_KEYS {
            pane
        } else {
            tiles.insert_tab_tile(vec![pane])
        }));
    }
    let children = value
        .as_array()
        .ok_or_else(|| crate::i18n::t("layout_node"))?;
    let child_keys: &[&str] = match kind {
        "row" => &["width"],
        "column" => &["height"],
        "tabs" => TAB_KEYS,
        _ => &[],
    };
    let mut ids = vec![];
    for child in children {
        let child = child
            .as_table()
            .ok_or_else(|| crate::i18n::t("layout_node"))?;
        if let Some(id) = build(tiles, child, child_keys, seen)? {
            ids.push((id, child));
        }
    }
    let only: Vec<_> = ids.iter().map(|(id, _)| *id).collect();
    if only.is_empty() && !children.is_empty() {
        return Ok(None);
    }
    Ok(Some(match kind {
        "row" | "column" => {
            let dir = if kind == "row" {
                LinearDir::Horizontal
            } else {
                LinearDir::Vertical
            };
            let mut linear = egui_tiles::Linear::new(dir, only);
            for (id, child) in &ids {
                if let Some(share) = child.get(child_keys[0]).and_then(number) {
                    linear.shares.set_share(*id, share);
                }
            }
            tiles.insert_container(linear)
        }
        "tabs" => {
            let mut tabs = egui_tiles::Tabs::new(only);
            let active = ids.iter().find(|(_, child)| {
                child.get("active").and_then(toml::Value::as_bool) == Some(true)
            });
            if let Some((id, _)) = active {
                tabs.set_active(*id);
            }
            tiles.insert_container(tabs)
        }
        _ => tiles.insert_container(egui_tiles::Grid::new(only)),
    }))
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
