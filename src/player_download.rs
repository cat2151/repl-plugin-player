//! `cat-plugin-player.exe`, fetched beside this exe from the release zip that
//! the player's README links, once a call needed the player and found none.
use crate::i18n::{render, t};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

const README: &str = "https://raw.githubusercontent.com/cat2151/cat-plugin-player/main/README.md";
const RELEASE: &str = "https://github.com/cat2151/cat-plugin-player/releases/download/";
const EXECUTABLE: &str = "cat-plugin-player.exe";

static MISSED: AtomicBool = AtomicBool::new(false);

pub fn missed() {
    MISSED.store(true, Ordering::Relaxed);
}

/// Whether a call missed the player since the last take.
pub fn take_missed() -> bool {
    MISSED.swap(false, Ordering::Relaxed)
}

/// Blocks on the network; the GUI runs it on a worker thread.
pub fn install(directory: &Path) -> Result<(), String> {
    let readme = run(curl().args(["-fsSL", README]))?;
    let url = zip_url(&String::from_utf8_lossy(&readme)).ok_or(t("player_download_00"))?;
    let work = std::env::temp_dir().join(format!("repl-player-download-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = fetch(&url, &work, directory);
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Extracts in `work` and renames into place, so a failure never leaves a partial exe.
fn fetch(url: &str, work: &Path, directory: &Path) -> Result<(), String> {
    let zip = work.join("player.zip");
    run(curl().args(["-fsSL", "-o"]).arg(&zip).arg(url))?;
    run(system("tar.exe")
        .arg("-xf")
        .arg(&zip)
        .arg("-C")
        .arg(work)
        .arg(EXECUTABLE))?;
    let part = directory.join(format!("{EXECUTABLE}.part"));
    std::fs::copy(work.join(EXECUTABLE), &part).map_err(|e| e.to_string())?;
    std::fs::rename(&part, directory.join(EXECUTABLE)).map_err(|e| e.to_string())
}

fn zip_url(readme: &str) -> Option<String> {
    readme.match_indices(RELEASE).find_map(|(start, _)| {
        let url: String = readme[start..]
            .chars()
            .take_while(|c| !matches!(c, ')' | '"' | '\'' | '>' | '<') && !c.is_whitespace())
            .collect();
        url.ends_with(".zip").then_some(url)
    })
}

fn system(name: &str) -> Command {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    Command::new(PathBuf::from(root).join("System32").join(name))
}

fn curl() -> Command {
    system("curl.exe")
}

fn run(command: &mut Command) -> Result<Vec<u8>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let program = Path::new(command.get_program())
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let failed = |error: String| {
        render(
            "player_download_01",
            &[("program", program.clone()), ("error", error)],
        )
    };
    let output = command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| failed(e.to_string()))?;
    if !output.status.success() {
        return Err(failed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_release_zip_link_is_taken() {
        let readme = "## Download\n\n[Windows](https://github.com/cat2151/cat-plugin-player/releases/download/v0.1.0/cat-plugin-player-0.1.0-windows-x64.zip)\n\
            [later](https://github.com/cat2151/cat-plugin-player/releases/download/v0.2.0/b.zip)";
        assert_eq!(
            zip_url(readme).as_deref(),
            Some("https://github.com/cat2151/cat-plugin-player/releases/download/v0.1.0/cat-plugin-player-0.1.0-windows-x64.zip")
        );
    }

    /// Network: `cargo test --release player_download -- --ignored`.
    #[test]
    #[ignore]
    fn the_readme_zip_installs_a_player_that_answers_help() {
        let directory =
            std::env::temp_dir().join(format!("repl-player-install-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        install(&directory).unwrap();
        let help = run(Command::new(directory.join(EXECUTABLE)).arg("--help")).unwrap();
        assert!(String::from_utf8_lossy(&help).contains("--patch-roots"));
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_release_link_that_is_not_a_zip_is_skipped() {
        let readme =
            "<https://github.com/cat2151/cat-plugin-player/releases/download/v0.1.0/notes.txt>";
        assert_eq!(zip_url(readme), None);
        assert_eq!(zip_url("no links"), None);
    }
}
