use crate::{
    execution::{self, Scope},
    snippets::{Library, Snippet},
};
use eframe::egui;

#[derive(Default)]
pub struct SnippetsPanel {
    pub library: Library,
    open: bool,
    entries: Vec<Snippet>,
    filter: String,
    name: String,
    error: String,
    notice: String,
    highlight: crate::syntax_highlight::JavaScriptHighlight,
    preview: Option<usize>,
}

#[cfg(test)]
#[path = "snippets_ui_tests.rs"]
mod tests;

impl SnippetsPanel {
    pub fn initial(&mut self) -> String {
        match self.library.initial() {
            Ok(code) => code,
            Err(error) => {
                self.error = error;
                String::new()
            }
        }
    }

    fn refresh(&mut self) {
        self.preview = None;
        match self.library.load() {
            Ok(entries) => {
                self.entries = entries;
                self.error.clear();
            }
            Err(error) => {
                self.entries.clear();
                self.error = error;
            }
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        code: &str,
        caret: usize,
        ready: bool,
    ) -> Option<Snippet> {
        if ui.button(crate::i18n::t("m022")).clicked() {
            self.open = true;
            self.refresh();
        }
        if !self.error.is_empty() && !self.open {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        let mut open = self.open;
        let mut chosen = None;
        let viewport = ui.ctx().content_rect();
        let width = viewport.width().min(960.0);
        egui::Window::new(crate::i18n::t("m022"))
            .id(egui::Id::unique("snippet_window"))
            .open(&mut open)
            .fixed_rect(egui::Rect::from_min_size(
                egui::pos2(viewport.center().x - width / 2.0, viewport.top()),
                egui::vec2(width, viewport.height()),
            ))
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("m024"));
                    ui.text_edit_singleline(&mut self.filter);
                    if ui.button(crate::i18n::t("m025")).clicked() {
                        self.refresh();
                    }
                });
                if !ready {
                    ui.weak(crate::i18n::t("m026"));
                }
                egui::Panel::bottom("snippets_footer")
                    .frame(egui::Frame::NONE)
                    .resizable(false)
                    .show(ui, |ui| self.footer(ui, code, caret));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        chosen = self.browser(ui, ready);
                    });
            });
        self.open = open && chosen.is_none();
        chosen
    }

    fn footer(&mut self, ui: &mut egui::Ui, code: &str, caret: usize) {
        ui.separator();
        ui.label(crate::i18n::t("m027"));
        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("m028"));
            ui.text_edit_singleline(&mut self.name);
            if ui.button(crate::i18n::t("m029")).clicked() {
                let cell = execution::request(code, caret, caret, Scope::Cell);
                match self.library.save(&self.name, &cell.code) {
                    Ok(path) => {
                        self.notice =
                            crate::i18n::render("f012", &[("arg0", format!("{}", path.display()))]);
                        self.name.clear();
                        self.refresh();
                    }
                    Err(error) => self.error = error,
                }
            }
        });
        if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        egui::CollapsingHeader::new(crate::i18n::t("m030"))
            .id_salt("m030")
            .show(ui, |ui| {
                ui.label(crate::i18n::render(
                    "f013",
                    &[("arg0", format!("{}", self.library.builtin.display()))],
                ));
                if let Some(path) = &self.library.user {
                    ui.label(crate::i18n::render(
                        "f014",
                        &[("arg0", format!("{}", path.display()))],
                    ));
                }
                ui.label(crate::i18n::t("m031"));
                ui.label(crate::i18n::t("m032"));
            });
    }

    fn matches_filter(snippet: &Snippet, filter: &str) -> bool {
        format!(
            "{} {} {} {}",
            snippet.group,
            snippet.name,
            crate::i18n::group(snippet.group),
            if snippet.group == "Built-in" {
                crate::i18n::snippet_name(&snippet.name)
            } else {
                &snippet.name
            },
        )
        .to_lowercase()
        .contains(filter)
    }

    fn browser(&mut self, ui: &mut egui::Ui, ready: bool) -> Option<Snippet> {
        let filter = self.filter.to_lowercase();
        if self.preview.is_some_and(|index| {
            self.entries
                .get(index)
                .is_none_or(|snippet| !Self::matches_filter(snippet, &filter))
        }) {
            self.preview = None;
        }
        let mut chosen = None;
        let height = ui.available_height();
        ui.columns(2, |columns| {
            for column in columns.iter_mut() {
                column.set_clip_rect(column.clip_rect().intersect(column.max_rect()));
            }
            egui::ScrollArea::vertical()
                .id_salt("snippets_list")
                .max_height(height)
                .auto_shrink([false, false])
                .show(&mut columns[0], |ui| {
                    for (index, snippet) in self.entries.iter().enumerate() {
                        if !Self::matches_filter(snippet, &filter) {
                            continue;
                        }
                        let name = if snippet.group == "Built-in" {
                            crate::i18n::snippet_name(&snippet.name)
                        } else {
                            &snippet.name
                        };
                        let response = ui.add_enabled(
                            ready && !snippet.code.trim().is_empty(),
                            egui::Button::new(format!(
                                "{} · {}   | {}",
                                crate::i18n::group(snippet.group),
                                name,
                                crate::i18n::t("insert_run")
                            ))
                            .wrap()
                            .selected(self.preview == Some(index)),
                        );
                        if response.contains_pointer() && self.preview != Some(index) {
                            self.preview = Some(index);
                            ui.ctx().request_repaint();
                        }
                        if response.clicked() {
                            chosen = Some(snippet.clone());
                        }
                    }
                });
            let ui = &mut columns[1];
            egui::Frame::new()
                .fill(crate::syntax_highlight::BACKGROUND)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .id_salt(("snippet_preview", self.preview))
                        .max_height((height - 12.0).max(0.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(snippet) = self.preview.and_then(|i| self.entries.get(i)) {
                                ui.label(format!("{}.js", snippet.name));
                                let job = self.highlight.layout(
                                    &snippet.code,
                                    egui::TextStyle::Monospace.resolve(ui.style()),
                                );
                                ui.add(egui::Label::new(job).wrap());
                            }
                        });
                });
        });
        chosen
    }
}
