//! What an app may call itself on the consent screen and in settings.
//!
//! A name a stranger chose (dynamic registration, a metadata document) or a
//! workspace admin typed is cleaned of invisible formatting and direction
//! characters, which could make it read as something else, and may not pass
//! itself off as Campbooks: "Campbooks" and "Not A Camp" belong to the
//! first-party apps, whatever the case, spacing or look-alike letters (the
//! Unicode confusables table, UTS #39). Nor may one word mix scripts, the way
//! a Cyrillic `а` hides in a Latin word.

use icu_properties::{
    CodePointMapData,
    props::{GeneralCategory, GeneralCategoryGroup},
};
use unicode_security::{RestrictionLevel, RestrictionLevelDetection as _};

/// The longest name kept, in characters.
pub const MAX_NAME: usize = 100;

/// `name` without control, format and direction characters (bidi
/// embeddings and isolates, zero-width characters, the BOM, tags…), with
/// runs of whitespace collapsed, trimmed and cut to [`MAX_NAME`]
/// characters. Empty when nothing visible is left.
pub fn clean(name: &str) -> String {
    visible(name).chars().take(MAX_NAME).collect()
}

/// [`clean`] without the cut: what the checks read after normalisation,
/// which can lengthen a name many times (`ﷺ` is 18 characters), so nothing
/// pushed past [`MAX_NAME`] escapes them. Names are at most [`MAX_NAME`]
/// characters before, so this stays small.
fn visible(name: &str) -> String {
    let visible: String = name
        .chars()
        .filter(|c| !is_invisible(*c))
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    visible.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// Whether `name` would read as Campbooks' own. The name is decomposed for
/// compatibility (NFKD: full-width, mathematical and circled letters become
/// plain ones), stripped of every mark (general category M: accents, vowel
/// signs, overlays, in any script) and lower-cased, then reduced to its
/// UTS #39 confusables skeleton (the Unicode Consortium's table of
/// look-alikes: Cyrillic, Greek, Armenian, Cherokee, Coptic, IPA, digits of
/// other scripts…; `rn` and `m` share one), with leetspeak digits read as
/// letters. It reads as Campbooks when that skeleton contains the skeleton of
/// `campbooks` or `notacamp`, ignoring spaces and punctuation. A letter or
/// digit nothing maps stands for any character, and so does a symbol (or the
/// symbol is ignored, whichever matches), at most [`MAX_UNKNOWN`] of them in
/// the word: `Campb○○ks` reads as Campbooks, while a name in another script is
/// not a match by wildcards alone.
pub fn impersonates_campbooks(name: &str) -> bool {
    let read = skeleton(name);
    let without_symbols: Vec<Folded> = read
        .iter()
        .copied()
        .filter(|f| *f != Folded::Symbol)
        .collect();
    RESERVED.iter().any(|word| {
        let word: Vec<char> = skeleton(word)
            .into_iter()
            .filter_map(|f| match f {
                Folded::Ascii(c) => Some(c),
                Folded::Unknown | Folded::Symbol => None,
            })
            .collect();
        contains(&read, &word) || contains(&without_symbols, &word)
    })
}

/// Whether a word of `name` mixes scripts in a way that lets it pass for
/// another word: worse than UTS #39's "highly restrictive" (Latin with
/// Armenian, Cherokee, Coptic, Cyrillic, Greek…, or a letter no identifier
/// may use). A name in one script, Latin or not, or Latin with Han, kana or
/// Hangul, is fine.
pub fn mixes_scripts(name: &str) -> bool {
    let categories = CodePointMapData::<GeneralCategory>::new();
    let composed = icu_normalizer::ComposingNormalizerBorrowed::new_nfkc().normalize(name);
    let cleaned = visible(&composed);
    cleaned
        .split(|c: char| {
            let category = categories.get(c);
            !(GeneralCategoryGroup::Letter.contains(category)
                || GeneralCategoryGroup::Mark.contains(category)
                || GeneralCategoryGroup::Number.contains(category))
        })
        .filter(|word| !word.is_empty())
        .any(|word| word.detect_restriction_level() > RestrictionLevel::HighlyRestrictive)
}

/// The names only Campbooks' own apps may take (`campbook` covers the
/// plural too).
const RESERVED: [&str; 2] = ["campbook", "notacamp"];

/// The most characters nothing maps that a match may lean on.
const MAX_UNKNOWN: usize = 2;

/// One character of a name's skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Folded {
    /// The ASCII letter or digit it reads as.
    Ascii(char),
    /// A letter or digit outside ASCII nothing maps: it may read as anything.
    Unknown,
    /// A symbol outside ASCII nothing maps (`○`, `☺`): it may read as
    /// anything, or be ignored.
    Symbol,
}

/// `name` as the characters it reads as (see [`impersonates_campbooks`]);
/// spaces and punctuation are left out.
fn skeleton(name: &str) -> Vec<Folded> {
    let categories = CodePointMapData::<GeneralCategory>::new();
    let not_a_mark = |c: &char| !GeneralCategoryGroup::Mark.contains(categories.get(*c));
    let decomposed = icu_normalizer::DecomposingNormalizerBorrowed::new_nfkd().normalize(name);
    // `()` drawn for `o`, and ligatures read as their first letter (`æ`).
    let plain: String = visible(&decomposed)
        .replace("()", "o")
        .chars()
        .filter(not_a_mark)
        .map(|c| match c {
            'æ' | 'Æ' => 'a',
            'œ' | 'Œ' => 'o',
            c => c,
        })
        .collect();
    // Look-alikes first (the table maps capitals of other scripts to Latin
    // capitals), then lower case, then once more for the lower-case ones
    // (`m` and `rn` meet there).
    let lower: String = unicode_security::skeleton(&plain)
        .flat_map(char::to_lowercase)
        .collect();
    unicode_security::skeleton(&lower)
        .flat_map(char::to_lowercase)
        .filter(not_a_mark)
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() || c == '@' || c == '$' {
                return Some(Folded::Ascii(leet(c)));
            }
            if c.is_ascii() {
                return None;
            }
            let category = categories.get(c);
            if GeneralCategoryGroup::Letter.contains(category)
                || GeneralCategoryGroup::Number.contains(category)
            {
                Some(Folded::Unknown)
            } else if GeneralCategoryGroup::Symbol.contains(category) {
                Some(Folded::Symbol)
            } else {
                None
            }
        })
        .collect()
}

/// A digit or sign written for the letter it resembles (`4` and `@` for
/// `a`, `$` for `s`); any other character as itself.
fn leet(c: char) -> char {
    match c {
        '4' | '@' => 'a',
        '8' => 'b',
        '3' => 'e',
        '5' | '$' => 's',
        '7' => 't',
        c => c,
    }
}

/// Whether `skeleton` holds `word` somewhere, each unknown character standing
/// for any one of its letters, at most [`MAX_UNKNOWN`] of them.
fn contains(skeleton: &[Folded], word: &[char]) -> bool {
    skeleton.windows(word.len()).any(|window| {
        window
            .iter()
            .filter(|f| !matches!(f, Folded::Ascii(_)))
            .count()
            <= MAX_UNKNOWN
            && window
                .iter()
                .zip(word)
                .all(|(folded, expected)| match folded {
                    Folded::Ascii(c) => c == expected,
                    Folded::Unknown | Folded::Symbol => true,
                })
    })
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
            // Homoglyphs of other scripts (UTS #39 confusables).
            "Cɑmpbօօks",
            "ᏟᎪᎷᏢᏴOOᏦᏚ",
            "Ꮯampbօօks",
            "Ꮯɑmpbօօkѕ",
            "ⲥampbօօks",
            "Nоt Ꭺ Ꮯɑmp",
            // Zeros of other scripts, and circles.
            "Campb००ks",
            "Campb๐๐ks",
            "Campb߀߀ks",
            "Campb○○ks",
            "Campb◯◯ks",
            // Lengthened past 100 characters by normalisation.
            &format!("{} Campbooks", "\u{FDFA}".repeat(6)),
            "Ứng dụng quản lý thư điện tử chính thức được chứng nhận bởi đội ngũ Campbooks",
            &format!("{} Campbooks", "é".repeat(60)),
            &format!("{} C\u{0430}mpb\u{043E}\u{043E}ks", "…".repeat(40)),
            &format!("{} Not A Camp", "⑴".repeat(34)),
            // ASCII leet, the singular, a ligature.
            "C@mpbooks",
            "Campbook$",
            "C@mpb00k$",
            "Campb()()ks",
            "Campbook",
            "Cæmpbooks",
        ] {
            assert!(impersonates_campbooks(fake), "{fake}");
        }
        for fine in [
            "Desk Agent",
            "Camping planner",
            "Camping Notebook",
            "Bookcamp",
            "Books & Co",
            "Café Ørsted",
            "José",
            "Françoise",
            "メールアシスタントプロ",
            "我的邮件管理助手应用",
            "تطبيق البريد",
            "Learning Corner",
            "東京 Notes",
            "Ελληνικό ταχυδρομείο",
            "Почтовый помощник",
            "메일 도우미",
            "ფოსტის ასისტენტი",
            "Փոստի օգնական",
        ] {
            assert!(!impersonates_campbooks(fine), "{fine}");
        }
    }

    #[test]
    fn a_word_keeps_to_one_script() {
        for mixed in [
            "Cɑmpbօօks",
            "Ꮯampbօօks",
            "ⲥampbօօks",
            "Nоt Ꭺ Ꮯɑmp",
            "Pаypal Sync",
            &format!("{} C\u{0430}mpb\u{043E}\u{043E}ks", "…".repeat(40)),
            "Gооgle Drive",
            // Small capitals: Latin, but no identifier may use them.
            "ᴄᴀᴍᴘʙᴏᴏᴋꜱ",
        ] {
            assert!(mixes_scripts(mixed), "{mixed}");
        }
        for single in [
            "Desk Agent",
            "José's Books",
            "Françoise",
            "メールアシスタントプロ",
            "Gmail助手",
            "我的邮件管理助手应用",
            "تطبيق البريد",
            "Ελληνικό ταχυδρομείο",
            "Почтовый помощник",
            "메일 도우미",
            "ფოსტის ასისტენტი",
            "Փոստի օգնական",
            "Sync 2",
        ] {
            assert!(!mixes_scripts(single), "{single}");
        }
    }
}
