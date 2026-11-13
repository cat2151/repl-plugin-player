//! `repl.ui` windows as panes: the registry decides which exist, the layout where.
use super::{Float, Layout, Pane, Panes, Window};
use eframe::egui;
use egui_tiles::{Tile, Tree};

impl Layout {
    /// Drops the script panes whose window is gone and puts a newly shown window in its
    /// own OS window. A window shown again keeps its place.
    pub(super) fn sync(&mut self, windows: &[Window]) {
        let shown = |pane: &Pane| match pane {
            Pane::Script(id) => windows.iter().any(|window| &window.id == id),
            _ => true,
        };
        let drop = |tree: &mut Tree<Pane>| {
            let gone: Vec<_> = (tree.tiles.iter())
                .filter(|(_, tile)| matches!(tile, Tile::Pane(pane) if !shown(pane)))
                .map(|(id, _)| *id)
                .collect();
            for tile in gone {
                tree.remove_recursively(tile);
            }
        };
        drop(&mut self.tree);
        for float in &mut self.floating {
            drop(&mut float.tree);
        }
        self.floating.retain(|float| !float.panes().is_empty());
        let floating = self.floating_panes();
        for window in windows {
            let pane = Pane::Script(window.id.clone());
            if !floating.contains(&pane) && self.tree.tiles.find_pane(&pane).is_none() {
                let float = Float::single(pane);
                self.fresh.push(float.id);
                self.floating.push(float);
            }
        }
    }
}

impl Panes<'_> {
    /// The widgets of a script window, while the engine can answer them.
    pub(super) fn script(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(window) = self.windows.iter().find(|window| window.id == id) else {
            return;
        };
        let editor = &mut *self.editor;
        let control = editor
            .control
            .as_mut()
            .filter(|control| control.ready && !control.stopping && !self.preparing);
        if let Some(control) = control {
            let renderer = &mut editor.script_ui;
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| renderer.contents(ui, window, control, |_, _| {}));
        }
    }

    /// Removes a script pane's window from the registry; the next frame's `sync` drops
    /// the pane.
    pub(super) fn close(&self, pane: &Pane) {
        if let Pane::Script(id) = pane {
            if let Some(window) = self.windows.iter().find(|window| &window.id == id) {
                crate::script_ui::close(window);
            }
        }
    }
}
