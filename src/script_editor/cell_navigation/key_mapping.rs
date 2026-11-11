use eframe::egui::Key;

pub(super) fn direction(key: Key, physical_key: Option<Key>) -> Option<bool> {
    // Ctrl+Alt can turn winit's logical key into an unrecognized control
    // character. egui then uses the US physical key name as `key` instead.
    // Recover the unmodified character from the active Windows layout so a
    // JIS '[' (US BracketRight) and ']' (US Backslash) keep their meanings.
    if let Some(character) = physical_key.and_then(layout_character) {
        return character_direction(character);
    }
    match key {
        Key::OpenBracket => Some(false),
        Key::CloseBracket => Some(true),
        _ => None,
    }
}

fn character_direction(character: char) -> Option<bool> {
    match character {
        '[' => Some(false),
        ']' => Some(true),
        _ => None,
    }
}

#[cfg(windows)]
fn layout_character(key: Key) -> Option<char> {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn GetKeyboardLayout(thread_id: u32) -> *mut c_void;
        fn MapVirtualKeyExW(code: u32, map_type: u32, layout: *mut c_void) -> u32;
    }

    let scan_code = match key {
        Key::OpenBracket => 0x1a,
        Key::CloseBracket => 0x1b,
        Key::Backslash => 0x2b,
        _ => return None,
    };
    // SAFETY: These user32 queries take scalar values and an opaque keyboard
    // layout handle. No caller-owned memory is read/written. Thread 0 selects
    // this UI thread's current layout, including runtime layout changes.
    let character = unsafe {
        let layout = GetKeyboardLayout(0);
        if layout.is_null() {
            return None;
        }
        let virtual_key = MapVirtualKeyExW(scan_code, 3, layout); // MAPVK_VSC_TO_VK_EX
        if virtual_key == 0 {
            return None;
        }
        MapVirtualKeyExW(virtual_key, 2, layout) // MAPVK_VK_TO_CHAR (unshifted)
    };
    if character == 0 || character & 0x8000_0000 != 0 {
        return None; // No character, or a dead key.
    }
    char::from_u32(character & 0xffff)
}

#[cfg(not(windows))]
fn layout_character(_: Key) -> Option<char> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_fallback_is_decoded_using_the_layout_character() {
        // Windows queries for scan codes 1A/1B/2B: JIS gives @/[/], US [/]/\.
        for (characters, expected) in [
            (['@', '[', ']'], [None, Some(false), Some(true)]),
            (['[', ']', '\\'], [Some(false), Some(true), None]),
        ] {
            assert_eq!(characters.map(character_direction), expected);
        }
        assert_eq!(direction(Key::OpenBracket, None), Some(false));
        assert_eq!(direction(Key::CloseBracket, None), Some(true));
    }

    #[cfg(windows)]
    #[test]
    fn active_jis_or_us_layout_handles_physical_fallback_in_the_editor() {
        use crate::script_editor::{
            tests::{cursor, frame},
            ScriptEditor,
        };
        use eframe::egui::{self, Event, Modifiers};

        let expected = match layout_character(Key::OpenBracket) {
            Some('@') => [None, Some(false), Some(true)], // JIS 106/109
            Some('[') => [Some(false), Some(true), None], // US 101/104
            _ => return, // This regression covers the two layouts above.
        };
        for (physical_key, next) in [Key::OpenBracket, Key::CloseBracket, Key::Backslash]
            .into_iter()
            .zip(expected)
        {
            let ctx = egui::Context::default();
            let mut editor = ScriptEditor::default();
            let mut code = "// %% first\na();\n// %% middle\nb();\n// %% last\nc();".to_owned();
            let before = code.clone();
            let middle = code.find("// %% middle").unwrap();
            let last = code.find("// %% last").unwrap();
            frame(&ctx, &mut editor, &mut code, vec![], true);
            cursor(&ctx, middle, middle);
            // This is exactly egui-winit's event when the logical Ctrl+Alt
            // character is unrecognized: key falls back to the US position.
            let events = vec![Event::Key {
                key: physical_key,
                physical_key: Some(physical_key),
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    command: true,
                    ..Modifiers::NONE
                },
            }];
            assert!(frame(&ctx, &mut editor, &mut code, events, true).is_none());
            let expected = match next {
                Some(false) => 0,
                Some(true) => last,
                None => middle,
            };
            let state = egui::TextEdit::load_state(&ctx, egui::Id::unique("script_text")).unwrap();
            assert_eq!(state.cursor.char_range().unwrap().primary.index.0, expected);
            assert_eq!(code, before);
        }
    }
}
