use eframe::egui;

#[derive(Default)]
pub(super) struct Startup {
    presented_frame: Option<u64>,
    complete: bool,
    failed: bool,
    focused: bool,
}

impl Startup {
    pub fn can_initialize(&self, ctx: &egui::Context) -> bool {
        self.presented_frame
            .is_some_and(|frame| ctx.cumulative_frame_nr() > frame)
    }

    pub fn update(&mut self, ready: bool, failed: bool) {
        self.complete |= ready;
        self.failed = failed;
    }

    pub fn is_preparing(&self) -> bool {
        !self.complete && !self.failed
    }

    pub fn focus_editor(&mut self, ui: &egui::Ui) {
        let id = egui::Id::unique("script_text");
        if self.is_preparing() {
            ui.memory_mut(|memory| memory.surrender_focus(id));
            return;
        }
        // A modal also blocks the frame immediately after it disappears.
        // Wait until the editor's layer accepts focus, then request it once.
        if self.complete
            && !self.focused
            && ui.is_enabled()
            && ui.memory(|memory| memory.allows_interaction(ui.layer_id()))
        {
            ui.memory_mut(|memory| memory.request_focus(id));
            self.focused = true;
        }
    }

    pub fn show_overlay(&mut self, ctx: &egui::Context) {
        if self.is_preparing() {
            let id = egui::Id::unique("uapmd_startup");
            let response = egui::Modal::new(id)
                .area(egui::Modal::default_area(id).fade_in(false))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.heading(crate::i18n::t("m010"));
                    });
                    ui.label(crate::i18n::t("m011"));
                    ui.is_visible()
                });
            // The Area's first pass only measures its size and is invisible.
            if response.inner {
                self.presented_frame
                    .get_or_insert(ctx.cumulative_frame_nr());
            }
        }
    }
}

#[cfg(test)]
mod tests;
