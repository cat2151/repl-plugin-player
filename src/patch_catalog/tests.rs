use super::*;
use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

fn sample_groups() -> Groups {
    let patch = |display: &str| CatalogPatch {
        display: display.into(),
        category: None,
        drum_kit: false,
    };
    [
        (
            ("alpha".into(), "Alpha".into()),
            vec![
                patch("Pads/Soft"),
                patch("Bass/Pad Bass"),
                patch("Leads/Pad Lead"),
            ],
        ),
        (("beta".into(), "Beta".into()), vec![patch("Pads/Soft")]),
    ]
    .into()
}

#[test]
fn pad_kind_uses_exclusive_roles_and_keeps_plugin_identity() {
    let selected = filter(&classify(sample_groups(), &[]), PatchRole::Chord, "pad");
    assert_eq!(
        selected,
        json!([
            {"format":"CLAP","pluginId":"alpha","plugin":"Alpha","patch":"Pads/Soft"},
            {"format":"CLAP","pluginId":"alpha","plugin":"Alpha","patch":"Leads/Pad Lead"},
            {"format":"CLAP","pluginId":"beta","plugin":"Beta","patch":"Pads/Soft"}
        ])
    );
}

/// The classification before it was held: build the index and test each patch per request.
fn select_per_request(groups: Groups, role: PatchRole, kind: &str) -> Value {
    let mut result = Vec::new();
    for ((id, name), patches) in groups {
        let normalized: Vec<_> = patches.iter().map(|p| p.display.to_lowercase()).collect();
        let index = PatchRoleIndex::build(
            patches
                .iter()
                .zip(&normalized)
                .map(|(patch, n)| PatchRoleInput {
                    display: &patch.display,
                    normalized_display: n,
                    selector_category: patch.category.as_deref(),
                    plugin: Some(&name),
                }),
            &[],
        );
        for CatalogPatch { display, .. } in patches {
            if index.role_of(&display) == Some(role)
                && index.preset_label_of(&display) == Some(kind)
            {
                result.push(json!({"format":"CLAP","pluginId":id,"plugin":name,"patch":display}));
            }
        }
    }
    json!(result)
}

#[test]
fn held_entries_filter_like_per_request_classification_for_every_role_and_kind() {
    let entries = classify(sample_groups(), &[]);
    let kinds: Vec<_> = entries.iter().filter_map(|e| e.kind).collect();
    assert!(!kinds.is_empty());
    for role in PatchRole::ALL {
        for kind in &kinds {
            assert_eq!(
                filter(&entries, role, kind),
                select_per_request(sample_groups(), role, kind)
            );
        }
    }
}

/// Calls `held` and counts how often it read.
fn call(slot: &Held, reads: &AtomicU32) -> Result<usize, String> {
    held(slot, || {
        reads.fetch_add(1, Ordering::SeqCst);
        Ok(classify(sample_groups(), &[]))
    })
    .map(|entries| entries.len())
}

#[test]
fn second_call_is_not_read_again() {
    let (slot, reads) = (Mutex::new(None), AtomicU32::new(0));
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn list_during_preload_waits_instead_of_reading_again() {
    let slot = Arc::new(Mutex::new(None));
    let reads = Arc::new(AtomicU32::new(0));
    let (started, entered) = mpsc::channel();
    let preload = {
        let (slot, reads) = (slot.clone(), reads.clone());
        std::thread::spawn(move || {
            held(&slot, || {
                reads.fetch_add(1, Ordering::SeqCst);
                started.send(()).unwrap();
                std::thread::sleep(Duration::from_millis(200));
                Ok(classify(sample_groups(), &[]))
            })
            .map(|entries| entries.len())
        })
    };
    entered.recv().unwrap();
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(preload.join().unwrap(), Ok(4));
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_read_is_not_held_and_is_retried() {
    let (slot, reads) = (Mutex::new(None), AtomicU32::new(0));
    let failed = held(&slot, || {
        reads.fetch_add(1, Ordering::SeqCst);
        Err("broken cache".to_owned())
    });
    assert_eq!(failed.err().as_deref(), Some("broken cache"));
    assert!(slot.lock().unwrap().is_none());
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[test]
fn reload_reads_once_more_and_is_held_again() {
    let (slot, reads) = (Mutex::new(None), AtomicU32::new(0));
    assert_eq!(call(&slot, &reads), Ok(4));
    let rebuilt = rebuild(
        &slot,
        || {},
        || {
            reads.fetch_add(1, Ordering::SeqCst);
            Ok(classify(sample_groups(), &[]))
        },
    );
    assert_eq!(rebuilt.map(|entries| entries.len()), Ok(4));
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[test]
fn failed_reload_drops_the_held_catalog_and_reports_the_error() {
    let (slot, reads) = (Mutex::new(None), AtomicU32::new(0));
    assert_eq!(call(&slot, &reads), Ok(4));
    let failed = rebuild(&slot, || {}, || Err("broken cache".to_owned()));
    assert_eq!(failed.err().as_deref(), Some("broken cache"));
    assert!(slot.lock().unwrap().is_none());
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[test]
fn list_after_reload_returns_waits_for_the_new_catalog() {
    let slot = Arc::new(Mutex::new(None));
    let reads = Arc::new(AtomicU32::new(0));
    assert_eq!(call(&slot, &reads), Ok(4));
    let (locked, wait) = mpsc::channel();
    let reload = {
        let (slot, reads) = (slot.clone(), reads.clone());
        std::thread::spawn(move || {
            rebuild(
                &slot,
                || locked.send(()).unwrap(),
                || {
                    std::thread::sleep(Duration::from_millis(200));
                    reads.fetch_add(1, Ordering::SeqCst);
                    Ok(classify(sample_groups(), &[]))
                },
            )
            .map(|entries| entries.len())
        })
    };
    wait.recv().unwrap();
    assert_eq!(call(&slot, &reads), Ok(4));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(reload.join().unwrap(), Ok(4));
}
