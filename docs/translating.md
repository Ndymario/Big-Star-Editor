# Translating the editor

The interface's text lives in `crates/ui-egui/lang/`, one flat JSON file per language. Every `<code>.json` there is a
language: the file's name is its code (`en`, `pt`, `pt-br`), and its `language.name` key is how the language is listed in
Preferences, in its own words. `build.rs` finds the files and embeds them, so nothing is read from disk at run time and the
web build gets them too. English (`en.json`) is the language every other one falls back to.

## Adding a language

1. Copy `crates/ui-egui/lang/en.json` to `<code>.json` (lowercase letters, digits and dashes).
2. Set `language.name` to the language's own name, and translate every other value.
3. Run `cargo test -p bigstar-ui-egui i18n`.

The tests fail if a key is missing, if a text uses a `{placeholder}` the English one lacks, if a value is empty, or if the
keys are not sorted. A language that is only partly done is not accepted; a key a file lacks would read in English.

## Keys and texts

- Keys are `<area>.<snake_case_text>`, for example `files.open_folder`. Text shared everywhere is under `common.`.
- `{name}` in a text is an argument. A translation may reorder or leave out arguments.
- A counted text has `key.one` and `key.other`, and `{n}` is the number.
- Files are sorted by key, with no repeats.

## In the code

```rust
use crate::i18n::{key, tr, tr_n};

ui.label(tr!("files.open_folder"));                       // &'static str
ui.label(tr!("app.delete_name", name = file));            // String, `{name}` filled in
ui.label(tr_n!("hex_editor.status_selection", n));        // `.one` or `.other`, `{n}` grouped
const TABS: [(Tab, &str); 2] = [(Tab::A, key!("x.a")), ..]; // a `const`: show it with `i18n::text(label)`
```

Keys are written as literals, so the tests can check them against the files. Write a new text in `en.json` and in every
other language file, in sorted position. The current language is set once per frame from the Preferences choice, so no
function takes a language.

## Level names

The level list's names come from `crates/project/levels/levels.yaml`, and the host sends them in English. The list reads
each run of digits in a name as `{1}`, `{2}` and so on, and looks the rest up as `level_names.<template>`: the key is the
template in lowercase with each run of other characters as one `_`, so "W4 Tower" and "W1 Tower" are both
`level_names.w_1_tower`, and "World 4 (secret)" is `level_names.world_1_secret`. The key holds the whole template in the
language, with the numbers where it wants them, such as "Torre M{1}" ("M" for Mundo). "Area 2", from the host, is
`level_names.area_1`. A name with no key, such as a project's own level, is shown as written. When the catalog gains a
name, add its key to every language file; a test lists the ones that are missing.

Left in English on purpose: data names (tilesets, backgrounds and actors, the first from
`crates/project/layers/layers.yaml`), file formats, and messages that come from the host.
