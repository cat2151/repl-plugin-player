use eframe::egui::Key;

/// The key the user pressed, as the active keyboard layout names it; `None` when the
/// layout's character has no egui key (JIS `@`).
pub(super) fn logical(key: Key, physical_key: Option<Key>) -> Option<Key> {
    // Ctrl+Alt can turn winit's logical key into an unrecognized control
    // character. egui then uses the US physical key name as `key` instead.
    // Recover the unmodified character from the active Windows layout so a
    // JIS '[' (US BracketRight) and ']' (US Backslash) keep their meanings.
    match physical_key.and_then(layout_character) {
        Some(character) => Key::from_name(&character.to_string()),
        None => Some(key),
    }
}

#[cfg(windows)]
pub(crate) fn layout_character(key: Key) -> Option<char> {
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
    fn layout_characters_name_the_key_or_none() {
        // Windows queries for scan codes 1A/1B/2B: JIS gives @/[/], US [/]/\.
        let name = |character: char| Key::from_name(&character.to_string());
        assert_eq!(
            ['@', '[', ']'].map(name),
            [None, Some(Key::OpenBracket), Some(Key::CloseBracket)]
        );
        assert_eq!(
            ['[', ']', '\\'].map(name),
            [
                Some(Key::OpenBracket),
                Some(Key::CloseBracket),
                Some(Key::Backslash)
            ]
        );
        assert_eq!(logical(Key::Period, None), Some(Key::Period));
    }
}
