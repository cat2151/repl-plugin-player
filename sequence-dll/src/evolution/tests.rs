use super::*;
use repl_extension_api::sequence::Step;

#[test]
fn a_ready_voice_waits_for_a_plan_even_when_transport_already_runs() {
    let e = Evolution::new(5.0, 1.0).unwrap();
    assert!(e.claim(0));
    e.ready(0, true);
    assert_eq!(block(&e, 100), [vec![0.0; 100], vec![0.0; 100]]);
    assert_eq!(e.status(0), READY);
    assert_eq!(e.status(1), WAITING);
    sequence(&e);
    block(&e, 1);
    assert_eq!(e.status(0), ENGAGED);
    assert_eq!(e.status(1), NEED);
}

fn sequence(e: &Evolution) {
    let mut plan = Plan {
        count: 1,
        seconds: 1.0,
        gate: 1.0,
        velocity: 20000,
        ..Default::default()
    };
    plan.steps[0] = Step {
        notes: [60, 64, 67, 71, 0, 0, 0, 0],
        count: 4,
    };
    assert_eq!(e.publish(&plan), 0);
}
fn block(e: &Evolution, frames: u32) -> [Vec<f32>; 2] {
    unsafe { e.begin(frames, 100.0) };
    let mut gains = [vec![1.0; frames as usize], vec![1.0; frames as usize]];
    for (slot, gain) in gains.iter_mut().enumerate() {
        unsafe {
            e.render(slot, frames, 100.0, |_, _, _, _| {});
            e.mix(slot, gain);
            e.finish(slot);
        }
    }
    gains
}
#[test]
fn late_preparation_holds_current_and_retirement_requests_immediate_reverse_load() {
    let e = Evolution::new(5.0, 1.0).unwrap();
    sequence(&e);
    assert!(e.claim(0));
    e.ready(0, true);
    assert_eq!(e.status(1), WAITING);
    assert_eq!(block(&e, 700), [vec![1.0; 700], vec![0.0; 700]]);
    assert_eq!(e.status(1), NEED);
    assert!(!e.claim(0));
    assert!(e.claim(1));
    assert_eq!(block(&e, 100), [vec![1.0; 100], vec![0.0; 100]]);
    e.ready(1, true);
    block(&e, 100); // Prime a whole chord before blending.
    let gains = block(&e, 101);
    for (&a, &b) in gains[0].iter().zip(&gains[1]) {
        assert!((a + b - 1.0).abs() < 1e-6);
    }
    assert_eq!(*gains[0].last().unwrap(), 0.0);
    assert_eq!(*gains[1].last().unwrap(), 1.0);
    assert_eq!(e.status(0), NEED);
    assert_eq!(e.status(1), ENGAGED);
    assert_eq!(e.gain(0), 0.0);
    assert_eq!(e.gain(1), 1.0);
    assert!(e.claim(0));
    e.ready(0, true);
    let hold = block(&e, 400);
    assert!(hold[0].iter().all(|&x| x == 0.0));
    assert!(hold[1].iter().all(|&x| x == 1.0));
    block(&e, 201);
    assert_eq!(e.status(1), NEED);
}
#[test]
fn ready_next_never_shortens_five_seconds_and_failed_load_keeps_current() {
    let e = Evolution::new(5.0, 1.0).unwrap();
    sequence(&e);
    assert!(e.claim(0));
    e.ready(0, true);
    block(&e, 1);
    assert!(e.claim(1));
    e.ready(1, false);
    let hold = block(&e, 499);
    assert!(hold[0].iter().all(|&g| g == 1.0));
    assert!(e.claim(1));
    e.ready(1, true);
    block(&e, 100);
    block(&e, 101);
    assert_eq!(e.status(0), NEED);
}
#[test]
fn retire_sends_all_note_offs_before_control_ownership_and_silent_slot_sends_none() {
    let e = Evolution::new(5.0, 1.0).unwrap();
    sequence(&e);
    assert!(e.claim(0));
    e.ready(0, true);
    block(&e, 500);
    assert!(e.claim(1));
    e.ready(1, true);
    block(&e, 100);
    unsafe { e.begin(101, 100.0) };
    let mut events = Vec::new();
    unsafe {
        e.render(0, 101, 100.0, |at, p, on, _| events.push((at, p, on)));
    }
    assert_eq!(events.iter().filter(|(_, _, on)| !*on).count(), 4);
    assert_eq!(e.status(0), ENGAGED);
    unsafe {
        e.finish(0);
    }
    assert_eq!(e.status(0), NEED);
    unsafe {
        e.render(1, 101, 100.0, |_, _, _, _| {});
        e.finish(1);
    }
    unsafe {
        e.begin(100, 100.0);
        e.render(0, 100, 100.0, |_, _, _, _| {
            panic!("silent slot emitted notes")
        });
    }
}

#[test]
fn failed_worker_batch_defers_retirement_until_a_successful_after_callback() {
    let e = Evolution::new(5.0, 1.0).unwrap();
    sequence(&e);
    assert!(e.claim(0));
    e.ready(0, true);
    block(&e, 500);
    assert!(e.claim(1));
    e.ready(1, true);
    block(&e, 100);
    unsafe {
        e.begin(101, 100.0);
        e.render(0, 101, 100.0, |_, _, _, _| {});
        e.render(1, 101, 100.0, |_, _, _, _| {});
    }
    // UAPMD skips after callbacks for a failed worker batch. Ownership must
    // remain with audio even if its clock has already completed the fade.
    assert_eq!(e.status(0), ENGAGED);
    assert!(!e.claim(0));
    unsafe {
        e.begin(1, 100.0);
        assert!(!e.render(0, 1, 100.0, |_, _, _, _| panic!("retired voice restarted")));
        e.finish(0);
        e.render(1, 1, 100.0, |_, _, _, _| {});
        e.finish(1);
    }
    assert_eq!(e.status(0), NEED);
    assert!(e.claim(0));
}
