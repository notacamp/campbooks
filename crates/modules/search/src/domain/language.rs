//! Which of the shipped languages an entry is written in.

use std::sync::LazyLock;

use whatlang::{Detector, Lang};

/// A text search configuration of the `search` schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// English.
    En,
    /// Portuguese.
    Pt,
    /// Spanish.
    Es,
    /// French.
    Fr,
    /// None of them, or too little text to tell.
    Simple,
}

impl Language {
    /// Every configuration a query term is looked up in.
    pub const ALL: [Self; 5] = [Self::En, Self::Pt, Self::Es, Self::Fr, Self::Simple];

    /// The `language` column and the configuration's name in schema `search`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Pt => "pt",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::Simple => "simple",
        }
    }
}

/// Below this, a detection is not trusted and the neutral configuration is
/// used; the query side looks a term up in every language anyway.
const MIN_CONFIDENCE: f64 = 0.4;

static DETECTOR: LazyLock<Detector> =
    LazyLock::new(|| Detector::with_allowlist(vec![Lang::Eng, Lang::Por, Lang::Spa, Lang::Fra]));

/// The language of `text`.
pub fn detect(text: &str) -> Language {
    let Some(info) = DETECTOR.detect(text) else {
        return Language::Simple;
    };
    if !info.is_reliable() && info.confidence() < MIN_CONFIDENCE {
        return Language::Simple;
    }
    match info.lang() {
        Lang::Eng => Language::En,
        Lang::Por => Language::Pt,
        Lang::Spa => Language::Es,
        Lang::Fra => Language::Fr,
        _ => Language::Simple,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_languages() {
        let cases = [
            (
                "Hi Sam, I've attached the Q4 plan. Could you review the budget section and send me your comments by Friday?",
                Language::En,
            ),
            (
                "Olá Inês, a fatura n.º FT 2026/09812, no valor de 23,47 €, já está disponível na sua área de cliente.",
                Language::Pt,
            ),
            (
                "Buenos días, adjuntamos la factura por importe de 1.284,60 € correspondiente al pedido de septiembre.",
                Language::Es,
            ),
            (
                "Bonsoir Camille, il y a une fuite d'eau dans la cave près de votre box, pouvez-vous passer ce soir ?",
                Language::Fr,
            ),
        ];
        for (text, language) in cases {
            assert_eq!(detect(text), language, "{text}");
        }
    }

    #[test]
    fn too_little_text_is_neutral() {
        assert_eq!(detect(""), Language::Simple);
        assert_eq!(detect("1234 5678"), Language::Simple);
    }
}
