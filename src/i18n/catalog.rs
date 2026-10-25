//! Embedded translation catalogs (`locales/*.json`) and key lookup.
//!
//! Lookup is an in-memory hash read, safe for render paths. Missing keys fall
//! back to English, then to the key itself, so a gap is visible but never fatal.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use super::lang::Language;

const ZH_HANS: &str = include_str!("../../locales/zh-Hans.json");
const EN: &str = include_str!("../../locales/en.json");

type Messages = HashMap<String, String>;

static CURRENT: AtomicU8 = AtomicU8::new(1); // English until `set_language` runs
static CATALOGS: OnceLock<[Messages; 2]> = OnceLock::new();

pub(super) fn parse(source: &str) -> Messages {
    serde_json::from_str(source).expect("locale catalog must be a flat JSON string map")
}

fn catalogs() -> &'static [Messages; 2] {
    CATALOGS.get_or_init(|| {
        let mut loaded = [Messages::new(), Messages::new()];
        loaded[Language::ZhHans.index()] = parse(ZH_HANS);
        loaded[Language::En.index()] = parse(EN);
        loaded
    })
}

pub fn set_language(language: Language) {
    CURRENT.store(language.index() as u8, Ordering::Relaxed);
}

pub fn current() -> Language {
    Language::from_index(CURRENT.load(Ordering::Relaxed))
}

fn lookup(language: Language, key: &str) -> Option<&'static str> {
    catalogs()[language.index()].get(key).map(String::as_str)
}

/// Translate `key` for the current language.
pub fn t(key: &str) -> &str {
    lookup(current(), key)
        .or_else(|| lookup(Language::En, key))
        .unwrap_or_else(|| {
            eprintln!("missing translation key: {key}");
            key
        })
}

/// Translate `key` and substitute `{name}` placeholders.
#[allow(dead_code)] // first used by parameterized copy in the next migration step
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    substitute(t(key), args)
}

#[cfg_attr(not(test), allow(dead_code))]
fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
pub(super) fn placeholders(message: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = message;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        found.push(rest[start + 1..start + end].to_string());
        rest = &rest[start + end + 1..];
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn catalogs_define_identical_keys_and_placeholders() {
        let (zh, en) = (parse(ZH_HANS), parse(EN));
        let zh_keys: BTreeSet<_> = zh.keys().collect();
        let en_keys: BTreeSet<_> = en.keys().collect();
        assert_eq!(zh_keys, en_keys, "locale files must define the same keys");
        for (key, zh_text) in &zh {
            assert!(
                !zh_text.trim().is_empty() && !en[key].trim().is_empty(),
                "{key}"
            );
            assert_eq!(placeholders(zh_text), placeholders(&en[key]), "{key}");
        }
    }

    fn source_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                source_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Keys referenced as `t("…")` / `t_args("…", …)` string literals.
    fn used_keys() -> BTreeSet<String> {
        let mut files = Vec::new();
        source_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut keys = BTreeSet::new();
        for file in files {
            let text = std::fs::read_to_string(file).unwrap();
            for marker in ["t(\"", "t_args(\""] {
                let mut rest = text.as_str();
                while let Some(at) = rest.find(marker) {
                    let preceded_by_ident = rest[..at]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_');
                    let after = &rest[at + marker.len()..];
                    if !preceded_by_ident {
                        if let Some(end) = after.find('"') {
                            let key = &after[..end];
                            // Skip test fixtures and prose; real keys are dotted identifiers.
                            if key.contains('.')
                                && key
                                    .chars()
                                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
                            {
                                keys.insert(key.to_string());
                            }
                        }
                    }
                    rest = after;
                }
            }
        }
        keys
    }

    #[test]
    fn every_key_used_in_source_exists_and_every_key_is_used() {
        let defined: BTreeSet<String> = parse(EN).into_keys().collect();
        let used = used_keys();
        let missing: Vec<_> = used.difference(&defined).collect();
        let unused: Vec<_> = defined.difference(&used).collect();
        assert!(missing.is_empty(), "keys used but not defined: {missing:?}");
        assert!(unused.is_empty(), "keys defined but never used: {unused:?}");
    }

    #[test]
    fn lookup_falls_back_to_english_then_key_and_substitutes_arguments() {
        assert_eq!(lookup(Language::En, "tray.quit"), Some("Quit Notype"));
        assert_eq!(lookup(Language::ZhHans, "tray.quit"), Some("退出 Notype"));
        assert_eq!(lookup(Language::En, "no.such.key"), None);
        assert_eq!(
            substitute("{n} minutes ago", &[("n", "5")]),
            "5 minutes ago"
        );
        assert_eq!(placeholders("{b} and {a}"), vec!["a", "b"]);
    }
}
