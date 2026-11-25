//! The languages of the window: Fluent message catalogs built into the program, one folder per
//! language under `crates/gui/i18n`. English is the source and the fallback: a message missing
//! in another language shows in English. A new language is a folder with the same files and
//! one line in `LANGUAGES`.
//!
//! The language is kept per thread: the window draws on one thread, which `set_language` sets,
//! and every other thread (the tests among them) stays in English.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub use fluent_bundle::FluentValue;

/// The catalog files of a language, in `crates/gui/i18n/<code>/`.
macro_rules! catalog {
    ($code:literal) => {
        &[
            include_str!(concat!("../i18n/", $code, "/menu.ftl")),
            include_str!(concat!("../i18n/", $code, "/window.ftl")),
            include_str!(concat!("../i18n/", $code, "/data.ftl")),
            include_str!(concat!("../i18n/", $code, "/syntax.ftl")),
            include_str!(concat!("../i18n/", $code, "/output.ftl")),
            include_str!(concat!("../i18n/", $code, "/settings.ftl")),
        ]
    };
}

pub struct Language {
    /// The language tag, also the folder name.
    pub code: &'static str,
    /// The name of the language in that language, as Settings lists it.
    pub name: &'static str,
    files: &'static [&'static str],
}

/// The languages of the window, English first.
pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        name: "English",
        files: catalog!("en"),
    },
    Language {
        code: "es",
        name: "Español",
        files: catalog!("es"),
    },
];

thread_local! {
    static CURRENT: Cell<usize> = const { Cell::new(0) };
}

fn bundles() -> &'static [Bundle] {
    static BUNDLES: OnceLock<Vec<Bundle>> = OnceLock::new();
    BUNDLES.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|l| {
                let id = l.code.parse().unwrap_or_default();
                let mut bundle = FluentBundle::new_concurrent(vec![id]);
                // the marks that isolate an argument's direction show as boxes in egui
                bundle.set_use_isolating(false);
                for file in l.files {
                    // the tests check that every catalog parses; at run time a bad entry is
                    // only missing and shows in English
                    let resource =
                        FluentResource::try_new(file.to_string()).unwrap_or_else(|e| e.0);
                    bundle.add_resource_overriding(resource);
                }
                bundle
            })
            .collect()
    })
}

/// The window's language from the setting: a language code, or empty for the system's language.
pub fn set_language(setting: &str) {
    let code = match setting {
        "" => sys_locale::get_locale().unwrap_or_default(),
        code => code.to_string(),
    };
    CURRENT.set(language_index(&code));
}

/// The language that fits a tag such as `es-ES`, `es_MX.UTF-8` or `pt-BR`: the same tag, else
/// the same language, else English.
fn language_index(tag: &str) -> usize {
    let tag = tag.split('.').next().unwrap_or("").replace('_', "-");
    let primary = |s: &str| s.split('-').next().unwrap_or("").to_ascii_lowercase();
    let codes = LANGUAGES.iter().map(|l| l.code);
    let exact = codes.clone().position(|c| c.eq_ignore_ascii_case(&tag));
    exact
        .or_else(|| codes.clone().position(|c| primary(c) == primary(&tag)))
        .unwrap_or(0)
}

/// A message in the window's language, else in English, else the key. Each message is
/// formatted once per language and kept.
pub fn t(key: &'static str) -> &'static str {
    static KEPT: OnceLock<Mutex<HashMap<(usize, &'static str), &'static str>>> = OnceLock::new();
    let language = CURRENT.get();
    let mut kept = KEPT
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    kept.entry((language, key))
        .or_insert_with(|| Box::leak(format(language, key, None).into_boxed_str()))
}

/// A message with arguments, such as `t_args("key", &[("file", name.into())])`.
pub fn t_args(key: &str, args: &[(&str, FluentValue)]) -> String {
    let mut fluent = FluentArgs::new();
    for (name, value) in args {
        fluent.set(*name, value.clone());
    }
    format(CURRENT.get(), key, Some(&fluent))
}

type Bundle = FluentBundle<FluentResource>;

fn format(language: usize, key: &str, args: Option<&FluentArgs>) -> String {
    format_in([&bundles()[language], &bundles()[0]], key, args)
}

/// The message in the first bundle that has it, else the key.
fn format_in(bundles: [&Bundle; 2], key: &str, args: Option<&FluentArgs>) -> String {
    for bundle in bundles {
        if let Some(pattern) = bundle.get_message(key).and_then(|m| m.value()) {
            let mut errors = vec![];
            return bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
        }
    }
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// The messages of a catalog file and the arguments each one uses.
    fn messages(file: &str) -> Vec<(String, BTreeSet<String>)> {
        let mut list: Vec<(String, BTreeSet<String>)> = vec![];
        for line in file.lines() {
            let starts_message = line.starts_with(|c: char| c.is_ascii_alphabetic());
            if starts_message && let Some((id, _)) = line.split_once(" =") {
                list.push((id.trim().to_string(), BTreeSet::new()));
            }
            if let Some((_, args)) = list.last_mut() {
                let mut rest = line;
                while let Some(at) = rest.find('$') {
                    rest = &rest[at + 1..];
                    let end = rest
                        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                        .unwrap_or(rest.len());
                    args.insert(rest[..end].to_string());
                }
            }
        }
        list
    }

    fn catalog(language: &Language) -> Vec<(String, BTreeSet<String>)> {
        language.files.iter().flat_map(|f| messages(f)).collect()
    }

    #[test]
    fn every_catalog_parses_without_errors_or_doubles() {
        for language in LANGUAGES {
            let id = language.code.parse().unwrap();
            let mut bundle = FluentBundle::new_concurrent(vec![id]);
            for file in language.files {
                let resource = FluentResource::try_new(file.to_string());
                let resource = resource.unwrap_or_else(|e| panic!("{}: {:?}", language.code, e.1));
                let added = bundle.add_resource(resource);
                assert!(added.is_ok(), "{}: {added:?}", language.code);
            }
        }
    }

    #[test]
    fn every_translation_has_an_english_message_with_the_same_arguments() {
        let english: HashMap<_, _> = catalog(&LANGUAGES[0]).into_iter().collect();
        for language in &LANGUAGES[1..] {
            for (key, args) in catalog(language) {
                let source = english.get(&key);
                let source =
                    source.unwrap_or_else(|| panic!("{}: {key} not in English", language.code));
                assert_eq!(&args, source, "{}: arguments of {key}", language.code);
            }
        }
    }

    /// Every `t("key")` and `t_args("key"` in the source names a message of the English catalog;
    /// a key in a variable is not seen, so the calls name their keys.
    #[test]
    fn every_key_in_the_code_is_in_the_english_catalog() {
        let english: HashMap<_, _> = catalog(&LANGUAGES[0]).into_iter().collect();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![];
        let mut dirs = vec![dir];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    files.push(path);
                }
            }
        }
        let mut used = 0;
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            for call in ["t(", "t_args("] {
                for (at, _) in source.match_indices(call) {
                    // `t(` alone, not the end of a longer name such as `set(`
                    let before = source[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    // a key may start on the next line
                    let rest = source[at + call.len()..].trim_start();
                    let Some(rest) = rest.strip_prefix('"') else {
                        continue;
                    };
                    let key = &rest[..rest.find('"').unwrap()];
                    // not a key: the examples in comments and the strings of this test
                    let is_key = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
                    if key == "key" || key.is_empty() || !key.chars().all(is_key) {
                        continue;
                    }
                    used += 1;
                    assert!(english.contains_key(key), "{}: {key}", path.display());
                }
            }
        }
        assert!(used > 20, "only {used} keys found");
    }

    #[test]
    fn a_missing_translation_shows_english_and_a_missing_key_itself() {
        let bundle = |code: &str, text: &str| {
            let mut bundle: Bundle = FluentBundle::new_concurrent(vec![code.parse().unwrap()]);
            bundle
                .add_resource(FluentResource::try_new(text.into()).unwrap())
                .unwrap();
            bundle
        };
        let english = bundle("en", "a = Apple\nb = Banana\n");
        let spanish = bundle("es", "a = Manzana\n");
        assert_eq!(format_in([&spanish, &english], "a", None), "Manzana");
        assert_eq!(format_in([&spanish, &english], "b", None), "Banana");
        assert_eq!(format_in([&spanish, &english], "c", None), "c");
    }

    #[test]
    fn the_system_language_picks_the_closest_window_language() {
        assert_eq!(language_index("es_MX.UTF-8"), 1);
        assert_eq!(language_index("es-ES"), 1);
        assert_eq!(language_index("ES"), 1);
        assert_eq!(language_index("en-GB"), 0);
        assert_eq!(language_index("et-EE"), 0);
        assert_eq!(language_index(""), 0);
    }

    #[test]
    fn spanish_names_the_menus_in_spanish() {
        // the language is per thread, so this does not reach the other tests
        set_language("es");
        assert_eq!(t("menu-file"), "Archivo");
        assert_eq!(t("menu-analyze"), "Analizar");
        assert_eq!(
            t_args("status-saved-syntax", &[("file", "a.sps".into())]),
            "sintaxis guardada en a.sps"
        );
        set_language("en");
        assert_eq!(t("menu-file"), "File");
    }
}
