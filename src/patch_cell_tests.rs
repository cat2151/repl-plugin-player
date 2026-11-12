use super::*;
use crate::{host::PluginKey, patch_browser::tests::selected};

#[test]
fn names_and_patches_become_escaped_single_quoted_literals() {
    let mut odd = selected("dir/It's a\r\nname\u{2028}.fxp");
    odd.plugin = PluginKey {
        name: "Plug\\'in\n2".into(),
        ..odd.plugin
    };
    let snippet = snippet(&odd, 0);
    assert_eq!(
        snippet.code,
        concat!(
            "{\n    const track = 0;\n",
            r"    include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Plug\\\'in\n2', ",
            r"patch:'dir/It\'s a\r\nname\u2028.fxp', track});",
            "\n}"
        )
    );
    assert_eq!(snippet.name, "Plug\\'in 2 / dir/It's a  name\u{2028}.fxp");
}

#[test]
fn a_selection_without_a_patch_name_uses_the_plugin_default() {
    let mut plain = selected("unused");
    plain.preset_name = None;
    let snippet = snippet(&plain, 3);
    assert_eq!(
        snippet.code,
        "{\n    const track = 3;\n    include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', track});\n}"
    );
    assert_eq!(snippet.name, "Surge XT");
}
