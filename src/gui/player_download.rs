//! The overlay offered after a call needed `cat-plugin-player.exe` and found none.
use crate::i18n::t;
use eframe::egui;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
};

enum State {
    Offered,
    Downloading(Receiver<Result<(), String>>),
    Failed(String),
}

#[derive(Default)]
pub(super) struct Offer {
    state: Option<State>,
}

impl Offer {
    pub fn update(&mut self) {
        if crate::player_download::take_missed() && self.state.is_none() {
            self.state = Some(State::Offered);
        }
        let Some(State::Downloading(job)) = &self.state else {
            return;
        };
        self.state = match job.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(State::Failed(error)),
            Err(TryRecvError::Disconnected) => Some(State::Failed(t("install_worker_lost").into())),
        };
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let Some(state) = &self.state else {
            return;
        };
        let downloading = matches!(state, State::Downloading(_));
        let mut start = false;
        let response = egui::Modal::new(egui::Id::unique("player_download")).show(ctx, |ui| {
            ui.heading("cat-plugin-player.exe");
            if downloading {
                ui.add(egui::Spinner::new().size(28.0));
                return;
            }
            if let State::Failed(error) = state {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            start = ui.button(t("player_download")).clicked();
        });
        if start {
            self.start(beside_this_exe());
        } else if response.should_close() && !downloading {
            self.state = None;
        }
    }

    fn start(&mut self, directory: Result<PathBuf, String>) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(directory.and_then(|d| crate::player_download::install(&d)));
        });
        self.state = Some(State::Downloading(rx));
    }
}

fn beside_this_exe() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(executable
        .parent()
        .ok_or("exe has no directory")?
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_miss_offers_once_and_a_failed_download_offers_again() {
        let mut offer = Offer::default();
        crate::player_download::missed();
        offer.update();
        assert!(matches!(offer.state, Some(State::Offered)));
        offer.start(Err("offline".into()));
        while matches!(offer.state, Some(State::Downloading(_))) {
            offer.update();
        }
        assert!(matches!(&offer.state, Some(State::Failed(e)) if e == "offline"));
    }
}
