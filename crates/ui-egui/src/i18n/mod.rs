//! The interface's translations: English and the four languages `MainWindow.cpp`'s `uiText` table carried, one
//! file per language, each pairing an English string with its translation. A string a table lacks reads in English.

mod es;
mod fr;
mod nl;
mod pt;

/// Every string the tables translate, in English.
pub const KEYS: &[&str] = &[
    "Project Hub",
    "Level Listing",
    "Tilesets",
    "Backgrounds",
    "Filesystem",
    "Tools",
    "Open Editor",
    "Hex Editor",
    "File",
    "Edit",
    "View",
    "Save Project",
    "Cut",
    "Copy",
    "Paste",
    "Language",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    Dutch,
    Portuguese,
    Spanish,
    French,
}

impl Language {
    pub const ALL: [Language; 5] = [Language::English, Language::Dutch, Language::Portuguese, Language::Spanish, Language::French];

    /// The code the preference is stored as.
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Dutch => "nl",
            Language::Portuguese => "pt",
            Language::Spanish => "es",
            Language::French => "fr",
        }
    }

    /// The language's name, as the preferences list it.
    pub fn name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Dutch => "Nederlands",
            Language::Portuguese => "Portugues",
            Language::Spanish => "Espanol",
            Language::French => "Francais",
        }
    }

    /// The language a stored code names; English for anything else.
    pub fn from_code(code: &str) -> Self {
        Self::ALL.into_iter().find(|language| language.code() == code).unwrap_or_default()
    }

    fn strings(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Language::English => &[],
            Language::Dutch => nl::STRINGS,
            Language::Portuguese => pt::STRINGS,
            Language::Spanish => es::STRINGS,
            Language::French => fr::STRINGS,
        }
    }

    /// `english` in this language.
    pub fn tr(self, english: &'static str) -> &'static str {
        self.strings().iter().find(|(key, _)| *key == english).map_or(english, |(_, text)| text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_translates_every_string() {
        for language in Language::ALL.into_iter().skip(1) {
            let keys: Vec<&str> = language.strings().iter().map(|(key, _)| *key).collect();
            assert_eq!(keys, KEYS, "{language:?}");
            assert!(language.strings().iter().all(|(_, text)| !text.is_empty()));
        }
    }

    #[test]
    fn strings_read_in_the_chosen_language_and_fall_back_to_english() {
        assert_eq!(Language::Dutch.tr("Tools"), "Gereedschap");
        assert_eq!(Language::French.tr("Hex Editor"), "Éditeur hexadécimal");
        assert_eq!(Language::Portuguese.tr("Filesystem"), "Sistema de arquivos");
        assert_eq!(Language::Spanish.tr("Language"), "Idioma");
        assert_eq!(Language::English.tr("Tools"), "Tools");
        assert_eq!(Language::French.tr("Not a key"), "Not a key");
        assert_eq!(Language::from_code("pt"), Language::Portuguese);
        assert_eq!(Language::from_code("xx"), Language::English);
        assert!(Language::ALL.iter().all(|l| Language::from_code(l.code()) == *l));
    }
}
