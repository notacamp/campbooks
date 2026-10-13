//! What an app may call itself on the consent screen and in settings.
//!
//! A name a stranger chose (dynamic registration, a metadata document) or a
//! workspace admin typed is cleaned of invisible formatting and direction
//! characters, which could make it read as something else, and may not pass
//! itself off as Campbooks: "Campbooks" and "Not A Camp" belong to the
//! first-party apps, whatever the case, spacing or look-alike letters.

/// The longest name kept, in characters.
pub const MAX_NAME: usize = 100;

/// `name` without control, format and direction characters (bidi
/// embeddings and isolates, zero-width characters, the BOM, tags…), with
/// runs of whitespace collapsed, trimmed and cut to [`MAX_NAME`]
/// characters. Empty when nothing visible is left.
pub fn clean(name: &str) -> String {
    let visible: String = name
        .chars()
        .filter(|c| !is_invisible(*c))
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    visible
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_NAME)
        .collect()
}

fn is_invisible(c: char) -> bool {
    c.is_control()
        || matches!(
            u32::from(c),
            0x00AD
                | 0x034F
                | 0x061C
                | 0x115F
                | 0x1160
                | 0x17B4
                | 0x17B5
                | 0x180B..=0x180F
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x206F
                | 0x3164
                | 0xFE00..=0xFE0F
                | 0xFEFF
                | 0xFFA0
                | 0xFFF0..=0xFFFB
                | 0x1D173..=0x1D17A
                | 0xE0000..=0xE0FFF
        )
}

/// Whether `name` would read as Campbooks' own: once decomposed for
/// compatibility (NFKD: full-width, mathematical and circled letters become
/// plain ones), stripped of accents and other combining marks, lower-cased,
/// stripped of everything but letters and digits, and with common
/// look-alikes folded (Cyrillic and Greek letters, `0` for `o`, `1` for
/// `l`…), it contains `campbooks` or `notacamp`.
pub fn impersonates_campbooks(name: &str) -> bool {
    let decomposed = icu_normalizer::DecomposingNormalizerBorrowed::new_nfkd().normalize(name);
    let skeleton: String = clean(&decomposed)
        .chars()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter_map(fold)
        .collect();
    skeleton.contains("campbooks") || skeleton.contains("notacamp")
}

/// A combining mark (accents, overlays…) left by decomposition.
fn is_combining_mark(c: char) -> bool {
    matches!(
        u32::from(c),
        0x0300..=0x036F | 0x0483..=0x0489 | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

/// One character of a name as the ASCII letter it looks like; `None` for
/// what is dropped (spaces, punctuation, symbols).
fn fold(c: char) -> Option<char> {
    let folded = match c {
        'а' | 'α' | '@' | '4' => 'a',
        'в' | 'β' | 'ь' | '8' => 'b',
        'с' | 'ϲ' | '(' => 'c',
        'е' | 'ε' | '3' => 'e',
        'һ' => 'h',
        'і' | 'ι' | '!' | '|' => 'i',
        'к' | 'κ' => 'k',
        '1' | 'ӏ' => 'l',
        'м' | 'μ' => 'm',
        'п' | 'η' => 'n',
        'о' | 'ο' | 'σ' | '0' => 'o',
        'р' | 'ρ' => 'p',
        'ѕ' | '5' | '$' => 's',
        'т' | 'τ' | '7' | '+' => 't',
        'х' | 'χ' => 'x',
        'у' | 'γ' => 'y',
        c if c.is_ascii_alphanumeric() => c,
        _ => return None,
    };
    Some(folded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_and_direction_characters_go() {
        assert_eq!(
            clean("  Desk\u{202E}Agent\u{200B} \n Pro "),
            "DeskAgent Pro"
        );
        assert_eq!(clean("\u{2066}\u{2069}\u{FEFF}"), "");
        assert_eq!(clean(&"x".repeat(300)).chars().count(), MAX_NAME);
    }

    #[test]
    fn nobody_else_is_campbooks() {
        for fake in [
            "Campbooks",
            "campbooks desktop",
            "CAMP BOOKS",
            "Cаmpbооks",
            "C4mpb00ks",
            "Camp-books Sync",
            "Not A Camp",
            "not.a.camp mail",
            "Camp\u{200B}books",
            "Cámpbooks",
            "C\u{0301}ampbooks",
            "Ｃａｍｐｂｏｏｋｓ",
            "𝐂𝐚𝐦𝐩𝐛𝐨𝐨𝐤𝐬",
            "𝕮𝖆𝖒𝖕𝖇𝖔𝖔𝖐𝖘",
            "Ⓒⓐⓜⓟⓑⓞⓞⓚⓢ",
            "Nöt Á Cämp",
        ] {
            assert!(impersonates_campbooks(fake), "{fake}");
        }
        for fine in ["Desk Agent", "Camping planner", "Books & Co"] {
            assert!(!impersonates_campbooks(fine), "{fine}");
        }
    }
}
