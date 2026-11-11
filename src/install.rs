//! GUI-owned consent and worker result. Closing a dialog never drops a running job.
use crate::i18n::{render, t};
use eframe::egui;
use std::{
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Instant,
};

pub const COMMAND: &[&str] = &[
    "install",
    "--id",
    "SurgeSynth.SurgeXT",
    "-e",
    "--source",
    "winget",
    "--silent",
    "--disable-interactivity",
    "--accept-package-agreements",
    "--accept-source-agreements",
];
pub const MANUAL: &str = "https://surge-synthesizer.github.io/";
const LICENSE: &str = "https://github.com/surge-synthesizer/surge/blob/main/LICENSE";

#[derive(Debug, PartialEq)]
pub enum Failure {
    Missing,
    Cancelled,
    Exit(String),
}

#[derive(Default)]
pub struct Installer {
    open: bool,
    offered: bool,
    job: Option<Receiver<Result<(), Failure>>>,
    since: Option<Instant>,
    failure: Option<Failure>,
    checking: bool,
    installed_missing: bool,
}

fn run() -> Result<(), Failure> {
    use std::os::windows::process::CommandExt;
    let result = Command::new("winget")
        .args(COMMAND)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Failure::Missing
            } else {
                Failure::Exit(error.to_string())
            }
        })?;
    match result.status.code().map(|n| n as u32) {
        Some(0) => Ok(()),
        Some(1223 | 0x800704c7 | 0x8a15010c) => Err(Failure::Cancelled),
        code => Err(Failure::Exit(format!(
            "{:08X?}\n{}\n{}",
            code,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ))),
    }
}

impl Installer {
    fn accept(&mut self) {
        self.accept_with(run);
    }

    fn accept_with(&mut self, worker: impl FnOnce() -> Result<(), Failure> + Send + 'static) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(worker());
        });
        self.job = Some(rx);
        self.since = Some(Instant::now());
        self.failure = None;
        self.installed_missing = false;
    }

    fn poll(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let result = match job.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(Failure::Exit(t("install_worker_lost").into()))
            }
        };
        self.job = None;
        match result {
            Ok(()) => {
                self.checking = true;
                self.open = false;
                true
            }
            Err(error) => {
                self.failure = Some(error);
                self.open = true;
                false
            }
        }
    }

    /// True requests a fresh full scan. Polled even when the dialog is hidden.
    pub fn show(&mut self, ui: &mut egui::Ui, missing: bool) -> bool {
        let mut rescan = self.poll();
        if rescan || !missing {
            return rescan;
        }
        if self.checking {
            self.checking = false;
            self.installed_missing = true;
            self.open = true;
        }
        if !self.offered {
            self.open = true;
            self.offered = true;
        }
        ui.label(t("install_missing"));
        if ui.button(t("install_show")).clicked() {
            self.open = true;
        }
        if !self.open {
            return rescan;
        }
        let response = egui::Modal::new(egui::Id::unique("surge_install")).show(ui.ctx(), |ui| {
            ui.set_max_width(540.0);
            ui.heading(t("install_heading"));
            if self.job.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t("install_progress"));
                });
                ui.label(render(
                    "install_elapsed",
                    &[(
                        "seconds",
                        self.since.unwrap().elapsed().as_secs().to_string(),
                    )],
                ));
                ui.label(t("install_uac"));
            } else {
                if self.installed_missing {
                    ui.colored_label(ui.visuals().error_fg_color, t("install_still_missing"));
                }
                if let Some(error) = &self.failure {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        match error {
                            Failure::Missing => t("install_winget_missing"),
                            Failure::Cancelled => t("install_cancelled"),
                            Failure::Exit(_) => t("install_failed"),
                        },
                    );
                    if let Failure::Exit(detail) = error {
                        ui.label(detail);
                    }
                }
                ui.label(t("install_body"));
                ui.hyperlink_to(t("install_terms"), LICENSE);
                if ui
                    .button(t("install_accept"))
                    .on_hover_text(format!("winget {}", COMMAND.join(" ")))
                    .clicked()
                {
                    self.accept();
                }
                ui.hyperlink_to(t("install_manual"), MANUAL);
                if ui.button(t("install_rescan")).clicked() {
                    self.open = false;
                    rescan = true;
                }
            }
            if ui.button(t("install_later")).clicked() {
                self.open = false;
            }
        });
        if response.should_close() {
            self.open = false;
        }
        rescan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &egui::Context, installer: &mut Installer, missing: bool, escape: bool) -> bool {
        let mut rescan = false;
        ctx.run_ui(
            egui::RawInput {
                events: if escape {
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                rescan |= installer.show(ui, missing);
            },
        )
        .drop_without_applying_deltas();
        rescan
    }

    #[test]
    fn offer_escape_does_not_start_worker_and_install_is_not_duplicated() {
        let ctx = egui::Context::default();
        let mut installer = Installer::default();
        frame(&ctx, &mut installer, true, false);
        frame(&ctx, &mut installer, true, true);
        assert!(!installer.open);
        assert!(installer.job.is_none());
        let (started, received) = mpsc::channel();
        let (finish, wait) = mpsc::channel();
        installer.accept_with(move || {
            started.send(()).unwrap();
            wait.recv().unwrap();
            Ok(())
        });
        received.recv().unwrap();
        installer.accept_with(|| panic!("second installer must not start"));
        frame(&ctx, &mut installer, true, true);
        assert!(installer.job.is_some());
        finish.send(()).unwrap();
        // Wait on the actual worker receiver without polling/sleeping.
        let result = installer.job.as_ref().unwrap().recv().unwrap();
        assert!(result.is_ok());
        installer.job = None;
    }

    #[test]
    fn successful_job_requests_scan_before_classifying_missing_again() {
        let ctx = egui::Context::default();
        let mut installer = Installer::default();
        let (tx, rx) = mpsc::channel();
        installer.job = Some(rx);
        tx.send(Ok(())).unwrap();
        assert!(frame(&ctx, &mut installer, true, false));
        assert!(!installer.installed_missing);
        assert!(!frame(&ctx, &mut installer, false, false));
        assert!(!frame(&ctx, &mut installer, true, false));
        assert!(installer.installed_missing);
        assert!(!installer.checking);
        assert!(installer.job.is_none());
    }
    #[test]
    fn consent_dismissal_keeps_job_and_success_rescans_once() {
        let mut installer = Installer::default();
        assert!(installer.job.is_none()); // Showing/dismissing is not consent.
        let (tx, rx) = mpsc::channel();
        installer.job = Some(rx);
        installer.open = false;
        tx.send(Ok(())).unwrap();
        assert!(installer.poll());
        assert!(!installer.poll());
        assert!(installer.checking);
        assert!(installer.job.is_none());
    }
    #[test]
    fn errors_are_recovered_even_when_dialog_was_closed() {
        for error in [
            Failure::Missing,
            Failure::Cancelled,
            Failure::Exit("42".into()),
        ] {
            let mut installer = Installer::default();
            let (tx, rx) = mpsc::channel();
            installer.job = Some(rx);
            tx.send(Err(error)).unwrap();
            assert!(!installer.poll());
            assert!(installer.failure.is_some());
            assert!(installer.open);
            assert!(installer.job.is_none());
        }
        assert!(COMMAND.contains(&"--accept-package-agreements"));
    }
}
