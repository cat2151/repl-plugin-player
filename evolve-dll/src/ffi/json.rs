//! The JSON text `status` and `poll` return: flat objects of numbers, bools
//! and strings, written by hand.
use std::fmt::Write;

pub struct Object<'a> {
    out: &'a mut String,
    first: bool,
}

impl<'a> Object<'a> {
    pub fn open(out: &'a mut String) -> Self {
        out.push('{');
        Self { out, first: true }
    }

    fn key(&mut self, key: &str) {
        if !std::mem::take(&mut self.first) {
            self.out.push(',');
        }
        string(self.out, key);
        self.out.push(':');
    }

    /// `null` when not finite.
    pub fn number(&mut self, key: &str, value: f64) -> &mut Self {
        self.key(key);
        if value.is_finite() {
            let _ = write!(self.out, "{value}");
        } else {
            self.out.push_str("null");
        }
        self
    }

    pub fn boolean(&mut self, key: &str, value: bool) -> &mut Self {
        self.key(key);
        self.out.push_str(if value { "true" } else { "false" });
        self
    }

    pub fn string(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        string(self.out, value);
        self
    }

    pub fn close(&mut self) {
        self.out.push('}');
    }
}

fn string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `[` + each item + `]`, comma separated.
pub fn array<T>(
    items: impl IntoIterator<Item = T>,
    mut write: impl FnMut(&mut String, T),
) -> String {
    let mut out = String::from("[");
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write(&mut out, item);
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_escaped_and_non_finite_numbers_are_null() {
        let text = array([1.5, f64::NAN], |out, value| {
            Object::open(out)
                .number("n", value)
                .boolean("b", value > 1.0)
                .string("s", "a\"\\\n\0あ")
                .close();
        });
        assert_eq!(
            text,
            r#"[{"n":1.5,"b":true,"s":"a\"\\\u000a\u0000あ"},{"n":null,"b":false,"s":"a\"\\\u000a\u0000あ"}]"#
        );
    }
}
