//! Outstanding Note Ons at instrument inputs, including repeated pitches.
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

static NODES: Mutex<Vec<Arc<Published>>> = Mutex::new(Vec::new());

struct Published {
    instance: i32,
    count: AtomicU64,
}

struct Notes {
    held: [u32; 16 * 16 * 128],
    total: u64,
    published: Arc<Published>,
}
impl Notes {
    fn new(instance: i32) -> Self {
        let published = Arc::new(Published {
            instance,
            count: AtomicU64::new(0),
        });
        NODES.lock().unwrap().push(published.clone());
        Self {
            held: [0; 16 * 16 * 128],
            total: 0,
            published,
        }
    }
}
impl Notes {
    fn process(&mut self, words: &[u32]) {
        const LENGTHS: [usize; 16] = [1, 1, 1, 2, 2, 4, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4];
        let mut i = 0;
        while i < words.len() {
            let word = words[i];
            let kind = (word >> 28) as usize;
            let length = LENGTHS[kind];
            if length > words.len() - i {
                break;
            }
            if kind == 2 || kind == 4 {
                let channel = (((word >> 24) & 15) * 16 + ((word >> 16) & 15)) as usize;
                let pitch = ((word >> 8) & 127) as usize;
                let velocity = if kind == 2 {
                    word & 127
                } else {
                    words[i + 1] >> 16
                };
                let status = (word >> 20) & 15;
                let count = &mut self.held[channel * 128 + pitch];
                match status {
                    9 if velocity > 0 => {
                        if *count < u32::MAX {
                            *count += 1;
                            self.total += 1;
                        }
                    }
                    8 | 9 => {
                        if *count > 0 {
                            *count -= 1;
                            self.total -= 1;
                        }
                    }
                    11 if pitch == 120 || pitch == 123 => {
                        for held in &mut self.held[channel * 128..(channel + 1) * 128] {
                            self.total -= u64::from(*held);
                            *held = 0;
                        }
                    }
                    _ => (),
                }
            }
            i += length;
        }
        self.published.count.store(self.total, Ordering::Relaxed);
    }
}
impl Drop for Notes {
    fn drop(&mut self) {
        NODES
            .lock()
            .unwrap()
            .retain(|node| !Arc::ptr_eq(node, &self.published));
    }
}

#[no_mangle]
pub unsafe extern "C" fn repl_note_counter_read(
    ids: *const i32,
    capabilities: *const u32,
    count: usize,
) -> u64 {
    if count == 0 || ids.is_null() || capabilities.is_null() {
        return 0;
    }
    let ids = unsafe { std::slice::from_raw_parts(ids, count) };
    let capabilities = unsafe { std::slice::from_raw_parts(capabilities, count) };
    // The adapter supplies facts, not an instrument/effect policy:
    // bit 0 = MIDI input, bit 1 = audio output, bit 2 = upstream audio node.
    let selected = |instance| {
        ids.iter()
            .zip(capabilities)
            .any(|(id, flags)| *id == instance && flags & 7 == 3)
    };
    NODES
        .lock()
        .unwrap()
        .iter()
        .filter(|node| selected(node.instance))
        .map(|node| node.count.load(Ordering::Relaxed))
        .sum()
}

// Native constructs/destroys each state outside processing and serializes that
// node's process calls. Audio only publishes its own atomic count; registration,
// destruction and selected-instance summation use the registry off the audio thread.
#[no_mangle]
pub unsafe extern "C" fn repl_note_counter_create(instance: i32) -> *mut c_void {
    Box::into_raw(Box::new(Notes::new(instance))).cast()
}
#[no_mangle]
pub unsafe extern "C" fn repl_note_counter_destroy(state: *mut c_void) {
    if !state.is_null() {
        drop(unsafe { Box::from_raw(state.cast::<Notes>()) });
    }
}
#[no_mangle]
pub unsafe extern "C" fn repl_note_counter_process(
    state: *mut c_void,
    words: *const u32,
    count: usize,
) {
    if !state.is_null() && count > 0 && !words.is_null() {
        unsafe {
            (&mut *state.cast::<Notes>()).process(std::slice::from_raw_parts(words, count));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_instances_retriggers_channels_groups_and_releases() {
        let mut first = Notes::new(10);
        let mut second = Notes::new(20);
        let read = || unsafe { repl_note_counter_read([10, 20].as_ptr(), [3, 3].as_ptr(), 2) };
        first.process(&[0x20903c7f, 0x20903c7f, 0x20913c7f, 0x21903c7f]);
        second.process(&[0x40903c00, 0xffff0000]);
        assert_eq!((first.total, second.total, read()), (4, 1, 5));
        assert_eq!(
            unsafe { repl_note_counter_read([10, 20].as_ptr(), [7, 3].as_ptr(), 2) },
            1
        );
        assert_eq!(
            unsafe { repl_note_counter_read([10, 20].as_ptr(), [2, 1].as_ptr(), 2) },
            0
        );
        first.process(&[0x20803c00]);
        assert_eq!(first.total, 3);
        first.process(&[0x20b07b00]);
        assert_eq!(first.total, 2);
        first.process(&[0x20913c00, 0x21b07800]);
        assert_eq!(first.total, 0);
        second.process(&[0x40803c00, 0]);
        assert_eq!(second.total, 0);
        second.process(&[0x40903c00]); // truncated MIDI 2
        assert_eq!(second.total, 0);
        second.process(&[0x20903c7f]);
        drop(second);
        assert_eq!(read(), 0);
    }
}
