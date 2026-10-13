//! The locales Campbooks ships, and choosing one from a language tag or an
//! `Accept-Language` header.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A locale Campbooks ships, always as its full BCP 47 tag: a bare `pt`
/// means Brazilian Portuguese to CLDR and `Intl`, and we write European
/// Portuguese and Spanish from Spain.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    ToSchema,
)]
pub enum Locale {
    /// English (the source language and the last fallback).
    #[default]
    #[serde(rename = "en")]
    En,
    /// European Portuguese.
    #[serde(rename = "pt-PT")]
    PtPt,
    /// Spanish from Spain.
    #[serde(rename = "es-ES")]
    EsEs,
    /// French.
    #[serde(rename = "fr")]
    Fr,
}

impl Locale {
    /// Every shipped locale, English first.
    pub const ALL: [Self; 4] = [Self::En, Self::PtPt, Self::EsEs, Self::Fr];

    /// The BCP 47 tag, as stored, sent in `Content-Language` and used for
    /// catalogue file names (`<tag>.json`).
    pub const fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtPt => "pt-PT",
            Self::EsEs => "es-ES",
            Self::Fr => "fr",
        }
    }

    /// Position in [`ALL`](Self::ALL).
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::En => 0,
            Self::PtPt => 1,
            Self::EsEs => 2,
            Self::Fr => 3,
        }
    }

    /// The shipped locale closest to any language tag, by its primary
    /// language subtag: `pt-BR` → `pt-PT`, `es-MX` → `es-ES`, `fr-CA` →
    /// `fr`, `en-GB` → `en` (the closest translation beats English, as on the
    /// client). `None` for languages we do not ship.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag.trim().split(['-', '_']).next().unwrap_or_default();
        Self::ALL
            .into_iter()
            .find(|locale| locale.tag()[..2].eq_ignore_ascii_case(primary))
    }

    /// The first locale an `Accept-Language` value (RFC 9110 §12.5.4) asks
    /// for that we ship, by descending quality (ties keep their order).
    /// Entries with `q=0`, malformed weights and `*` are skipped.
    pub fn from_accept_language(header: &str) -> Option<Self> {
        let mut ranges: Vec<(&str, u16)> = header
            .split(',')
            .filter_map(|entry| {
                let mut parts = entry.split(';');
                let range = parts.next()?.trim();
                let mut quality = 1000;
                for param in parts {
                    let (name, value) = param.split_once('=')?;
                    if name.trim().eq_ignore_ascii_case("q") {
                        quality = parse_quality(value.trim())?;
                    }
                }
                (quality > 0 && !range.is_empty() && range != "*").then_some((range, quality))
            })
            .collect();
        ranges.sort_by_key(|&(_, quality)| std::cmp::Reverse(quality));
        ranges
            .into_iter()
            .find_map(|(range, _)| Self::from_tag(range))
    }
}

/// A `qvalue` (`0`, `1`, `0.5`, at most three decimals) in thousandths.
fn parse_quality(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let thousandths = format!("{fraction:0<3}").parse::<u16>().ok()?;
    match whole {
        "0" => Some(thousandths),
        "1" if thousandths == 0 => Some(1000),
        _ => None,
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// Exactly one of the shipped tags (as stored); see [`Locale::from_tag`] for
/// the lenient mapping of any language tag.
impl std::str::FromStr for Locale {
    type Err = UnknownLocale;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|locale| locale.tag() == s)
            .ok_or_else(|| UnknownLocale(s.to_owned()))
    }
}

/// A string that is not one of the shipped locale tags.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown locale `{0}` (expected en, pt-PT, es-ES or fr)")]
pub struct UnknownLocale(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip() {
        for locale in Locale::ALL {
            assert_eq!(locale.tag().parse::<Locale>().unwrap(), locale);
            assert_eq!(
                serde_json::to_value(locale).unwrap(),
                serde_json::json!(locale.tag())
            );
            assert_eq!(Locale::ALL[locale.index()], locale);
        }
        assert!("pt".parse::<Locale>().is_err());
        assert!("es".parse::<Locale>().is_err());
    }

    #[test]
    fn any_tag_maps_by_language() {
        for (tag, expected) in [
            ("pt-BR", Some(Locale::PtPt)),
            ("pt", Some(Locale::PtPt)),
            ("PT-pt", Some(Locale::PtPt)),
            ("es-MX", Some(Locale::EsEs)),
            ("es-419", Some(Locale::EsEs)),
            ("fr-CA", Some(Locale::Fr)),
            ("en-GB", Some(Locale::En)),
            ("en_US", Some(Locale::En)),
            ("de-DE", None),
            ("", None),
            ("e", None),
        ] {
            assert_eq!(Locale::from_tag(tag), expected, "{tag}");
        }
    }

    #[test]
    fn accept_language_by_quality() {
        for (header, expected) in [
            ("pt-BR, en;q=0.5", Some(Locale::PtPt)),
            ("de-DE, fr;q=0.3", Some(Locale::Fr)),
            ("en;q=0.2, es-ES;q=0.9", Some(Locale::EsEs)),
            ("fr;q=0, es", Some(Locale::EsEs)),
            ("*, fr;q=0.1", Some(Locale::Fr)),
            ("fr;q=0.5, pt;q=0.5", Some(Locale::Fr)),
            ("de, it;q=0.8", None),
            ("fr;q=2, en", Some(Locale::En)),
            ("fr;q=abc, en", Some(Locale::En)),
            ("", None),
        ] {
            assert_eq!(Locale::from_accept_language(header), expected, "{header}");
        }
    }

    #[test]
    fn quality_values() {
        assert_eq!(parse_quality("1"), Some(1000));
        assert_eq!(parse_quality("1.000"), Some(1000));
        assert_eq!(parse_quality("0.5"), Some(500));
        assert_eq!(parse_quality("0.125"), Some(125));
        assert_eq!(parse_quality("0"), Some(0));
        assert_eq!(parse_quality("1.5"), None);
        assert_eq!(parse_quality("0.1234"), None);
        assert_eq!(parse_quality("-0.5"), None);
    }
}
