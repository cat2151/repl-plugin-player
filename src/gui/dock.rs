//! The main window as `egui_tiles`: every section is a tab that can be split and moved,
//! or taken out of the tree into its own OS window (`float`).
use super::Editor;
use crate::{script_ui::Window, timeline::projection::View};
use eframe::egui;
use egui_tiles::{Container, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse};
use std::collections::HashMap;

mod file;
mod float;
mod harness;
mod monitor;
mod owner;
pub(super) mod persist;
pub(crate) mod regression;
mod script;
pub(crate) mod script_regression;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Pane {
    Editor,
    Main,
    Console,
    /// An extension enabled in `config.toml`, by its `<dll>/<pane>` name.
    Extension(String),
    Scope,
    Log,
    Reference,
    /// A `repl.ui` window, by its id; it lives while the registry has the id.
    Script(String),
    /// A `repl.ui.surface` canvas, by its id.
    Surface(String),
}

impl Pane {
    /// The tab and window title; a script window's comes from its definition.
    fn title(&self, windows: &[Window]) -> String {
        match self {
            Pane::Surface(id) => return id.clone(),
            Pane::Editor => "JavaScript",
            Pane::Main => crate::i18n::t("m001"),
            Pane::Console => "Console",
            Pane::Extension(name) => crate::extension::title(name),
            Pane::Scope => crate::i18n::t("pane_scope"),
            Pane::Log => crate::i18n::t("m008"),
            Pane::Reference => crate::i18n::t("pane_reference"),
            Pane::Script(id) => {
                return windows
                    .iter()
                    .find(|window| &window.id == id)
                    .map_or(id, |window| &window.title)
                    .clone()
            }
        }
        .into()
    }

    fn is_script(&self) -> bool {
        matches!(self, Self::Script(_) | Self::Surface(_))
    }
}

/// Where every pane is: in `tree`, or in the tree of an OS window (`floating`).
pub(super) struct Layout {
    pub tree: Tree<Pane>,
    /// The OS windows outside the main window, in the order they opened.
    pub floating: Vec<Float>,
    /// Windows opened since the last frame drew them; that frame brings each to the
    /// front once.
    pub fresh: Vec<egui::ViewportId>,
    /// The tabs container the user last clicked in; panes coming back join it.
    active: Option<TileId>,
    /// Where a window is asked to be: outer position, inner size.
    pub placed: HashMap<egui::ViewportId, egui::Rect>,
    /// Where a window was last drawn, in the same terms.
    pub seen: HashMap<egui::ViewportId, egui::Rect>,
    /// Windows asked to be on a monitor; they override `placed`.
    monitor: HashMap<egui::ViewportId, monitor::MonitorPlacement>,
    /// `monitor` on the monitors connected when the layout was read.
    pub resolved: HashMap<egui::ViewportId, [i32; 4]>,
    /// The main window's place in the source: `[main]` of `layout-auto.toml`, or
    /// `[window]` of `config.toml`.
    pub main: crate::config::Window,
    /// The source was switched: the next frame moves the main window to `main` and
    /// closes every OS window, so they open again where the new source puts them
    /// instead of staying where the other source put them.
    pub switched: bool,
    /// Frames left before the main window's place is taken as the user's again; until
    /// then it can still be where the other source put it.
    main_hold: u8,
    /// The extensions enabled in `config.toml`, in its order; each has a pane.
    pub extensions: Vec<String>,
}

/// An OS window outside the main window, with a tree of its own.
pub(super) struct Float {
    /// The viewport of the pane the window opened with, so a pane floated again gets
    /// its last place back.
    pub id: egui::ViewportId,
    pub tree: Tree<Pane>,
}

impl Float {
    /// A window of one pane.
    pub fn single(pane: Pane) -> Self {
        let mut tiles = Tiles::default();
        let id = pane.viewport();
        let pane = tiles.insert_pane(pane);
        let root = tiles.insert_tab_tile(vec![pane]);
        Self {
            id,
            tree: Tree::new(Self::tree_id(id), root, tiles),
        }
    }

    /// The id of the window's tree. `Id::unique` is a hash of its argument, so the
    /// main tree's id would give tiles of the same number the same widget id here.
    pub fn tree_id(id: egui::ViewportId) -> egui::Id {
        id.0.with("dock")
    }

    /// The window's panes in tree order.
    pub fn panes(&self) -> Vec<Pane> {
        fn walk(tiles: &Tiles<Pane>, id: TileId, panes: &mut Vec<Pane>) {
            match tiles.get(id) {
                Some(Tile::Pane(pane)) => panes.push(pane.clone()),
                Some(Tile::Container(container)) => {
                    for &child in container.children() {
                        walk(tiles, child, panes);
                    }
                }
                None => {}
            }
        }
        let mut panes = vec![];
        if let Some(root) = self.tree.root {
            walk(&self.tree.tiles, root, &mut panes);
        }
        panes
    }
}

/// Frames after a switch in which the main window may not have moved yet.
const MAIN_HOLD: u8 = 10;

impl Layout {
    /// Asks the main window to be at `main`; without a position it stays where it is.
    fn move_main(&mut self, ctx: &egui::Context) {
        let root = egui::ViewportId::ROOT;
        if let Some([x, y]) = self.main.position {
            ctx.send_viewport_cmd_to(root, egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        }
        let [width, height] = self.main.size;
        let size = egui::vec2(width, height);
        ctx.send_viewport_cmd_to(root, egui::ViewportCommand::InnerSize(size));
        self.main_hold = MAIN_HOLD;
    }

    /// Remembers where the main window is, in the terms of `seen`.
    fn see_main(&mut self, ctx: &egui::Context) {
        if self.main_hold > 0 {
            self.main_hold -= 1;
            return;
        }
        let viewport = ctx.input(|input| input.viewport().clone());
        if let (Some(outer), Some(inner)) = (viewport.outer_rect, viewport.inner_rect) {
            let rect = egui::Rect::from_min_size(outer.min, inner.size());
            self.seen.insert(egui::ViewportId::ROOT, rect);
        }
    }

    /// The panes of every OS window, window by window.
    pub fn floating_panes(&self) -> Vec<Pane> {
        self.floating.iter().flat_map(Float::panes).collect()
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            tree: Tree::empty(egui::Id::unique("dock")),
            floating: vec![],
            fresh: vec![],
            active: None,
            placed: HashMap::new(),
            seen: HashMap::new(),
            monitor: HashMap::new(),
            resolved: HashMap::new(),
            main: crate::config::Config::default().window,
            switched: false,
            main_hold: 0,
            extensions: vec![],
        }
    }
}

/// Editor on the left; the right column stacks main over the fixed-height sections, with
/// the console at the bottom right.
/// Shares are pixels (tab bar included) at the default 1250×800 window. The enabled
/// `extensions`, in order under main, and scope get their whole drawing; log and
/// reference start scrolled.
pub(super) fn initial(extensions: &[String]) -> Layout {
    let mut tiles = Tiles::default();
    let tab = |tiles: &mut Tiles<Pane>, pane| {
        let pane = tiles.insert_pane(pane);
        tiles.insert_tab_tile(vec![pane])
    };
    let sections = extensions
        .iter()
        .map(|name| (Pane::Extension(name.clone()), 126.0));
    let column: Vec<_> = [(Pane::Main, 250.0)]
        .into_iter()
        .chain(sections)
        .chain([
            (Pane::Scope, 144.0),
            (Pane::Log, 96.0),
            (Pane::Reference, 84.0),
            (Pane::Console, 98.0),
        ])
        .map(|(pane, share)| (tab(&mut tiles, pane), share))
        .collect();
    let editor = tab(&mut tiles, Pane::Editor);
    let right = tiles.insert_vertical_tile(column.iter().map(|(id, _)| *id).collect());
    share(&mut tiles, right, &column);
    let root = tiles.insert_horizontal_tile(vec![editor, right]);
    share(&mut tiles, root, &[(editor, 630.0), (right, 620.0)]);
    Layout {
        tree: Tree::new(egui::Id::unique("dock"), root, tiles),
        extensions: extensions.to_vec(),
        ..Layout::default()
    }
}

fn share(tiles: &mut Tiles<Pane>, linear: TileId, shares: &[(TileId, f32)]) {
    if let Some(Tile::Container(Container::Linear(linear))) = tiles.get_mut(linear) {
        for &(child, share) in shares {
            linear.shares.set_share(child, share);
        }
    }
}

/// Borrows the editor for one frame of `Tree::ui` and the floating windows.
pub(super) struct Panes<'a> {
    pub editor: &'a mut Editor,
    pub preparing: bool,
    pub notes: Vec<u8>,
    pub projected: Option<View>,
    /// The registered `repl.ui` windows at the start of the frame.
    pub windows: Vec<Window>,
    /// The pane tile clicked this frame (tab or contents).
    touched: Option<TileId>,
    /// The pane tile whose tab menu asked for its own window.
    float: Option<TileId>,
    /// While an OS window's tree is drawn: its clicks and tab menus are not the main
    /// tree's.
    in_window: bool,
}

impl egui_tiles::Behavior<Pane> for Panes<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, tile: TileId, pane: &mut Pane) -> UiResponse {
        if !self.in_window
            && ui.input(|input| input.pointer.any_pressed())
            && ui.rect_contains_pointer(ui.max_rect())
        {
            self.touched = Some(tile);
        }
        let frame = if matches!(pane, Pane::Surface(_)) {
            egui::Frame::NONE
        } else {
            egui::Frame::central_panel(ui.style())
        };
        frame.show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            self.contents(ui, pane);
        });
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title(&self.windows).into()
    }

    fn is_tab_closable(&self, tiles: &Tiles<Pane>, tile: TileId) -> bool {
        matches!(tiles.get(tile), Some(Tile::Pane(pane)) if pane.is_script())
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<Pane>, tile: TileId) -> bool {
        if let Some(Tile::Pane(pane)) = tiles.get(tile) {
            self.close(pane);
        }
        true
    }

    fn on_tab_button(
        &mut self,
        _: &mut Tiles<Pane>,
        tile: TileId,
        response: egui::Response,
    ) -> egui::Response {
        if self.in_window {
            return response;
        }
        if response.clicked() {
            self.touched = Some(tile);
        }
        response.context_menu(|ui| {
            if ui.button(crate::i18n::t("pane_float")).clicked() {
                self.float = Some(tile);
            }
        });
        response
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

impl Panes<'_> {
    fn contents(&mut self, ui: &mut egui::Ui, pane: &Pane) {
        let preparing = self.preparing;
        match pane {
            Pane::Script(id) => self.script(ui, id),
            Pane::Surface(id) => self.surface(ui, id),
            // Not disabled while a run is pending: results stay selectable.
            Pane::Console => super::console::show(
                ui,
                self.editor
                    .control
                    .as_ref()
                    .map_or(&[][..], |c| &c.results[..]),
            ),
            Pane::Editor => self.editor.editor_pane(ui, preparing),
            Pane::Main => self
                .editor
                .main_pane(ui, preparing, self.projected.as_ref()),
            _ => {
                if preparing {
                    ui.disable();
                }
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| self.section(ui, pane));
            }
        }
    }

    fn section(&mut self, ui: &mut egui::Ui, pane: &Pane) {
        match pane {
            Pane::Extension(name) => {
                let frame = repl_extension_api::Frame {
                    active_notes: &self.notes,
                    range: self.projected.as_ref().and_then(View::pitch_range),
                    language: crate::i18n::language(),
                };
                self.editor.extensions.show(name, ui, &frame);
            }
            Pane::Scope => self.editor.scope_pane(ui),
            Pane::Log => self.editor.log_pane(ui),
            Pane::Reference => self.editor.reference_pane(ui),
            Pane::Editor | Pane::Main | Pane::Console | Pane::Script(_) | Pane::Surface(_) => {}
        }
    }
}

impl Editor {
    /// Every pane, in the tree or in its own window; `preparing` disables the editor and
    /// the sections.
    pub(super) fn dock(&mut self, ui: &mut egui::Ui, preparing: bool) {
        let notes = self
            .control
            .as_ref()
            .filter(|control| control.ready)
            .and_then(|control| control.read_notes().ok())
            .unwrap_or_default();
        let projected = self
            .timeline
            .as_ref()
            .map(crate::timeline::projection::project);
        let ctx = ui.ctx().clone();
        let windows = crate::script_ui::snapshot();
        self.script_ui.retain(&windows);
        let mut layout = std::mem::take(&mut self.layout);
        layout.sync(&windows);
        let mut panes = Panes {
            editor: self,
            preparing,
            notes,
            projected,
            windows,
            touched: None,
            float: None,
            in_window: false,
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| layout.tree.ui(&mut panes, ui));
        let back = if std::mem::take(&mut layout.switched) {
            layout.move_main(&ctx);
            vec![]
        } else {
            let fresh = std::mem::take(&mut layout.fresh);
            panes.windows(&ctx, &mut layout, &fresh)
        };
        layout.see_main(&ctx);
        let (touched, float) = (panes.touched, panes.float);
        if let Some(tile) = touched {
            layout.touch(tile);
        }
        if let Some(tile) = float {
            layout.float(tile);
        }
        for id in back {
            layout.dock_window(id);
        }
        crate::script_ui::surface::finish_frame();
        if let Some(file) = &mut self.layout_file {
            file.poll(&mut layout);
        }
        self.layout = layout;
    }
}

#[cfg(test)]
mod tests;
