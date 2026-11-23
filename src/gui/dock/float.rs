//! Panes outside the main tree, in the trees of OS windows; each window is an immediate
//! viewport (an OS window under eframe; an embedded `egui::Window` in a headless
//! `Context`).
use super::{Float, Layout, Pane, Panes, Window};
use eframe::egui;
use egui_tiles::{Container, Tile, TileId};

impl Pane {
    pub(super) fn viewport(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("dock", self))
    }

    /// Inner size of the pane's own window, near its size in the first layout; a
    /// script window gets a row for its button and one per widget.
    fn window_size(&self, windows: &[Window]) -> egui::Vec2 {
        let height = match self {
            Pane::Editor => 760.0,
            Pane::Main => 400.0,
            Pane::Console => 220.0,
            Pane::Extension(_) => 150.0,
            Pane::Scope => 170.0,
            Pane::Log => 200.0,
            Pane::Reference => 300.0,
            Pane::Surface(id) => {
                return crate::script_ui::surface::size(id).unwrap_or(egui::vec2(620.0, 600.0));
            }
            Pane::Script(id) => {
                let widgets = windows
                    .iter()
                    .find(|window| &window.id == id)
                    .map_or(0, |window| window.widgets.len());
                (40.0 + 24.0 * (widgets + 1) as f32).min(600.0)
            }
        };
        let width = match self {
            Pane::Editor => 630.0,
            Pane::Script(_) => 420.0,
            _ => 620.0,
        };
        egui::vec2(width, height)
    }
}

impl Layout {
    /// Remembers the tabs container holding the pane tile the user clicked.
    pub(super) fn touch(&mut self, tile: TileId) {
        self.active = self.tree.tiles.parent_of(tile);
    }

    /// Moves the pane of `tile` out of the tree into its own window.
    pub(super) fn float(&mut self, tile: TileId) {
        if let Some(Tile::Pane(pane)) = self.tree.tiles.get(tile) {
            let float = Float::single(pane.clone());
            self.tree.remove_recursively(tile);
            self.fresh.push(float.id);
            self.floating.push(float);
        }
    }

    /// Puts every pane of window `id` back in the tree; the window's last place is kept
    /// for the next window with its id.
    pub(super) fn dock_window(&mut self, id: egui::ViewportId) {
        let Some(index) = self.floating.iter().position(|float| float.id == id) else {
            return;
        };
        let float = self.floating.remove(index);
        if let Some(rect) = self.seen.remove(&id) {
            self.placed.insert(id, rect);
        }
        for pane in float.panes() {
            self.dock(pane);
        }
    }

    /// Puts a pane as the active tab of the active tabs container.
    pub(super) fn dock(&mut self, pane: Pane) {
        let target = self.target();
        let tiles = &mut self.tree.tiles;
        let child = tiles.insert_pane(pane);
        if let Some(Tile::Container(Container::Tabs(tabs))) = target.and_then(|t| tiles.get_mut(t))
        {
            tabs.add_child(child);
            tabs.set_active(child);
            return;
        }
        let children = self.tree.root.into_iter().chain([child]).collect();
        let root = tiles.insert_tab_tile(children);
        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(child);
        }
        self.tree.root = Some(root);
    }

    /// The last clicked tabs container if it is still there, else main's, else any.
    pub(super) fn target(&self) -> Option<TileId> {
        let tiles = &self.tree.tiles;
        let tabs =
            |id: &TileId| matches!(tiles.get(*id), Some(Tile::Container(Container::Tabs(_))));
        let main = tiles
            .find_pane(&Pane::Main)
            .and_then(|main| tiles.parent_of(main));
        self.active
            .filter(tabs)
            .or(main.filter(tabs))
            .or_else(|| tiles.iter().map(|(id, _)| *id).find(tabs))
    }
}

impl Float {
    /// The window title: its panes' titles.
    fn title(&self, windows: &[Window]) -> String {
        let titles: Vec<_> = (self.panes().iter())
            .map(|pane| pane.title(windows))
            .collect();
        titles.join(" / ")
    }

    /// Inner size of a new window: its pane's, or one that fits a few panes.
    fn size(&self, windows: &[Window]) -> egui::Vec2 {
        match &self.panes()[..] {
            [pane] => pane.window_size(windows),
            _ => egui::vec2(620.0, 600.0),
        }
    }
}

impl Panes<'_> {
    /// Draws every OS window with its tree and returns the windows asked back into the
    /// main tree (its button, or the OS window's close). The OS window's close closes
    /// its script windows instead. The `fresh` windows are brought to the front: a
    /// window opened by a cell run from the keyboard otherwise opens behind the main
    /// window.
    pub(super) fn windows(
        &mut self,
        ctx: &egui::Context,
        layout: &mut Layout,
        fresh: &[egui::ViewportId],
    ) -> Vec<egui::ViewportId> {
        let mut back = vec![];
        let mut titles = vec![];
        let Layout {
            floating,
            placed,
            seen: seen_rects,
            resolved,
            ..
        } = layout;
        for float in floating.iter_mut() {
            let id = float.id;
            if fresh.contains(&id) {
                ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
            }
            let title = float.title(&self.windows);
            let placed = placed.get(&id);
            let size = placed.map_or_else(|| float.size(&self.windows), |r| r.size());
            let mut builder = egui::ViewportBuilder::default()
                .with_title(title.clone())
                .with_inner_size(size);
            if let Some(rect) = placed {
                builder = builder.with_position(rect.min);
            } else if matches!(&float.panes()[..], [Pane::Surface(_)])
                && ctx.input(|input| {
                    input
                        .viewport()
                        .monitor_size
                        .is_some_and(|monitor| size.x >= monitor.x || size.y >= monitor.y)
                })
            {
                builder = builder.with_maximized(true);
            }
            let outer = resolved.get(&id).copied();
            let mut seen = None;
            let mut closed = false;
            self.in_window = true;
            let docked = ctx.show_viewport_immediate(id, builder, |ui, class| {
                let mut docked = false;
                let mut contents = |ui: &mut egui::Ui| {
                    docked = ui.button(crate::i18n::t("pane_dock")).clicked();
                    float.tree.ui(self, ui);
                };
                if class == egui::ViewportClass::EmbeddedWindow {
                    contents(ui);
                } else {
                    egui::CentralPanel::default().show(ui, contents);
                    if let Some(outer) = outer {
                        super::monitor::apply(ui.ctx(), &title, outer);
                    }
                    let viewport = ui.input(|input| input.viewport().clone());
                    closed = viewport.close_requested();
                    if let (Some(outer), Some(inner)) = (viewport.outer_rect, viewport.inner_rect) {
                        seen = Some(egui::Rect::from_min_size(outer.min, inner.size()));
                    }
                }
                docked
            });
            self.in_window = false;
            titles.push(title);
            if let Some(rect) = seen {
                seen_rects.insert(id, rect);
            }
            let panes = float.panes();
            if closed {
                panes
                    .iter()
                    .filter(|pane| pane.is_script())
                    .for_each(|p| self.close(p));
            }
            if docked || (closed && !panes.iter().all(Pane::is_script)) {
                back.push(id);
            }
        }
        super::owner::own(super::super::TITLE, &titles);
        back
    }
}
