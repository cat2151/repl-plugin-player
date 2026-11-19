use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use super::*;
use crate::{
    ownership::{ENGAGED, FREE, READY},
    pool::SPARE_TARGET,
    test_host::{kind, Call, MockHost},
};

const LIMIT: Duration = Duration::from_secs(5);

fn setup() -> (Arc<MockHost>, Arc<Ownership>, Loader) {
    let host = Arc::new(MockHost::default());
    let ownership = Arc::new(Ownership::default());
    let loader = Loader::spawn(host.clone(), Arc::clone(&ownership));
    (host, ownership, loader)
}

fn wait_until(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + LIMIT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn settle(loader: &Loader) -> PreloadOutcome {
    let mut out = Vec::new();
    wait_until(|| {
        out.extend(loader.poll());
        !out.is_empty()
    });
    assert_eq!(out.len(), 1);
    out.remove(0)
}

fn spares(loader: &Loader, plugin: &str) -> usize {
    let wanted = kind(plugin);
    loader
        .view()
        .spares
        .iter()
        .filter(|(k, _)| *k == wanted)
        .map(|(_, n)| n)
        .sum()
}

/// Loads `plugin` into `slot`, lets playback take it, then hands it back.
fn load_and_return(loader: &Loader, ownership: &Ownership, slot: u8, plugin: &str) {
    loader.submit(slot, kind(plugin), vec![1], loader.generation());
    let outcome = settle(loader);
    assert!(outcome.error.is_none() && !outcome.stale, "{outcome:?}");
    assert!(ownership.engage(usize::from(slot)));
    assert!(ownership.retire(usize::from(slot)));
}

fn slot(loader: &Loader, slot: usize) -> (PluginKind, InstanceHandle) {
    loader.view().slots[slot].clone().expect("slot is loaded")
}

#[test]
fn the_same_kind_only_reloads_state_on_the_same_instance() {
    let (host, ownership, loader) = setup();
    load_and_return(&loader, &ownership, 0, "a");
    wait_until(|| spares(&loader, "a") == SPARE_TARGET);
    let (_, before) = slot(&loader, 0);
    let creates = host.creates();

    loader.submit(0, kind("a"), vec![2], loader.generation());
    let outcome = settle(&loader);

    assert!(outcome.error.is_none());
    assert_eq!(host.creates(), creates, "no instance is created");
    assert_eq!(slot(&loader, 0).1, before, "the physical instance is kept");
    assert_eq!(host.count(|c| *c == Call::Load(before)), 2);
    assert_eq!(ownership.get(0), READY);
}

#[test]
fn another_kind_swaps_in_a_waiting_spare_and_bags_the_old_instance() {
    let (host, ownership, loader) = setup();
    load_and_return(&loader, &ownership, 0, "a");
    load_and_return(&loader, &ownership, 1, "b");
    wait_until(|| spares(&loader, "a") == 1 && spares(&loader, "b") == 1);
    let (_, old) = slot(&loader, 0);
    let existing: Vec<_> = (1..=host.creates() as u64).collect();
    *host.create_delay.lock().unwrap() = Duration::from_millis(300);

    loader.submit(0, kind("b"), vec![3], loader.generation());
    let outcome = settle(&loader);

    assert!(outcome.error.is_none());
    assert!(
        outcome.elapsed < Duration::from_millis(300),
        "the swap does not wait for a creation: {:?}",
        outcome.elapsed
    );
    let (now_kind, now) = slot(&loader, 0);
    assert_eq!(now_kind, kind("b"));
    assert!(existing.contains(&now), "a spare made before the request");
    assert_eq!(host.attached(0), Some(now));
    assert_eq!(
        spares(&loader, "a"),
        2,
        "the evicted instance joins its bag"
    );
    assert!(!host.calls().contains(&Call::Destroy(old)));
}

#[test]
fn without_a_spare_the_builder_creates_one_while_submit_returns_at_once() {
    let (host, _ownership, loader) = setup();
    *host.create_delay.lock().unwrap() = Duration::from_millis(200);

    let started = Instant::now();
    let id = loader.submit(0, kind("a"), vec![1], loader.generation());
    let submit_time = started.elapsed();

    assert_ne!(id, 0);
    assert!(submit_time < Duration::from_millis(1), "{submit_time:?}");
    assert!(loader.poll().is_empty());
    let outcome = settle(&loader);
    assert_eq!(outcome.request_id, id);
    assert!(outcome.error.is_none());
    assert!(outcome.elapsed >= Duration::from_millis(200));
    assert_eq!(slot(&loader, 0).0, kind("a"));
    assert_eq!(host.attached(0), Some(slot(&loader, 0).1));
}

#[test]
fn physical_instances_per_kind_stay_within_slots_plus_target() {
    let (host, ownership, loader) = setup();
    let plan = [
        (0, "a"),
        (1, "a"),
        (0, "b"),
        (1, "b"),
        (0, "c"),
        (1, "a"),
        (0, "a"),
        (1, "c"),
        (0, "b"),
        (1, "b"),
    ];
    for (s, plugin) in plan {
        load_and_return(&loader, &ownership, s, plugin);
        let view = loader.view();
        for p in ["a", "b", "c"] {
            assert!(
                view.physical(&kind(p)) <= SLOT_COUNT + SPARE_TARGET,
                "{p}: {view:?}"
            );
        }
    }
    // Once the builder is idle, everything alive is accounted for.
    wait_until(|| {
        let view = loader.view();
        let total: usize = ["a", "b", "c"]
            .iter()
            .map(|p| view.physical(&kind(p)))
            .sum();
        total == host.alive()
    });
}

#[test]
fn a_sounding_slot_is_not_touched_until_playback_returns_it() {
    let (host, ownership, loader) = setup();
    load_and_return(&loader, &ownership, 0, "a");
    loader.submit(0, kind("a"), vec![1], loader.generation());
    settle(&loader);
    assert!(ownership.engage(0));
    let (_, sounding) = slot(&loader, 0);
    let calls = host.calls().len();

    loader.submit(0, kind("b"), vec![2], loader.generation());
    std::thread::sleep(Duration::from_millis(100));

    assert!(loader.poll().is_empty());
    assert_eq!(host.calls().len(), calls, "nothing is done for that slot");
    assert_eq!(host.attached(0), Some(sounding));
    assert_eq!(ownership.get(0), ENGAGED);

    assert!(ownership.retire(0));
    let outcome = settle(&loader);
    assert!(outcome.error.is_none());
    assert_eq!(slot(&loader, 0).0, kind("b"));
}

#[test]
fn a_failed_creation_is_an_error_and_the_slot_keeps_its_instance() {
    let (host, ownership, loader) = setup();
    load_and_return(&loader, &ownership, 0, "a");
    let before = slot(&loader, 0);
    host.fail_create.lock().unwrap().insert("x".into());

    loader.submit(0, kind("x"), vec![1], loader.generation());
    let outcome = settle(&loader);

    assert_eq!(outcome.error.as_deref(), Some("cannot create x"));
    assert_eq!(slot(&loader, 0), before);
    assert_eq!(host.attached(0), Some(before.1));
    assert_eq!(ownership.get(0), FREE);
}

#[test]
fn a_failed_state_load_on_a_spare_keeps_the_slot_and_bags_the_spare() {
    let (host, ownership, loader) = setup();
    load_and_return(&loader, &ownership, 0, "a");
    load_and_return(&loader, &ownership, 1, "b");
    wait_until(|| spares(&loader, "b") == 1);
    let before = slot(&loader, 0);
    *host.fail_load.lock().unwrap() = true;

    loader.submit(0, kind("b"), vec![1], loader.generation());
    let outcome = settle(&loader);

    assert_eq!(outcome.error.as_deref(), Some("bad state"));
    assert_eq!(slot(&loader, 0), before);
    assert_eq!(host.attached(0), Some(before.1));
    // The refill ordered when the spare was taken lands next to it.
    wait_until(|| host.creates() == 5);
    wait_until(|| spares(&loader, "b") == 2);
    assert_eq!(host.alive(), 5, "nothing was destroyed");
    assert_eq!(ownership.get(0), FREE);
}

#[test]
fn a_load_folded_by_a_bump_is_stale_and_not_handed_to_playback() {
    let (host, ownership, loader) = setup();
    *host.load_delay.lock().unwrap() = Duration::from_millis(100);
    loader.submit(0, kind("a"), vec![1], loader.generation());
    wait_until(|| host.count(|c| matches!(c, Call::Load(_))) == 1);

    loader.bump_generation();
    let outcome = settle(&loader);

    assert!(outcome.stale);
    assert_eq!(ownership.get(0), FREE);
}

#[test]
fn an_out_of_range_slot_is_rejected() {
    let (_host, _ownership, loader) = setup();
    assert_eq!(loader.submit(2, kind("a"), vec![1], 0), 0);
}

#[test]
fn dropping_the_loader_destroys_every_instance() {
    let (host, ownership, loader) = setup();
    load_and_return(&loader, &ownership, 0, "a");
    load_and_return(&loader, &ownership, 1, "b");
    *host.load_delay.lock().unwrap() = Duration::from_millis(100);
    loader.submit(0, kind("b"), vec![1], loader.generation());
    wait_until(|| host.count(|c| matches!(c, Call::Load(_))) == 3);

    drop(loader);

    assert_eq!(host.alive(), 0, "{:?}", host.calls());
    assert_eq!(host.attached(0), None);
    assert_eq!(host.attached(1), None);
}
