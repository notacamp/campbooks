//! Highlighted text as segments, never markup.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::text::{HIT_END, HIT_START};

/// A run of plain text, part of a hit or not. Clients render it as text
/// (a `<mark>` around hits); it never carries markup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Segment {
    /// The text, verbatim.
    pub text: String,
    /// Whether it matched the query.
    pub hit: bool,
}

/// `highlighted` (text whose hits Postgres delimited with [`HIT_START`] and
/// [`HIT_END`]) as segments. Stray delimiters are dropped; adjacent runs of
/// the same kind are merged; empty runs are left out.
pub fn split(highlighted: &str) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut current = String::new();
    let mut hit = false;
    let push = |segments: &mut Vec<Segment>, text: &mut String, hit: bool| {
        if text.is_empty() {
            return;
        }
        match segments.last_mut() {
            Some(last) if last.hit == hit => last.text.push_str(text),
            _ => segments.push(Segment {
                text: std::mem::take(text),
                hit,
            }),
        }
        text.clear();
    };
    for c in highlighted.chars() {
        match c {
            HIT_START if !hit => {
                push(&mut segments, &mut current, hit);
                hit = true;
            }
            HIT_END if hit => {
                push(&mut segments, &mut current, hit);
                hit = false;
            }
            HIT_START | HIT_END => {}
            c => current.push(c),
        }
    }
    push(&mut segments, &mut current, hit);
    segments
}

/// `text` as one segment without hits (or none when empty).
pub fn plain(text: &str) -> Vec<Segment> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![Segment {
            text: text.to_owned(),
            hit: false,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimited_hits_become_segments() {
        let segments = split("Your \u{e000}invoice\u{e001} for \u{e000}October\u{e001}");
        assert_eq!(
            segments,
            vec![
                Segment {
                    text: "Your ".into(),
                    hit: false
                },
                Segment {
                    text: "invoice".into(),
                    hit: true
                },
                Segment {
                    text: " for ".into(),
                    hit: false
                },
                Segment {
                    text: "October".into(),
                    hit: true
                },
            ]
        );
    }

    #[test]
    fn markup_stays_text_and_stray_delimiters_go() {
        let segments = split("<b>urgent</b> \u{e001}x\u{e000}\u{e000}y\u{e001}");
        assert_eq!(segments[0].text, "<b>urgent</b> x");
        assert!(!segments[0].hit);
        assert_eq!(
            segments[1],
            Segment {
                text: "y".into(),
                hit: true
            }
        );
    }
}
