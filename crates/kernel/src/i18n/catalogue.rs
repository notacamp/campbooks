//! Message catalogues: one JSON file per locale, embedded at compile time,
//! rendered with arguments and CLDR plural forms.

use std::{collections::HashMap, sync::OnceLock};

use icu_locale_core::locale;
use icu_plurals::{PluralCategory, PluralRules};
use serde_json::Value;

use super::Locale;

/// The argument that selects a message's plural form.
pub const COUNT: &str = "count";

/// The messages of one namespace (the kernel, or a module) in every shipped
/// locale. Declare one per crate with [`catalogue!`](crate::catalogue):
///
/// ```ignore
/// pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!("workspaces");
/// ```
///
/// The files are i18next JSON, the client's format: nested objects of
/// strings (a message id is the dotted path, `errors.last_owner`),
/// `{{name}}` placeholders, and plural forms as `<id>_one`, `<id>_other`
/// (and `_many`, … where the locale has them) chosen by the `count`
/// argument. They are parsed on first use; `cargo xtask i18n` keeps them
/// valid and at parity.
pub struct Catalogue {
    namespace: &'static str,
    sources: [&'static str; 4],
    parsed: OnceLock<[Messages; 4]>,
}

/// One locale's messages, by dotted key.
type Messages = HashMap<String, String>;

impl std::fmt::Debug for Catalogue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalogue")
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl Catalogue {
    /// A catalogue from the JSON text of each locale, in
    /// [`Locale::ALL`] order (`en`, `pt-PT`, `es-ES`, `fr`). Prefer the
    /// [`catalogue!`](crate::catalogue) macro, which embeds the crate's
    /// `i18n/<locale>.json` files.
    pub const fn new(namespace: &'static str, sources: [&'static str; 4]) -> Self {
        Self {
            namespace,
            sources,
            parsed: OnceLock::new(),
        }
    }

    /// The namespace: `kernel` or the module's name.
    pub const fn namespace(&self) -> &'static str {
        self.namespace
    }

    /// The JSON text of `locale`'s file.
    pub(crate) const fn source(&self, locale: Locale) -> &'static str {
        self.sources[locale.index()]
    }

    fn messages(&self, locale: Locale) -> &Messages {
        let parsed = self.parsed.get_or_init(|| {
            Locale::ALL.map(|locale| {
                flatten(self.source(locale)).unwrap_or_else(|error| {
                    // `cargo xtask i18n` rejects invalid files; never panic here.
                    tracing::error!(
                        namespace = self.namespace,
                        locale = %locale,
                        %error,
                        "message catalogue is not valid; using English"
                    );
                    Messages::new()
                })
            })
        });
        &parsed[locale.index()]
    }

    /// Message `key` in `locale` with `args` filled in. A `count` argument
    /// picks the plural form by the locale's CLDR rules (falling back to
    /// `_other`). A message missing in `locale` is taken from English; one
    /// missing in English too renders as its key (and logs a warning), so
    /// rendering never fails.
    ///
    /// ```ignore
    /// let subject = MESSAGES.t(locale, "emails.invitation.subject", &[("workspace", name.into())]);
    /// ```
    pub fn t(&self, locale: Locale, key: &str, args: &[(&str, Arg)]) -> String {
        self.get(locale, key, args).unwrap_or_else(|| {
            tracing::warn!(
                namespace = self.namespace,
                key,
                "message missing from the catalogue"
            );
            key.to_owned()
        })
    }

    /// Like [`t`](Self::t), but `None` when the message is in neither
    /// `locale` nor English.
    pub fn get(&self, locale: Locale, key: &str, args: &[(&str, Arg)]) -> Option<String> {
        let count = args.iter().find_map(|(name, value)| match value {
            Arg::Integer(n) if *name == COUNT => Some(*n),
            _ => None,
        });
        let fallback = (locale != Locale::En).then_some(Locale::En);
        std::iter::once(locale).chain(fallback).find_map(|locale| {
            lookup(self.messages(locale), locale, key, count).map(|text| interpolate(text, args))
        })
    }

    /// An error reason of this catalogue: the stable `code` a client matches
    /// on, explained by the message `errors.<code>`.
    pub const fn reason(&'static self, code: &'static str) -> Reason {
        Reason {
            code,
            catalogue: self,
            args: Vec::new(),
        }
    }

    /// The parity problems of the embedded files (see
    /// [`check_catalogue`](super::check_catalogue)); empty when they are fine.
    /// Each catalogue's crate runs it in a unit test.
    pub fn check(&self) -> Vec<String> {
        let files = Locale::ALL
            .into_iter()
            .map(|locale| {
                (
                    format!("{}.json", locale.tag()),
                    self.source(locale).to_owned(),
                )
            })
            .collect();
        super::check_catalogue(self.namespace, &files)
    }
}

/// A message argument: text or an integer. A `count` integer selects the
/// plural form. Values are inserted as they are; whoever renders into HTML
/// escapes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// Text, inserted verbatim.
    Text(String),
    /// An integer, written without grouping separators.
    Integer(i64),
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => f.write_str(text),
            Self::Integer(n) => write!(f, "{n}"),
        }
    }
}

impl From<&str> for Arg {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Arg {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for Arg {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for Arg {
    fn from(value: i32) -> Self {
        Self::Integer(value.into())
    }
}

impl From<u16> for Arg {
    fn from(value: u16) -> Self {
        Self::Integer(value.into())
    }
}

impl From<u32> for Arg {
    fn from(value: u32) -> Self {
        Self::Integer(value.into())
    }
}

impl From<usize> for Arg {
    fn from(value: usize) -> Self {
        i64::try_from(value).map_or_else(|_| Self::Text(value.to_string()), Self::Integer)
    }
}

impl From<u64> for Arg {
    fn from(value: u64) -> Self {
        i64::try_from(value).map_or_else(|_| Self::Text(value.to_string()), Self::Integer)
    }
}

/// Why an action failed, for the client: a stable machine-readable
/// [`code`](Self::code) (the problem's `code`, never localized) and the
/// catalogue message `errors.<code>` that explains it (the problem's
/// `detail`, in the caller's locale). Built with
/// [`Catalogue::reason`]; arguments with [`arg`](Self::arg).
#[derive(Debug, Clone)]
pub struct Reason {
    code: &'static str,
    catalogue: &'static Catalogue,
    args: Vec<(&'static str, Arg)>,
}

impl Reason {
    /// The same reason with argument `name` set to `value`.
    #[must_use]
    pub fn arg(mut self, name: &'static str, value: impl Into<Arg>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    /// The stable code, e.g. `last_owner`.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// The explanation in `locale`.
    pub fn render(&self, locale: Locale) -> String {
        self.catalogue
            .t(locale, &format!("errors.{}", self.code), &self.args)
    }
}

/// The code; never the arguments (they may echo input).
impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

/// `key` in one locale's messages: with a count, its plural form, then
/// `_other`, then the bare key; without, the bare key, then `_other`.
fn lookup<'m>(
    messages: &'m Messages,
    locale: Locale,
    key: &str,
    count: Option<i64>,
) -> Option<&'m str> {
    let form = |category: &str| messages.get(&format!("{key}_{category}"));
    let found = match count {
        Some(n) => form(plural_category(locale, n))
            .or_else(|| form("other"))
            .or_else(|| messages.get(key)),
        None => messages.get(key).or_else(|| form("other")),
    };
    found.map(String::as_str)
}

/// `text` with each `{{name}}` replaced by its argument; placeholders
/// without one stay as written.
fn interpolate(text: &str, args: &[(&str, Arg)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(len) = rest[start + 2..].find("}}") else {
            break;
        };
        out.push_str(&rest[..start]);
        let name = rest[start + 2..start + 2 + len].trim();
        match args.iter().find(|(arg, _)| *arg == name) {
            Some((_, value)) => out.push_str(&value.to_string()),
            None => out.push_str(&rest[start..start + 4 + len]),
        }
        rest = &rest[start + 4 + len..];
    }
    out.push_str(rest);
    out
}

/// The names of the `{{name}}` placeholders in `text`, or the byte offset of
/// an unclosed `{{`.
pub(super) fn placeholders(text: &str) -> Result<Vec<&str>, usize> {
    let mut names = Vec::new();
    let mut offset = 0;
    while let Some(start) = text[offset..].find("{{") {
        let open = offset + start;
        let Some(len) = text[open + 2..].find("}}") else {
            return Err(open);
        };
        names.push(text[open + 2..open + 2 + len].trim());
        offset = open + 4 + len;
    }
    Ok(names)
}

/// A catalogue file as dotted key → text. Fails on invalid JSON, a root that
/// is not an object, or a value that is neither an object nor a string.
pub(super) fn flatten(source: &str) -> Result<Messages, String> {
    fn walk(prefix: &str, value: &Value, out: &mut Messages) -> Result<(), String> {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(&path, value, out)?;
                }
                Ok(())
            }
            Value::String(text) if !prefix.is_empty() => {
                out.insert(prefix.to_owned(), text.clone());
                Ok(())
            }
            _ if prefix.is_empty() => Err("the file is not a JSON object".to_owned()),
            _ => Err(format!("`{prefix}` is not a string or an object")),
        }
    }
    let value: Value = serde_json::from_str(source).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut out = Messages::new();
    walk("", &value, &mut out)?;
    Ok(out)
}

thread_local! {
    /// CLDR cardinal rules of every shipped locale, from compiled-in data.
    /// Per thread: ICU4X data handles are not `Sync` without its `sync`
    /// feature, and building them from baked data is cheap.
    static PLURAL_RULES: [Option<PluralRules>; 4] = Locale::ALL.map(|locale| {
        let icu = match locale {
            Locale::En => locale!("en"),
            Locale::PtPt => locale!("pt-PT"),
            Locale::EsEs => locale!("es-ES"),
            Locale::Fr => locale!("fr"),
        };
        PluralRules::try_new_cardinal((&icu).into())
            .inspect_err(|error| tracing::error!(%locale, %error, "no plural rules"))
            .ok()
    });
}

/// The CLDR cardinal category of `n` in `locale`: `one`, `many`, `other`, …
pub fn plural_category(locale: Locale, n: i64) -> &'static str {
    PLURAL_RULES.with(|rules| {
        rules[locale.index()]
            .as_ref()
            .map_or("other", |rules| category_name(rules.category_for(n)))
    })
}

/// Every cardinal category `locale` uses.
pub(super) fn plural_categories(locale: Locale) -> Vec<&'static str> {
    PLURAL_RULES.with(|rules| {
        rules[locale.index()].as_ref().map_or_else(
            || vec!["other"],
            |rules| rules.categories().map(category_name).collect(),
        )
    })
}

/// The names of all CLDR plural categories, as used in key suffixes.
pub(super) const CATEGORY_NAMES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

const fn category_name(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST: Catalogue = Catalogue::new(
        "test",
        [
            r#"{"greet": "Hello, {{name}}!", "files_one": "One file", "files_other": "{{count}} files",
                "only_en": "English only", "errors": {"too_long": "At most {{max}} characters."}}"#,
            r#"{"greet": "Olá, {{name}}!", "files_one": "Um ficheiro", "files_many": "{{count}} de ficheiros",
                "files_other": "{{count}} ficheiros", "errors": {"too_long": "No máximo {{max}} caracteres."}}"#,
            r#"{"greet": "¡Hola, {{ name }}!", "files_one": "Un archivo", "files_other": "{{count}} archivos"}"#,
            r#"{"greet": "Bonjour, {{name}} !", "files_one": "{{count}} fichier", "files_other": "{{count}} fichiers"}"#,
        ],
    );

    static BROKEN: Catalogue = Catalogue::new("broken", ["{", "[]", "{}", "{\"a\": 1}"]);

    #[test]
    fn renders_with_arguments() {
        let args = [("name", Arg::from("Ana"))];
        assert_eq!(TEST.t(Locale::En, "greet", &args), "Hello, Ana!");
        assert_eq!(TEST.t(Locale::PtPt, "greet", &args), "Olá, Ana!");
        assert_eq!(TEST.t(Locale::EsEs, "greet", &args), "¡Hola, Ana!");
        assert_eq!(TEST.t(Locale::Fr, "greet", &args), "Bonjour, Ana !");
        assert_eq!(TEST.t(Locale::En, "greet", &[]), "Hello, {{name}}!");
    }

    #[test]
    fn plural_forms_follow_cldr() {
        let files = |locale, n: i64| TEST.t(locale, "files", &[(COUNT, n.into())]);
        assert_eq!(files(Locale::En, 1), "One file");
        assert_eq!(files(Locale::En, 0), "0 files");
        // pt-PT is not pt: zero is `other` (pt-BR says `one`).
        assert_eq!(files(Locale::PtPt, 0), "0 ficheiros");
        assert_eq!(files(Locale::PtPt, 1), "Um ficheiro");
        assert_eq!(files(Locale::PtPt, 1_000_000), "1000000 de ficheiros");
        // French: 0 and 1 are `one`; millions are `many`, absent here → `other`.
        assert_eq!(files(Locale::Fr, 0), "0 fichier");
        assert_eq!(files(Locale::Fr, 2), "2 fichiers");
        assert_eq!(files(Locale::Fr, 1_000_000), "1000000 fichiers");
        assert_eq!(files(Locale::EsEs, 1), "Un archivo");
        // Without a count, the `other` form.
        assert_eq!(TEST.t(Locale::En, "files", &[]), "{{count}} files");
    }

    #[test]
    fn categories_per_locale() {
        assert_eq!(plural_categories(Locale::En), ["one", "other"]);
        for locale in [Locale::PtPt, Locale::EsEs, Locale::Fr] {
            let categories = plural_categories(locale);
            assert!(
                ["one", "many", "other"]
                    .iter()
                    .all(|c| categories.contains(c)),
                "{locale}: {categories:?}"
            );
            assert!(!categories.contains(&"few"), "{locale}");
        }
        assert_eq!(plural_category(Locale::PtPt, 0), "other");
        assert_eq!(plural_category(Locale::Fr, 0), "one");
        assert_eq!(plural_category(Locale::EsEs, 0), "other");
    }

    #[test]
    fn falls_back_to_english_then_the_key() {
        assert_eq!(TEST.t(Locale::Fr, "only_en", &[]), "English only");
        assert_eq!(TEST.t(Locale::Fr, "nowhere", &[]), "nowhere");
        assert_eq!(BROKEN.t(Locale::PtPt, "a", &[]), "a");
    }

    #[test]
    fn reasons_render_errors_messages() {
        let reason = TEST.reason("too_long").arg("max", 100_u32);
        assert_eq!(reason.code(), "too_long");
        assert_eq!(reason.to_string(), "too_long");
        assert_eq!(reason.render(Locale::PtPt), "No máximo 100 caracteres.");
        // Missing in Spanish: English.
        assert_eq!(reason.render(Locale::EsEs), "At most 100 characters.");
    }

    #[test]
    fn interpolation_edge_cases() {
        let args = [("a", Arg::from(1_i64)), ("b", Arg::from("x"))];
        assert_eq!(interpolate("{{a}}{{b}}", &args), "1x");
        assert_eq!(interpolate("{{a", &args), "{{a");
        assert_eq!(interpolate("{{c}} {{a}}", &args), "{{c}} 1");
        assert_eq!(interpolate("no placeholders", &args), "no placeholders");
        assert_eq!(placeholders("{{a}} and {{ b }}"), Ok(vec!["a", "b"]));
        assert_eq!(placeholders("x {{a"), Err(2));
    }

    #[test]
    fn flattening() {
        let flat = flatten(r#"{"a": {"b": "1", "c": {"d": "2"}}, "e": "3"}"#).unwrap();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat["a.c.d"], "2");
        assert!(flatten("[]").is_err());
        assert!(flatten(r#"{"a": 1}"#).unwrap_err().contains("`a`"));
        assert!(flatten("{").unwrap_err().contains("invalid JSON"));
    }
}
