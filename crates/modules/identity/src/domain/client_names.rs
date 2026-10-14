//! What an app may call itself on the consent screen and in settings.
//!
//! A name a stranger chose (dynamic registration, a metadata document) or a
//! workspace admin typed is cleaned of invisible formatting and direction
//! characters, which could make it read as something else, and may not pass
//! itself off as Campbooks: "Campbooks" and "Not A Camp" belong to the
//! first-party apps, whatever the case, spacing or look-alike letters.

use icu_properties::{
    CodePointMapData,
    props::{GeneralCategory, GeneralCategoryGroup},
};

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
/// plain ones), stripped of every mark (general category M: accents, vowel
/// signs, overlays, in any script), lower-cased, ligatures split, stripped of
/// everything but letters and digits, and with look-alikes folded (Cyrillic
/// and Greek letters, small capitals, letters with a stroke such as `ø` and
/// `ł`, `0` for `o`, `1` for `l`, `rn` for `m`…), it contains `campbooks` or
/// `notacamp`. A letter outside ASCII nothing folds may stand for any letter,
/// at most [`MAX_UNKNOWN`] of them in the word: `Campb··ks` still reads as
/// Campbooks, while a name in another script is not a match by wildcards
/// alone.
pub fn impersonates_campbooks(name: &str) -> bool {
    let categories = CodePointMapData::<GeneralCategory>::new();
    let decomposed = icu_normalizer::DecomposingNormalizerBorrowed::new_nfkd().normalize(name);
    let folded: Vec<Folded> = clean(&decomposed)
        .chars()
        .filter(|c| !GeneralCategoryGroup::Mark.contains(categories.get(*c)))
        .flat_map(char::to_lowercase)
        .flat_map(ligature)
        .filter_map(fold)
        .collect();
    let skeleton = rn_as_m(&folded);
    ["campbooks", "notacamp"].iter().any(|word| {
        skeleton.windows(word.len()).any(|window| {
            let unknown = window.iter().filter(|f| **f == Folded::Other).count();
            unknown <= MAX_UNKNOWN
                && window
                    .iter()
                    .zip(word.chars())
                    .all(|(folded, expected)| match folded {
                        Folded::Ascii(c) => *c == expected,
                        Folded::Other => true,
                    })
        })
    })
}

/// The most letters nothing folds that a match may lean on.
const MAX_UNKNOWN: usize = 2;

/// `skeleton` with every `r` directly followed by `n` read as one `m`.
fn rn_as_m(skeleton: &[Folded]) -> Vec<Folded> {
    let mut out = Vec::with_capacity(skeleton.len());
    let mut rest = skeleton.iter().peekable();
    while let Some(folded) = rest.next() {
        if *folded == Folded::Ascii('r') && rest.peek() == Some(&&Folded::Ascii('n')) {
            rest.next();
            out.push(Folded::Ascii('m'));
        } else {
            out.push(*folded);
        }
    }
    out
}

/// A ligature as the letters it joins (`ꝏ` is `oo`); any other character
/// as itself.
fn ligature(c: char) -> impl Iterator<Item = char> {
    let pair = match c {
        'ꝏ' => Some(['o', 'o']),
        'æ' => Some(['a', 'e']),
        'œ' => Some(['o', 'e']),
        'ß' => Some(['s', 's']),
        'ꜳ' => Some(['a', 'a']),
        _ => None,
    };
    let (first, second) = pair.map_or((c, None), |[a, b]| (a, Some(b)));
    std::iter::once(first).chain(second)
}

/// One character of a name's skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Folded {
    /// The ASCII letter or digit it reads as.
    Ascii(char),
    /// A letter outside ASCII nothing folds: it may read as any letter.
    Other,
}

/// One character of a name as the ASCII letter it looks like, or as an
/// unknown letter; `None` for what is dropped (spaces, punctuation,
/// symbols).
fn fold(c: char) -> Option<Folded> {
    let folded = match c {
        'а' | 'α' | '@' | '4' | 'ᴀ' => 'a',
        'в' | 'β' | 'ь' | '8' | 'ʙ' | 'ᴃ' => 'b',
        'с' | 'ϲ' | '(' | 'ᴄ' | 'ȼ' => 'c',
        'đ' | 'ᴅ' => 'd',
        'е' | 'ε' | '3' | 'ᴇ' | 'ɇ' => 'e',
        'ɢ' => 'g',
        'һ' | 'ħ' | 'ʜ' => 'h',
        'і' | 'ι' | '!' | '|' | 'ı' | 'ɪ' | 'ɨ' => 'i',
        'ȷ' | 'ᴊ' => 'j',
        'к' | 'κ' | 'ᴋ' => 'k',
        '1' | 'ӏ' | 'ł' | 'ʟ' | 'ƚ' | 'ŀ' => 'l',
        'м' | 'μ' | 'ᴍ' => 'm',
        'п' | 'η' | 'ɴ' => 'n',
        'о' | 'ο' | 'σ' | '0' | 'ø' | 'ᴏ' | 'ɵ' | 'ᴓ' => 'o',
        'р' | 'ρ' | 'ᴘ' | 'ᵽ' => 'p',
        'ʀ' => 'r',
        'ѕ' | '5' | '$' | 'ꜱ' => 's',
        'т' | 'τ' | '7' | '+' | 'ŧ' | 'ᴛ' => 't',
        'ᴜ' | 'ʉ' => 'u',
        'ᴠ' => 'v',
        'ᴡ' => 'w',
        'х' | 'χ' => 'x',
        'у' | 'γ' | 'ʏ' => 'y',
        'ᴢ' | 'ƶ' => 'z',
        c if c.is_ascii_alphanumeric() => c,
        c if c.is_alphabetic() => return Some(Folded::Other),
        _ => return None,
    };
    Some(Folded::Ascii(folded))
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
            "Campbøøks",
            "ᴄᴀᴍᴘʙᴏᴏᴋꜱ",
            "Campbʘʘks",
            "Campbꝏks",
            "Νοt Α Cαmp",
            "Camp ☺ books",
            "Campb\u{05B4}ooks",
            "Campb\u{064E}ooks",
            "Camp\u{0E31}books",
            "Carnpbooks",
            "Not A Carnp",
            "Campb\u{0D20}\u{0D20}ks",
        ] {
            assert!(impersonates_campbooks(fake), "{fake}");
        }
        for fine in [
            "Desk Agent",
            "Camping planner",
            "Books & Co",
            "Café Ørsted",
            "José",
            "Françoise",
            "メールアシスタントプロ",
            "我的邮件管理助手应用",
            "تطبيق البريد",
            "Learning Corner",
            "東京 Notes",
        ] {
            assert!(!impersonates_campbooks(fine), "{fine}");
        }
    }
}
