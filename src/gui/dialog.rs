//! The dialogs of `repl.ui.dialog`, one at a time in the order given. Once
//! `close_after` is called, the window closes when none is left.
//! A dialog takes the keyboard focus so that Enter presses OK instead of
//! reaching the code editor under it, and gives it back when it closes.
use crate::{i18n::t, script_ui::Dialog};
use eframe::egui;

#[derive(Default)]
pub(super) struct Dialogs {
    current: Option<Dialog>,
    close_after: bool,
    /// Some while open: the widget that had the focus before.
    returning_focus: Option<Option<egui::Id>>,
}

impl Dialogs {
    pub fn close_after(&mut self) {
        self.close_after = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.current.is_none() {
            self.current = crate::script_ui::take_dialog();
        }
        let Some(dialog) = &self.current else {
            if self.close_after {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        };
        let mut ok = false;
        let response = egui::Modal::new(egui::Id::unique("script_dialog")).show(ctx, |ui| {
            ui.heading(&dialog.title);
            ui.label(&dialog.text);
            let button = ui.button(t("dialog_ok"));
            if self.returning_focus.is_none() {
                self.returning_focus = Some(ui.memory(|m| m.focused()));
                button.request_focus();
            }
            ok = button.clicked();
        });
        if ok || response.should_close() {
            self.current = None;
            if let Some(Some(id)) = self.returning_focus.take() {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, RawInput};

    #[test]
    fn enter_closes_the_dialog_and_the_code_gets_the_focus_back() {
        let ctx = egui::Context::default();
        let code_id = egui::Id::unique("code");
        let mut code = String::from("a");
        let mut dialogs = Dialogs::default();
        let frame = |dialogs: &mut Dialogs, code: &mut String, events: Vec<Event>| {
            let output = ctx.run_ui(
                RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(code).id(code_id));
                    });
                    dialogs.show(ui.ctx());
                },
            );
            output.drop_without_applying_deltas();
        };
        ctx.memory_mut(|memory| memory.request_focus(code_id));
        frame(&mut dialogs, &mut code, vec![]);
        dialogs.current = Some(Dialog {
            title: "title".into(),
            text: "dialog".into(),
        });
        frame(&mut dialogs, &mut code, vec![]);
        let enter = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        frame(&mut dialogs, &mut code, vec![enter]);
        assert!(dialogs.current.is_none(), "Enter must close the dialog");
        assert_eq!(code, "a", "Enter must not reach the code under the dialog");
        frame(&mut dialogs, &mut code, vec![Event::Text("b".into())]);
        assert_eq!(code, "ab", "the code must have the focus back");
    }

    #[test]
    fn close_after_waits_until_no_dialog_is_left() {
        let ctx = egui::Context::default();
        let mut dialogs = Dialogs {
            current: Some(Dialog {
                title: "title".into(),
                text: "dialog".into(),
            }),
            ..Default::default()
        };
        dialogs.close_after();
        let closes = |dialogs: &mut Dialogs| {
            let output = ctx.run_ui(RawInput::default(), |ui| dialogs.show(ui.ctx()));
            output
                .viewport_output
                .values()
                .any(|viewport| viewport.commands.contains(&egui::ViewportCommand::Close))
        };
        assert!(!closes(&mut dialogs), "an open dialog holds the close");
        dialogs.current = None;
        assert!(closes(&mut dialogs), "the close follows the last dialog");
    }
}
