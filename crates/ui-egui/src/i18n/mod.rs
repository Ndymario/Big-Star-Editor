//! The interface's text, in `lang/*.json`: one flat file per language, each mapping a dotted key
//! (`<area>.<snake_case_text>`) to its text. Every file there is a language: its name without `.json` is its code, and its
//! `language.name` is how the languages are listed. `build.rs` finds them and embeds them, so this stays wasm-safe. A key
//! a language lacks reads in English, and a key English lacks reads as the key.
//!
//! `{name}` in a text is an argument, and a counted text has `.one` and `.other` keys with `{n}` for the number.

use std::{cell::Cell, collections::HashMap, fmt::Display, sync::OnceLock};

type Catalog = HashMap<&'static str, &'static str>;

include!(concat!(env!("OUT_DIR"), "/languages.rs"));

/// The key of a language's own name, in its own file.
const NAME_KEY: &str = "language.name";

/// A language the interface can be written in: one of the files in `lang/`. English is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Language(usize);

impl Language {
    pub const ENGLISH: Language = Language(0);

    /// Every language, English first. The index never leaves the array, and a `const` cannot use `get_mut`.
    #[allow(clippy::indexing_slicing)]
    pub const ALL: [Language; SOURCES.len()] = {
        let mut all = [Language::ENGLISH; SOURCES.len()];
        let mut i = 0;
        while i < all.len() {
            all[i] = Language(i);
            i += 1;
        }
        all
    };

    /// The code the preference is stored as: the language file's name.
    pub fn code(self) -> &'static str {
        SOURCES.get(self.0).map_or("en", |(code, _)| code)
    }

    /// The language's name, as the preferences list it: its own file's `language.name`, else its code.
    pub fn name(self) -> &'static str {
        self.catalog().get(NAME_KEY).copied().unwrap_or_else(|| self.code())
    }

    /// The language a stored code names; English for anything else.
    pub fn from_code(code: &str) -> Self {
        Self::ALL.into_iter().find(|language| language.code() == code).unwrap_or_default()
    }

    fn catalog(self) -> &'static Catalog {
        static CATALOGS: OnceLock<Vec<Catalog>> = OnceLock::new();
        let catalogs = CATALOGS.get_or_init(|| SOURCES.iter().map(|(_, json)| parse(json)).collect());
        catalogs.get(self.0).or_else(|| catalogs.first()).unwrap_or_else(|| empty())
    }
}

fn empty() -> &'static Catalog {
    static EMPTY: OnceLock<Catalog> = OnceLock::new();
    EMPTY.get_or_init(Catalog::new)
}

/// A language file's texts. A file that does not parse reads as empty, and the tests keep that from happening.
fn parse(json: &str) -> Catalog {
    let texts: HashMap<String, String> = serde_json::from_str(json).unwrap_or_default();
    let leak = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    texts.into_iter().map(|(key, text)| (leak(key), leak(text))).collect()
}

thread_local! {
    // The interface is drawn on one thread, so the language is that thread's; tests that choose one do not meet.
    static CURRENT: Cell<Language> = const { Cell::new(Language::ENGLISH) };
}

/// The language the interface is written in.
pub fn current() -> Language {
    CURRENT.with(Cell::get)
}

pub fn set(language: Language) {
    CURRENT.with(|current| current.set(language));
}

/// The text of `key` in the current language, else in English, else the key itself.
pub fn text(key: &'static str) -> &'static str {
    lookup(current(), key).or_else(|| lookup(Language::ENGLISH, key)).unwrap_or(key)
}

/// The text of a key known only at run time, in the current language, else in English; `None` when there is none.
pub fn find(key: &str) -> Option<&'static str> {
    lookup(current(), key).or_else(|| lookup(Language::ENGLISH, key))
}

fn lookup(language: Language, key: &str) -> Option<&'static str> {
    language.catalog().get(key).copied()
}

/// The text that counts `n` things: `one_key` for one where the language has it, else `other_key`.
pub fn counted(one_key: &'static str, other_key: &'static str, n: i64) -> &'static str {
    let in_language = |language| if n == 1 { lookup(language, one_key).or_else(|| lookup(language, other_key)) } else { lookup(language, other_key) };
    in_language(current()).or_else(|| in_language(Language::ENGLISH)).unwrap_or(other_key)
}

/// `template` with each `{name}` replaced by its argument. A name with no argument, or a `{` never closed, stays as
/// written, so a translation may reorder or leave out arguments.
pub fn format(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, after)) = rest.split_once('{') {
        result.push_str(before);
        let Some((name, tail)) = after.split_once('}') else {
            result.push('{');
            result.push_str(after);
            return result;
        };
        match args.iter().find(|(argument, _)| *argument == name) {
            Some((_, value)) => result.push_str(&value.to_string()),
            None => {
                result.push('{');
                result.push_str(name);
                result.push('}');
            }
        }
        rest = tail;
    }
    result.push_str(rest);
    result
}

/// A number with its thousands grouped, as `tr_n!` writes it.
pub fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut result = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    if n < 0 { format!("-{result}") } else { result }
}

/// The text of a key: `tr!("files.open_folder")` is a `&'static str`, and with arguments, `tr!("app.delete_name",
/// name = file)` or just `name`, is the `String` with each `{name}` filled in.
macro_rules! tr {
    ($key:literal) => {
        $crate::i18n::text($key)
    };
    ($key:literal, $($name:ident $(= $value:expr)?),+ $(,)?) => {
        $crate::i18n::format($crate::i18n::text($key), &[$((stringify!($name), &$crate::i18n::tr_arg!($name $(, $value)?) as &dyn ::std::fmt::Display)),+])
    };
}

/// The text that counts something: `tr_n!("files.count", n)` is `files.count.one` for one and `files.count.other`
/// otherwise, with `{n}` the number. More arguments are given as to `tr!`.
macro_rules! tr_n {
    ($key:literal, $n:expr $(, $name:ident $(= $value:expr)?)* $(,)?) => {{
        let n = ($n) as i64;
        $crate::i18n::format(
            $crate::i18n::counted(concat!($key, ".one"), concat!($key, ".other"), n),
            &[("n", &$crate::i18n::grouped(n) as &dyn ::std::fmt::Display), $((stringify!($name), &$crate::i18n::tr_arg!($name $(, $value)?) as &dyn ::std::fmt::Display)),*],
        )
    }};
}

/// A key written where no text can be asked for yet, such as a `const` table; `i18n::text` turns it into words where
/// it is shown.
macro_rules! key {
    ($key:literal) => {
        $key
    };
}

macro_rules! tr_arg {
    ($name:ident) => {
        $name
    };
    ($name:ident, $value:expr) => {
        $value
    };
}

pub(crate) use {key, tr, tr_arg, tr_n};

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::Path,
    };

    use super::*;

    fn read(json: &str) -> BTreeMap<String, String> {
        serde_json::from_str(json).expect("a language file is a flat object of texts")
    }

    /// The names of the `{arguments}` a text has.
    fn arguments(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            found.insert(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        found
    }

    fn sources(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("the source directory reads") {
            let path = entry.expect("a directory entry reads").path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") && !path.ends_with("i18n/mod.rs") {
                found.push(path);
            }
        }
    }

    /// Every key the code asks for with a literal, and whether it is a counted one.
    fn keys_asked_for() -> BTreeMap<String, bool> {
        let mut files = Vec::new();
        sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut keys = BTreeMap::new();
        for file in files {
            let code = std::fs::read_to_string(&file).expect("a source file reads");
            for (call, counted) in [("tr!(", false), ("tr_n!(", true), ("key!(", false)] {
                for (at, _) in code.match_indices(call) {
                    if code[..at].chars().next_back().is_some_and(|before| before.is_alphanumeric() || before == '_') {
                        continue;
                    }
                    // A key is a literal, and rustfmt may have put it on the line after the `(`.
                    let Some(literal) = code[at + call.len()..].trim_start().strip_prefix('"') else { continue };
                    let end = literal.find('"').expect("a key is closed");
                    keys.insert(literal[..end].to_string(), counted);
                }
            }
        }
        keys
    }

    fn english() -> BTreeMap<String, String> {
        read(SOURCES[0].1)
    }

    #[test]
    fn every_key_the_code_asks_for_is_written_in_english() {
        let english = english();
        for (key, counted) in keys_asked_for() {
            if counted {
                assert!(english.contains_key(&format!("{key}.one")) && english.contains_key(&format!("{key}.other")), "{key}");
            } else {
                assert!(english.contains_key(&key), "{key}");
            }
        }
    }

    #[test]
    fn every_english_text_is_asked_for() {
        let asked = keys_asked_for();
        for key in english().keys().filter(|key| *key != NAME_KEY && !key.starts_with("level_names.")) {
            let stem = key.strip_suffix(".one").or_else(|| key.strip_suffix(".other")).unwrap_or(key);
            assert!(asked.contains_key(key) || asked.get(stem) == Some(&true), "{key} is never asked for");
        }
    }

    #[test]
    fn english_comes_first_and_every_language_names_itself() {
        assert_eq!(SOURCES[0].0, "en");
        assert_eq!(Language::ENGLISH.code(), "en");
        let codes: Vec<&str> = Language::ALL.iter().map(|language| language.code()).collect();
        assert!(codes.windows(2).all(|pair| pair[0] < pair[1] || pair[0] == "en"), "{codes:?}");
        for (language, (code, json)) in Language::ALL.into_iter().zip(SOURCES) {
            let name = read(json).get(NAME_KEY).cloned();
            assert!(name.as_deref().is_some_and(|name| !name.is_empty()), "lang/{code}.json has no {NAME_KEY}");
            assert_eq!(Some(language.name()), name.as_deref());
        }
    }

    #[test]
    fn every_language_has_every_english_text_and_no_argument_of_its_own() {
        let english = english();
        for (code, json) in SOURCES.into_iter().skip(1) {
            let texts = read(json);
            let missing: Vec<_> = english.keys().filter(|key| !texts.contains_key(*key)).collect();
            assert!(missing.is_empty(), "missing from lang/{code}.json: {missing:?}");
            for (key, text) in &texts {
                let original = english.get(key).unwrap_or_else(|| panic!("{key} is in lang/{code}.json but not in en.json"));
                assert!(!text.is_empty(), "{code}: {key} is empty");
                assert!(arguments(text).is_subset(&arguments(original)), "{code}: {key} has an argument English lacks");
            }
        }
    }

    /// The names in the built-in level catalog, which the host sends as they are and the level list translates by key.
    fn catalog_names() -> BTreeSet<String> {
        let yaml =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../project/levels/levels.yaml")).expect("the level catalog reads");
        let mut names: BTreeSet<String> = yaml
            .lines()
            .filter_map(|line| {
                Some(line.trim().strip_prefix("- name: ").or_else(|| line.trim().strip_prefix("name: "))?.trim_matches('"').to_string())
            })
            .collect();
        // The group the host adds for a module's own levels.
        names.insert("Custom Levels".to_string());
        // And the name it gives an area of a level with more than one.
        names.insert("Area 1".to_string());
        names
    }

    #[test]
    fn the_catalogs_level_names_are_translated_by_key() {
        let english = english();
        let names = catalog_names();
        assert!(names.len() > 50, "the catalog's names were not found");
        for name in &names {
            // A name that is only numbers and a letter or two of a code ("1-1", "1-A") reads the same everywhere.
            let (template, _) = crate::level_list::numbered(name);
            if template.chars().collect::<Vec<_>>().windows(2).any(|pair| pair.iter().all(|c| c.is_alphabetic())) {
                let key = crate::level_list::name_key(&template);
                assert_eq!(english.get(&key), Some(&template), "{key}");
            }
        }
        let keys: BTreeSet<String> = names.iter().map(|name| crate::level_list::name_key(&crate::level_list::numbered(name).0)).collect();
        for key in english.keys().filter(|key| key.starts_with("level_names.")) {
            assert!(keys.contains(key), "{key} names no level in the catalog");
        }
    }

    #[test]
    fn the_files_are_sorted_by_key() {
        for (code, json) in SOURCES {
            let keys: Vec<&str> = json.lines().filter_map(|line| line.trim().strip_prefix('"')?.split("\": ").next()).collect();
            assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "unsorted or repeated key in lang/{code}.json");
            assert_eq!(keys.len(), read(json).len(), "{code}");
        }
    }

    #[test]
    fn arguments_take_their_places() {
        let name = "Mario";
        assert_eq!(format("Hello {name}!", &[("name", &name)]), "Hello Mario!");
        assert_eq!(format("{b} before {a}", &[("a", &1), ("b", &2)]), "2 before 1");
        assert_eq!(format("{other} {name", &[("name", &name)]), "{other} {name");
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(grouped(-12), "-12");
    }

    #[test]
    fn texts_read_in_the_current_language_and_fall_back_to_english() {
        let portuguese = Language::from_code("pt");
        assert_ne!(portuguese, Language::ENGLISH);
        assert_eq!(portuguese.code(), "pt");
        set(portuguese);
        assert_eq!(text("preferences.language"), "Idioma");
        assert_eq!(text("no.such.key"), "no.such.key");
        set(Language::ENGLISH);
        assert_eq!(text("preferences.language"), "Language");
        assert_eq!(Language::from_code("xx"), Language::ENGLISH);
        assert!(Language::ALL.iter().all(|l| Language::from_code(l.code()) == *l));
    }
}
