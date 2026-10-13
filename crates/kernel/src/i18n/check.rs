//! The parity rules every server catalogue is held to. `cargo xtask i18n`
//! runs them over each `i18n/` directory under `crates/`; each catalogue's
//! unit test runs them over its embedded files.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Locale,
    catalogue::{CATEGORY_NAMES, COUNT, flatten, placeholders, plural_categories},
};

/// One message of one locale: a single text, or plural forms by category.
#[derive(Debug)]
enum Entry {
    Single(String),
    Forms(BTreeMap<String, String>),
}

/// The problems of one catalogue directory, given as file name → contents;
/// empty when it is at parity. `name` prefixes every problem.
///
/// Rules: exactly one `<tag>.json` per shipped locale; each a JSON object of
/// nested strings; every locale has the same messages as `en` (plural forms
/// `<id>_<category>` grouped as message `<id>`); a plural message has `_one`
/// and `_other` forms and none its locale's CLDR rules do not use; no value
/// is empty; every `{{placeholder}}` is closed; and each message has the
/// same placeholders as in `en` (for plural messages, those of all forms
/// together, `count` aside).
pub fn check_catalogue(name: &str, files: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    let expected: BTreeSet<String> = Locale::ALL
        .iter()
        .map(|locale| format!("{}.json", locale.tag()))
        .collect();
    for file in files.keys() {
        if !expected.contains(file) {
            problems.push(format!(
                "{name}: {file} is not a shipped locale (en, pt-PT, es-ES, fr)"
            ));
        }
    }

    let mut locales = Vec::new();
    for locale in Locale::ALL {
        let file = format!("{}.json", locale.tag());
        let Some(source) = files.get(&file) else {
            problems.push(format!("{name}: missing {file}"));
            continue;
        };
        match flatten(source) {
            Ok(flat) => {
                let path = format!("{name}/{file}");
                let entries = group(&flat);
                check_values(&path, locale, &entries, &mut problems);
                locales.push((locale, path, entries));
            }
            Err(error) => problems.push(format!("{name}/{file}: {error}")),
        }
    }

    let Some((_, _, reference)) = locales.iter().find(|(locale, ..)| *locale == Locale::En) else {
        return problems;
    };
    for (locale, path, entries) in &locales {
        if *locale == Locale::En {
            continue;
        }
        for (id, expected) in reference {
            let Some(entry) = entries.get(id) else {
                problems.push(format!("{path}: missing \"{id}\""));
                continue;
            };
            match (expected, entry) {
                (Entry::Single(_), Entry::Forms(_)) => problems.push(format!(
                    "{path}: \"{id}\" has plural forms but is a single message in en.json"
                )),
                (Entry::Forms(_), Entry::Single(_)) => problems.push(format!(
                    "{path}: \"{id}\" is a single message but has plural forms in en.json"
                )),
                _ => {
                    let want = names(expected);
                    let have = names(entry);
                    for missing in want.difference(&have) {
                        problems.push(format!(
                            "{path}: \"{id}\" lacks the placeholder {{{{{missing}}}}}"
                        ));
                    }
                    for extra in have.difference(&want) {
                        problems.push(format!(
                            "{path}: \"{id}\" has the placeholder {{{{{extra}}}}}, which en.json does not"
                        ));
                    }
                }
            }
        }
        for id in entries.keys().filter(|id| !reference.contains_key(*id)) {
            problems.push(format!("{path}: \"{id}\" is not in en.json"));
        }
    }
    problems
}

/// Flat keys grouped into messages: `<id>_<category>` keys become the forms
/// of message `<id>`.
fn group(flat: &std::collections::HashMap<String, String>) -> BTreeMap<String, Entry> {
    let mut entries = BTreeMap::new();
    for (key, text) in flat {
        let form = CATEGORY_NAMES.iter().find_map(|category| {
            key.strip_suffix(category)
                .and_then(|rest| rest.strip_suffix('_'))
                .filter(|id| !id.is_empty())
                .map(|id| (id, *category))
        });
        match form {
            Some((id, category)) => {
                let entry = entries
                    .entry(id.to_owned())
                    .or_insert_with(|| Entry::Forms(BTreeMap::new()));
                if let Entry::Forms(forms) = entry {
                    forms.insert(category.to_owned(), text.clone());
                } else {
                    // A single message and plural forms under one id: keep
                    // the forms so the mismatch with en.json is reported.
                    *entry = Entry::Forms(BTreeMap::from([(category.to_owned(), text.clone())]));
                }
            }
            None => {
                entries
                    .entry(key.clone())
                    .or_insert_with(|| Entry::Single(text.clone()));
            }
        }
    }
    entries
}

/// Empty values, unclosed placeholders and plural forms of one locale.
fn check_values(
    path: &str,
    locale: Locale,
    entries: &BTreeMap<String, Entry>,
    problems: &mut Vec<String>,
) {
    let categories = plural_categories(locale);
    for (id, entry) in entries {
        let texts: Vec<(String, &String)> = match entry {
            Entry::Single(text) => vec![(id.clone(), text)],
            Entry::Forms(forms) => {
                for required in ["one", "other"] {
                    if !forms.contains_key(required) {
                        problems.push(format!(
                            "{path}: \"{id}\" lacks the form \"{id}_{required}\""
                        ));
                    }
                }
                for category in forms.keys() {
                    if !categories.contains(&category.as_str()) {
                        problems.push(format!(
                            "{path}: \"{id}_{category}\" is a plural form {locale} does not use ({})",
                            categories.join(", ")
                        ));
                    }
                }
                forms
                    .iter()
                    .map(|(category, text)| (format!("{id}_{category}"), text))
                    .collect()
            }
        };
        for (key, text) in texts {
            if text.trim().is_empty() {
                problems.push(format!("{path}: \"{key}\" is empty"));
            }
            if let Err(offset) = placeholders(text) {
                problems.push(format!(
                    "{path}: \"{key}\" has an unclosed {{{{ at byte {offset}"
                ));
            }
        }
    }
}

/// A message's placeholder names; for plural forms, of all forms together
/// without `count` (the singular may spell the number out).
fn names(entry: &Entry) -> BTreeSet<String> {
    let texts: Vec<&String> = match entry {
        Entry::Single(text) => vec![text],
        Entry::Forms(forms) => forms.values().collect(),
    };
    texts
        .into_iter()
        .flat_map(|text| placeholders(text).unwrap_or_default())
        .filter(|name| !matches!(entry, Entry::Forms(_)) || *name != COUNT)
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(en: &str, pt: &str, es: &str, fr: &str) -> BTreeMap<String, String> {
        [("en", en), ("pt-PT", pt), ("es-ES", es), ("fr", fr)]
            .into_iter()
            .map(|(tag, text)| (format!("{tag}.json"), text.to_owned()))
            .collect()
    }

    const EN: &str = r#"{"errors": {"too_long": "At most {{max}}."}, "items_one": "One item", "items_other": "{{count}} items"}"#;

    #[test]
    fn a_catalogue_at_parity_passes() {
        let pt = r#"{"errors": {"too_long": "No máximo {{max}}."}, "items_one": "Um item", "items_many": "{{count}} de itens", "items_other": "{{count}} itens"}"#;
        let es = r#"{"errors": {"too_long": "Como máximo {{max}}."}, "items_one": "Un elemento", "items_other": "{{count}} elementos"}"#;
        let fr = r#"{"errors": {"too_long": "Au plus {{max}}."}, "items_one": "{{count}} élément", "items_other": "{{count}} éléments"}"#;
        assert_eq!(
            check_catalogue("t", &files(EN, pt, es, fr)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn reports_every_kind_of_drift() {
        let pt = r#"{"errors": {"too_long": "No máximo."}, "items_one": "Um item", "items_other": "{{count}} itens"}"#;
        let es =
            r#"{"errors": {"too_long": " "}, "items_other": "{{count}} elementos", "extra": "x"}"#;
        let fr = r#"{"errors": {"too_long": "Au plus {{max}} {{min}}."}, "items_one": "x", "items_few": "y", "items_other": "{{count"}"#;
        let mut all = files(EN, pt, es, fr);
        all.insert("de.json".to_owned(), "{}".to_owned());
        let problems = check_catalogue("t", &all);
        for expected in [
            "t: de.json is not a shipped locale",
            "t/pt-PT.json: \"errors.too_long\" lacks the placeholder {{max}}",
            "t/es-ES.json: \"errors.too_long\" is empty",
            "t/es-ES.json: \"items\" lacks the form \"items_one\"",
            "t/es-ES.json: \"extra\" is not in en.json",
            "t/fr.json: \"items_few\" is a plural form fr does not use",
            "t/fr.json: \"items_other\" has an unclosed {{",
            "t/fr.json: \"errors.too_long\" has the placeholder {{min}}, which en.json does not",
        ] {
            assert!(
                problems.iter().any(|p| p.starts_with(expected)),
                "missing `{expected}` in {problems:#?}"
            );
        }
    }

    #[test]
    fn missing_files_and_bad_json() {
        let mut all = files(EN, "{", EN, EN);
        all.remove("fr.json");
        let problems = check_catalogue("t", &all);
        assert!(
            problems.contains(&"t: missing fr.json".to_owned()),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.starts_with("t/pt-PT.json: invalid JSON")),
            "{problems:?}"
        );
    }

    #[test]
    fn single_and_plural_mismatch() {
        let en = r#"{"a": "x", "b_one": "1", "b_other": "n"}"#;
        let other = r#"{"a_one": "x", "a_other": "y", "b": "n"}"#;
        let problems = check_catalogue("t", &files(en, other, en, en));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("\"a\" has plural forms")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("\"b\" is a single message")),
            "{problems:?}"
        );
    }
}
