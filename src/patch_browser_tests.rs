use super::*;
use crate::host::PluginKey;
use crate::resident_ipc::StateRef;
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

thread_local! {
    static LAUNCHED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Records the link name; the test connects as the player itself.
pub(crate) fn record(_: &Path, name: &str) -> std::io::Result<Option<Child>> {
    LAUNCHED.with(|l| l.borrow_mut().push(name.to_owned()));
    Ok(None)
}

pub(crate) fn last_launched() -> String {
    LAUNCHED.with(|l| l.borrow().last().cloned().unwrap())
}

/// A player that exits at once without connecting.
fn exit_at_once(_: &Path, _: &str) -> std::io::Result<Option<Child>> {
    Command::new("cmd")
        .args(["/c", "exit", "3"])
        .spawn()
        .map(Some)
}

/// Polls like the GUI does until `done` holds, collecting the events.
fn poll_until(link: &mut Link, done: impl Fn(&Link, &[Event]) -> bool) -> Vec<Event> {
    let start = Instant::now();
    let mut events = Vec::new();
    while !done(link, &events) {
        assert!(start.elapsed() < Duration::from_secs(60), "timed out");
        events.extend(link.poll());
        std::thread::sleep(Duration::from_millis(2));
    }
    events
}

pub(crate) fn selected(preset_name: &str) -> Selected {
    Selected {
        kit: None,
        reference: None,
        target: Kind::Patch.open(),
        plugin: PluginKey {
            format: "CLAP".into(),
            id: "org.surge-synth-team.surge-xt".into(),
            name: "Surge XT".into(),
            vendor: "Surge Synth Team".into(),
            bundle_path: String::new(),
        },
        preset_name: Some(preset_name.into()),
        state: StateRef {
            name: "unused".into(),
            len: 0,
        },
    }
}

#[test]
fn the_player_gets_the_patch_browser_and_each_selection_is_answered() {
    let mut link = Link::start(Path::new("player.exe"), record).unwrap();
    let (mut player_out, mut player_in) = resident_ipc::connect(&last_launched()).unwrap();
    poll_until(&mut link, |link, _| link.sender.is_some());
    assert_eq!(
        player_in.receive::<ToPlayer>().unwrap(),
        Some(ToPlayer::Open(Open::Patch { track: 0 }))
    );

    for (name, result) in [("Attacky", Ok(())), ("Missing", Err("no patch".into()))] {
        player_out
            .send(&ToEditor::Selected(selected(name)))
            .unwrap();
        let events = poll_until(&mut link, |_, events| !events.is_empty());
        let [Event::Selected(got)] = events.as_slice() else {
            panic!("{events:?}");
        };
        assert_eq!(got.preset_name.as_deref(), Some(name));
        assert_eq!(got.plugin.name, "Surge XT");
        link.answer(result.clone()).unwrap();
        assert_eq!(
            player_in.receive::<ToPlayer>().unwrap(),
            Some(ToPlayer::Applied(result))
        );
    }

    link.show().unwrap();
    assert_eq!(
        player_in.receive::<ToPlayer>().unwrap(),
        Some(ToPlayer::Open(Open::Patch { track: 0 }))
    );
}

#[test]
fn notices_for_other_browsers_are_skipped() {
    let mut link = Link::start(Path::new("player.exe"), record).unwrap();
    let (mut player_out, _player_in) = resident_ipc::connect(&last_launched()).unwrap();
    player_out
        .send(&ToEditor::Cursor {
            context: "c".into(),
            patch: None,
        })
        .unwrap();
    player_out
        .send(&ToEditor::Selected(selected("Attacky")))
        .unwrap();
    let events = poll_until(&mut link, |_, events| !events.is_empty());
    assert!(
        matches!(events.as_slice(), [Event::Selected(_)]),
        "{events:?}"
    );
}

#[test]
fn a_player_closing_its_pipes_ends_the_link_without_an_error() {
    let mut link = Link::start(Path::new("player.exe"), record).unwrap();
    let player = resident_ipc::connect(&last_launched()).unwrap();
    poll_until(&mut link, |link, _| link.sender.is_some());
    drop(player);
    let events = poll_until(&mut link, |_, events| !events.is_empty());
    assert!(
        matches!(events.as_slice(), [Event::Closed(None)]),
        "{events:?}"
    );
}

#[test]
fn a_player_exiting_before_it_connects_is_reported() {
    let mut link = Link::start(Path::new("cmd.exe"), exit_at_once).unwrap();
    let events = poll_until(&mut link, |_, events| !events.is_empty());
    let [Event::Closed(Some(error))] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert!(error.contains("exited before connecting"), "{error}");
}

#[test]
fn the_player_is_found_in_the_sibling_release_build_then_on_path() {
    let root = std::env::temp_dir().join(format!("repl-patch-browser-{}", std::process::id()));
    let app = root.join("repl-plugin-player/target/release/repl-plugin-player.exe");
    let sibling = root
        .join("cat-plugin-player/target/release")
        .join(EXECUTABLE);
    let on_path = root.join("bin").join(EXECUTABLE);
    for file in [&sibling, &on_path] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "").unwrap();
    }
    let path = Some(on_path.parent().unwrap().as_os_str().to_owned());
    assert_eq!(resolve(&app, path.clone()), Ok(sibling.clone()));
    std::fs::remove_file(&sibling).unwrap();
    assert_eq!(resolve(&app, path), Ok(on_path));
    assert_eq!(resolve(&app, None), Err(t("patch_browser_00").into()));
    std::fs::remove_dir_all(&root).unwrap();
}

/// With `CAT_PLUGIN_PLAYER_EXE` set, starts that player, waits until it has
/// connected and still runs, prints its pid and leaves the link open, so a
/// caller can check the player ends with this test process.
#[test]
fn a_real_player_connects_and_stays_while_the_link_is_open() {
    let Some(executable) = std::env::var_os("CAT_PLUGIN_PLAYER_EXE") else {
        return;
    };
    let mut link = Link::start(Path::new(&executable), spawn).unwrap();
    let events = poll_until(&mut link, |link, events| {
        link.sender.is_some() || !events.is_empty()
    });
    assert!(events.is_empty(), "{events:?}");
    std::thread::sleep(Duration::from_secs(2));
    assert!(link.poll().is_empty());
    let child = link.child.as_mut().unwrap();
    assert!(child.try_wait().unwrap().is_none());
    println!("player pid {}", child.id());
    std::mem::forget(link);
}
