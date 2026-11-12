use crate::execution::Request;
use eframe::egui;

/// Native JS evaluation must stay on the control thread. Let egui present a
/// visible modal in an earlier frame before entering that synchronous call.
#[derive(Default)]
pub(super) struct Pending {
    request: Option<Request>,
    presented_frame: Option<u64>,
    evaluated: bool,
}

impl Pending {
    pub fn queue(&mut self, request: Request) {
        self.request = Some(request);
        self.presented_frame = None;
    }

    /// Snippets, and the first evaluation of any scope, which loads plugins
    /// and can block for seconds.
    pub fn presents(&self, request: &Request) -> bool {
        request.scope == "snippet" || !self.evaluated
    }

    pub fn is_pending(&self) -> bool {
        self.request.is_some()
    }

    pub fn take_ready(&mut self, ctx: &egui::Context) -> Option<Request> {
        if !self
            .presented_frame
            .is_some_and(|frame| ctx.cumulative_frame_nr() > frame)
        {
            return None;
        }
        self.presented_frame = None;
        self.evaluated |= self.request.is_some();
        self.request.take()
    }

    pub fn show_overlay(&mut self, ctx: &egui::Context) {
        if !self.is_pending() {
            return;
        }
        let id = egui::Id::unique("snippet_execution");
        let response = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).fade_in(false))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(28.0));
                    ui.add_space(8.0);
                    ui.heading(crate::i18n::t("execution_running"));
                });
                ui.is_visible()
            });
        // A new Area can have an invisible sizing pass. Do not evaluate yet.
        if response.inner {
            self.presented_frame
                .get_or_insert(ctx.cumulative_frame_nr());
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests;
