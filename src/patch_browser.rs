//! The app's one resident cat-plugin-player, used only for its patch and effect
//! browsers: started by the first request, then only asked to show a browser again.
//! Dropping the link (or ending this process) closes the pipes, which ends the
//! player.
use crate::i18n::{render, t};
use crate::resident_ipc::{self, Listener, Open, Selected, Sender, ToEditor, ToPlayer};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

const EXECUTABLE: &str = "cat-plugin-player.exe";

/// Which of the player's browsers is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Patch,
    Effect,
}

impl Kind {
    /// The player keeps its own track list, so one target per browser suffices.
    fn open(self) -> Open {
        match self {
            Kind::Patch => Open::Patch { track: 0 },
            Kind::Effect => Open::Effect {
                track: 0,
                slot: None,
            },
        }
    }
}

/// Starts a player that connects to the link `name`; `None` when there is no
/// process to watch.
pub type Launch = fn(executable: &Path, name: &str) -> std::io::Result<Option<Child>>;

/// What the GUI has to act on.
#[derive(Debug)]
pub enum Event {
    Selected(Box<Selected>),
    /// The user hid the browser; the player stays for the next [`Link::show`].
    Hidden,
    /// The player has gone; the text is the error to show, if any.
    Closed(Option<String>),
}

pub struct Link {
    name: String,
    incoming: mpsc::Receiver<Incoming>,
    child: Option<Child>,
    /// `None` until the player has connected.
    sender: Option<Sender>,
    /// The browser [`Link::show`] brings to the front.
    pub kind: Kind,
}

enum Incoming {
    Connected(Sender),
    Selected(Box<Selected>),
    Hidden,
    Closed(Option<String>),
}

/// The player for this process. A miss is reported to the GUI, which offers the download.
pub fn player() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    resolve(&executable, std::env::var_os("PATH")).inspect_err(|_| crate::player_download::missed())
}

/// The player beside this executable, then a release build in the sibling
/// cat-plugin-player repo, then one on `PATH`.
pub fn resolve(executable: &Path, path: Option<OsString>) -> Result<PathBuf, String> {
    let beside = executable.parent().map(|p| p.join(EXECUTABLE));
    let on_path = path
        .iter()
        .flat_map(std::env::split_paths)
        .map(|d| d.join(EXECUTABLE));
    beside
        .into_iter()
        .chain(sibling_release(executable))
        .chain(on_path)
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| t("patch_browser_00").into())
}

fn sibling_release(executable: &Path) -> Option<PathBuf> {
    let profile = executable.parent()?;
    if !matches!(profile.file_name()?.to_str()?, "debug" | "release") {
        return None;
    }
    let target = profile.parent()?;
    if target.file_name()? != "target" {
        return None;
    }
    Some(
        target
            .parent()?
            .parent()?
            .join("cat-plugin-player/target/release")
            .join(EXECUTABLE),
    )
}

impl Link {
    pub fn start(executable: &Path, launch: Launch) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            "repl-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let failed = |e: std::io::Error| render("patch_browser_01", &[("error", e.to_string())]);
        let listener = Listener::create(&name).map_err(failed)?;
        let child = launch(executable, &name).map_err(failed)?;
        let (deliver, incoming) = mpsc::channel();
        // `accept` and `receive` block, so the pipe is read off the GUI thread.
        std::thread::spawn(move || {
            let mut receiver = match listener.accept() {
                Ok((sender, receiver)) => {
                    if deliver.send(Incoming::Connected(sender)).is_err() {
                        return;
                    }
                    receiver
                }
                Err(error) => {
                    let _ = deliver.send(Incoming::Closed(Some(error.to_string())));
                    return;
                }
            };
            loop {
                let next = match receiver.receive::<ToEditor>() {
                    Ok(Some(ToEditor::Selected(selected))) => {
                        Incoming::Selected(Box::new(selected))
                    }
                    Ok(Some(ToEditor::BrowserClosed { .. })) => Incoming::Hidden,
                    // Kit, pane and cursor notices concern browsers this app never opens.
                    Ok(Some(_)) => continue,
                    Ok(None) => Incoming::Closed(None),
                    Err(error) => Incoming::Closed(Some(error.to_string())),
                };
                let closed = matches!(next, Incoming::Closed(_));
                if deliver.send(next).is_err() || closed {
                    return;
                }
            }
        });
        Ok(Self {
            name,
            incoming,
            child,
            sender: None,
            kind: Kind::Patch,
        })
    }

    /// Brings the browser of [`Link::kind`] to the front; before the player
    /// connects, it is shown on connection instead.
    pub fn show(&mut self) -> Result<(), String> {
        match &mut self.sender {
            Some(sender) => sender
                .send(&ToPlayer::Open(self.kind.open()))
                .map_err(|e| render("patch_browser_02", &[("error", e.to_string())])),
            None => Ok(()),
        }
    }

    /// Tells the player how the last [`Event::Selected`] went, so it offers the next one.
    pub fn answer(&mut self, result: Result<(), String>) -> Result<(), String> {
        let sender = self.sender.as_mut().ok_or_else(|| t("patch_browse_01"))?;
        sender
            .send(&ToPlayer::Applied(result))
            .map_err(|e| render("patch_browser_02", &[("error", e.to_string())]))
    }

    /// Everything that arrived since the last call. After [`Event::Closed`]
    /// the link is finished and should be dropped.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            let incoming = match self.incoming.try_recv() {
                Ok(incoming) => incoming,
                Err(mpsc::TryRecvError::Empty) => {
                    if let Some(error) = self.exited_early() {
                        events.push(Event::Closed(Some(error)));
                    }
                    return events;
                }
                Err(mpsc::TryRecvError::Disconnected) => Incoming::Closed(None),
            };
            match incoming {
                Incoming::Connected(sender) => {
                    self.sender = Some(sender);
                    if let Err(error) = self.show() {
                        events.push(Event::Closed(Some(error)));
                        return events;
                    }
                }
                Incoming::Selected(selected) => events.push(Event::Selected(selected)),
                Incoming::Hidden => events.push(Event::Hidden),
                Incoming::Closed(error) => {
                    let error = error.map(|e| render("patch_browser_03", &[("error", e)]));
                    events.push(Event::Closed(error));
                    return events;
                }
            }
        }
    }

    /// A player that exits before connecting would leave `accept` waiting
    /// forever; connecting from here releases that thread.
    fn exited_early(&mut self) -> Option<String> {
        if self.sender.is_some() {
            return None;
        }
        let status = self.child.as_mut()?.try_wait().ok()??;
        drop(resident_ipc::connect(&self.name));
        Some(render(
            "patch_browser_04",
            &[("status", status.to_string())],
        ))
    }
}

pub fn spawn(executable: &Path, name: &str) -> std::io::Result<Option<Child>> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(executable);
    crate::surge_home::restore_for_child(&mut command);
    command
        .arg("--resident")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(Some)
}

#[cfg(test)]
#[path = "patch_browser_tests.rs"]
pub(crate) mod tests;
