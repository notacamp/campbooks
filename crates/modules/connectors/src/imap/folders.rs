//! A mailbox's folders as places: which are the Inbox, Sent, Junk, Trash
//! and Archive, which are the member's own (read, shown as labels), and
//! which are never read (Drafts, the `\All` and `\Flagged` views of other
//! folders, and folders that cannot be opened).
//!
//! A folder's role comes from its SPECIAL-USE attribute (RFC 6154) when the
//! server gives one; otherwise from its name, matched exactly (ignoring
//! case, after decoding modified UTF-7, with or without an `INBOX.` prefix)
//! against the well-known names of the languages Campbooks ships and of
//! common providers. Only folder names given by the server are matched,
//! never anything in a message (ADR 0018).
//!
//! A folder named like a stream mark (`Campbooks/Updates`, under its
//! parents and whatever the server's delimiter) is a **stream folder**:
//! part of the inbox, where that stream's mail is filed (campbooks-cloud#312).
//! Campbooks creates one when a move needs it, its name built by
//! [`created_name`] as if a member had chosen it.

use std::collections::HashSet;

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::{contract::ProviderStream, domain::marks};

/// What a folder is to the adapter. The order is the order folders are
/// read and kept in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The Inbox.
    Inbox,
    /// A stream's folder (`Campbooks/<Stream>`): part of the inbox.
    Stream,
    /// Sent mail.
    Sent,
    /// Spam.
    Junk,
    /// The trash.
    Trash,
    /// Archived mail.
    Archive,
    /// One of the member's own folders: a label.
    User,
}

impl Role {
    /// The role's token in the index (`state`).
    pub const fn token(self) -> &'static str {
        match self {
            Self::Inbox => "INBOX",
            Self::Stream => "STREAM",
            Self::Sent => "SENT",
            Self::Junk => "JUNK",
            Self::Trash => "TRASH",
            Self::Archive => "ARCHIVE",
            Self::User => "USER",
        }
    }
}

/// Sent folders' well-known names.
const SENT: &[&str] = &[
    "sent",
    "sent items",
    "sent messages",
    "sent mail",
    "enviados",
    "itens enviados",
    "envoyés",
    "éléments envoyés",
    "gesendet",
    "gesendete elemente",
];

/// Spam folders' well-known names.
const JUNK: &[&str] = &[
    "junk",
    "spam",
    "bulk",
    "bulk mail",
    "lixo eletrónico",
    "correo no deseado",
    "courrier indésirable",
    "indésirables",
];

/// Trash folders' well-known names.
const TRASH: &[&str] = &[
    "trash",
    "deleted items",
    "deleted messages",
    "bin",
    "lixo",
    "reciclagem",
    "papelera",
    "corbeille",
    "papierkorb",
];

/// Archive folders' well-known names.
const ARCHIVE: &[&str] = &["archive", "archives", "arquivo", "archivo", "archiv"];

/// Drafts folders' well-known names: never read.
const DRAFTS: &[&str] = &[
    "drafts",
    "rascunhos",
    "borradores",
    "brouillons",
    "entwürfe",
];

/// Longest folder name read, in bytes.
pub const MAX_NAME: usize = 1000;

/// A folder as `LIST` gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its attributes, lower-cased (`\noselect`, `\sent`).
    pub attributes: Vec<String>,
    /// Its hierarchy delimiter.
    pub delimiter: Option<char>,
    /// Its name as the server spells it (modified UTF-7).
    pub name: String,
}

/// A folder the adapter reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracked {
    /// Its name as the server spells it.
    pub name: String,
    /// Its name as shown: decoded, under its parents (`Projects/Bakery`).
    pub display: String,
    /// What it is.
    pub role: Role,
}

/// The role of `folder`, or `None` for a folder never read.
pub fn role(folder: &Listed) -> Option<Role> {
    let has = |attribute: &str| folder.attributes.iter().any(|a| a == attribute);
    if has("\\noselect") || has("\\nonexistent") {
        return None;
    }
    if folder.name.eq_ignore_ascii_case("INBOX") {
        return Some(Role::Inbox);
    }
    if has("\\drafts") || has("\\all") || has("\\flagged") {
        return None;
    }
    for (attribute, role) in [
        ("\\sent", Role::Sent),
        ("\\junk", Role::Junk),
        ("\\trash", Role::Trash),
        ("\\archive", Role::Archive),
    ] {
        if has(attribute) {
            return Some(role);
        }
    }
    let decoded = decode(&folder.name)?;
    if stream_of(&display(&folder.name, folder.delimiter)).is_some() {
        return Some(Role::Stream);
    }
    let leaf = without_inbox(&decoded, folder.delimiter).to_lowercase();
    let named = |names: &[&str]| names.contains(&leaf.as_str());
    if named(DRAFTS) {
        return None;
    }
    Some(if named(SENT) {
        Role::Sent
    } else if named(JUNK) {
        Role::Junk
    } else if named(TRASH) {
        Role::Trash
    } else if named(ARCHIVE) {
        Role::Archive
    } else {
        Role::User
    })
}

/// `name` without an `INBOX` parent (`INBOX.Sent` → `Sent`).
fn without_inbox(name: &str, delimiter: Option<char>) -> &str {
    let separators = delimiter.map_or_else(|| vec!['.', '/'], |d| vec![d]);
    name.get(..5)
        .filter(|head| head.eq_ignore_ascii_case("INBOX"))
        .and_then(|_| {
            let rest = &name[5..];
            separators
                .iter()
                .find_map(|separator| rest.strip_prefix(*separator))
        })
        .filter(|rest| !rest.is_empty())
        .unwrap_or(name)
}

/// How `name` is shown: decoded, without an `INBOX` parent, its parents
/// separated by `/`, and without control or text-direction characters
/// (a name is the server's word, shown as a label).
pub fn display(name: &str, delimiter: Option<char>) -> String {
    let decoded = decode(name).unwrap_or_else(|| name.to_owned());
    let shown = without_inbox(&decoded, delimiter);
    let shown = match delimiter {
        Some(d) if d != '/' => shown.replace(d, "/"),
        _ => shown.to_owned(),
    };
    shown.chars().filter(|c| !hidden(*c)).collect()
}

/// A character that is not shown, or that changes how the text around it
/// reads: controls, and the bidirectional marks, embeddings, overrides and
/// isolates.
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

/// The stream a folder shown as `display` files (`Campbooks/Updates`, any
/// case), if it is a stream folder.
pub fn stream_of(display: &str) -> Option<ProviderStream> {
    marks::of_name(display)
}

/// Longest part of a folder name Campbooks creates, in characters.
pub const MAX_PART: usize = 64;

/// Longest folder name Campbooks creates, encoded, in bytes.
pub const MAX_CREATED_NAME: usize = 255;

/// The name of a folder Campbooks creates, as the server spells it: `parts`
/// (each checked by [`part`], then encoded in modified UTF-7) joined by the
/// server's hierarchy `delimiter` (a flat server, without one, takes them as
/// one name joined by `/`), under `parent` (a folder name the server gave,
/// such as `INBOX`) when given. `None` when a part could escape its place
/// (the delimiter, `/`, a `LIST` wildcard, `.` or `..`), cannot be shown
/// (controls, direction marks), is empty or too long, or the whole name is
/// longer than [`MAX_CREATED_NAME`].
pub fn created_name(
    parts: &[&str],
    delimiter: Option<char>,
    parent: Option<&str>,
) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    let joint = delimiter.unwrap_or('/');
    let mut name = String::new();
    if let Some(parent) = parent {
        if parent.is_empty() || !parent.bytes().all(|b| (0x21..0x7f).contains(&b)) {
            return None;
        }
        name.push_str(parent);
        name.push(joint);
    }
    for (n, text) in parts.iter().enumerate() {
        if n > 0 {
            name.push(joint);
        }
        name.push_str(&part(text, delimiter)?);
    }
    (name.len() <= MAX_CREATED_NAME).then_some(name)
}

/// One part of a folder name Campbooks creates, encoded; `None` when it is
/// refused (see [`created_name`]).
pub fn part(text: &str, delimiter: Option<char>) -> Option<String> {
    let refused =
        |c: char| Some(c) == delimiter || matches!(c, '/' | '\\' | '*' | '%' | '"') || hidden(c);
    let length = text.chars().count();
    if length == 0
        || length > MAX_PART
        || text.trim() != text
        || text == "."
        || text == ".."
        || text.chars().any(refused)
    {
        return None;
    }
    Some(encode(text))
}

/// `name` in modified UTF-7 (RFC 3501 §5.1.3): printable ASCII as itself
/// (`&` as `&-`), anything else as `&`, the modified base64 of its UTF-16,
/// and `-`.
pub fn encode(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut wide: Vec<u16> = Vec::new();
    for c in name.chars() {
        if (' '..='~').contains(&c) {
            flush(&mut out, &mut wide);
            if c == '&' {
                out.push_str("&-");
            } else {
                out.push(c);
            }
        } else {
            let mut units = [0_u16; 2];
            wide.extend_from_slice(c.encode_utf16(&mut units));
        }
    }
    flush(&mut out, &mut wide);
    out
}

/// Write the run of `wide` characters (UTF-16) as modified base64.
fn flush(out: &mut String, wide: &mut Vec<u16>) {
    if wide.is_empty() {
        return;
    }
    let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_be_bytes()).collect();
    out.push('&');
    out.push_str(&STANDARD_NO_PAD.encode(bytes).replace('/', ","));
    out.push('-');
    wide.clear();
}

/// Whether `name` can be read: printable ASCII (modified UTF-7 is), short
/// enough, and quotable.
fn readable(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME && name.bytes().all(|b| (0x20..0x7f).contains(&b))
}

/// The folders to read of those `LIST` gave: at most `max`, the Inbox
/// first, then the other places, then the member's own by name. `None`
/// when the listing has no Inbox (an answer not believed).
pub fn tracked(listed: &[Listed], max: usize) -> Option<Vec<Tracked>> {
    let mut folders: Vec<Tracked> = Vec::new();
    let mut names: HashSet<&str> = HashSet::new();
    let mut inbox = false;
    for folder in listed {
        if !readable(&folder.name) || !names.insert(folder.name.as_str()) {
            continue;
        }
        let Some(role) = role(folder) else {
            continue;
        };
        if role == Role::Inbox && std::mem::replace(&mut inbox, true) {
            continue;
        }
        folders.push(Tracked {
            name: folder.name.clone(),
            display: display(&folder.name, folder.delimiter),
            role,
        });
    }
    if !inbox {
        return None;
    }
    folders.sort_by(|a, b| (a.role, &a.name).cmp(&(b.role, &b.name)));
    folders.truncate(max.max(1));
    Some(folders)
}

/// Decode a modified UTF-7 folder name (RFC 3501 §5.1.3); `None` when it
/// is not valid.
pub fn decode(name: &str) -> Option<String> {
    let mut out = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let end = after.find('-')?;
        let encoded = &after[..end];
        if encoded.is_empty() {
            out.push('&');
        } else {
            out.push_str(&utf16(encoded)?);
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// The text of one modified base64 run (`,` for `/`, no padding) of
/// big-endian UTF-16.
fn utf16(encoded: &str) -> Option<String> {
    let mut bits: u32 = 0;
    let mut count = 0;
    let mut bytes = Vec::with_capacity(encoded.len());
    for c in encoded.bytes() {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b',' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push(u8::try_from((bits >> count) & 0xff).ok()?);
        }
        bits &= (1 << count) - 1;
    }
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(name: &str, attributes: &[&str]) -> Listed {
        Listed {
            attributes: attributes.iter().map(|a| (*a).to_owned()).collect(),
            delimiter: Some('.'),
            name: name.to_owned(),
        }
    }

    #[test]
    fn modified_utf7_is_decoded() {
        assert_eq!(decode("Entw&APw-rfe").as_deref(), Some("Entwürfe"));
        assert_eq!(
            decode("&AMk-l&AOk-ments envoy&AOk-s").as_deref(),
            Some("Éléments envoyés")
        );
        assert_eq!(decode("Tom &- Jerry").as_deref(), Some("Tom & Jerry"));
        assert_eq!(decode("&ZeVnLIqe-").as_deref(), Some("日本語"));
        assert_eq!(decode("Broken &AMk"), None);
        assert_eq!(decode("Bad &*-"), None);
    }

    #[test]
    fn special_use_wins_then_names_then_the_members_own() {
        assert_eq!(role(&listed("INBOX", &[])), Some(Role::Inbox));
        assert_eq!(role(&listed("Things", &["\\sent"])), Some(Role::Sent));
        assert_eq!(role(&listed("INBOX.Sent Items", &[])), Some(Role::Sent));
        assert_eq!(role(&listed("INBOX.Bulk Mail", &[])), Some(Role::Junk));
        assert_eq!(role(&listed("Lixo Eletr&APM-nico", &[])), Some(Role::Junk));
        assert_eq!(role(&listed("Corbeille", &[])), Some(Role::Trash));
        assert_eq!(role(&listed("Archiv", &[])), Some(Role::Archive));
        assert_eq!(role(&listed("Bakery", &[])), Some(Role::User));
        // Never read.
        for never in [
            listed("Drafts", &[]),
            listed("Brouillons", &[]),
            listed("Mine", &["\\drafts"]),
            listed("All Mail", &["\\all"]),
            listed("Starred", &["\\flagged"]),
            listed("Parent", &["\\noselect"]),
        ] {
            assert_eq!(role(&never), None, "{never:?}");
        }
    }

    #[test]
    fn folders_are_ordered_capped_and_shown_under_their_parents() {
        let folders = tracked(
            &[
                listed("INBOX.Projects.Bakery", &[]),
                listed("INBOX.Sent", &[]),
                listed("INBOX", &[]),
                listed("INBOX", &[]),
                listed("Drafts", &[]),
                listed("Line\r\nBreak", &[]),
            ],
            10,
        )
        .unwrap();
        let shown: Vec<(&str, Role)> = folders
            .iter()
            .map(|f| (f.display.as_str(), f.role))
            .collect();
        assert_eq!(
            shown,
            [
                ("INBOX", Role::Inbox),
                ("Sent", Role::Sent),
                ("Projects/Bakery", Role::User)
            ]
        );
        let capped = tracked(
            &[listed("INBOX", &[]), listed("A", &[]), listed("B", &[])],
            2,
        );
        assert_eq!(capped.unwrap().len(), 2);
        // No Inbox: not believed.
        assert_eq!(tracked(&[listed("Sent", &[])], 10), None);
    }

    #[test]
    fn stream_folders_are_found_by_name_under_their_parents() {
        assert_eq!(role(&listed("Campbooks.Updates", &[])), Some(Role::Stream));
        assert_eq!(
            role(&listed("INBOX.campbooks.NEWSLETTERS", &[])),
            Some(Role::Stream)
        );
        let slash = Listed {
            delimiter: Some('/'),
            ..listed("Campbooks/Receipts", &[])
        };
        assert_eq!(role(&slash), Some(Role::Stream));
        assert_eq!(role(&listed("Campbooks", &[])), Some(Role::User));
        assert_eq!(role(&listed("Campbooks.Recibos", &[])), Some(Role::User));
        assert_eq!(
            stream_of(&display("INBOX.Campbooks.Updates", Some('.'))),
            Some(ProviderStream::Updates)
        );
        // Kept right after the Inbox, so a cap on folders never drops one.
        let kept = tracked(
            &[
                listed("A", &[]),
                listed("Campbooks.Updates", &[]),
                listed("INBOX", &[]),
            ],
            2,
        )
        .unwrap();
        assert_eq!(kept[1].role, Role::Stream);
    }

    #[test]
    fn modified_utf7_is_encoded_as_it_is_decoded() {
        for name in [
            "Entwürfe",
            "Éléments envoyés",
            "Tom & Jerry",
            "日本語",
            "Café ☕ 🍞",
            "Plain",
        ] {
            let encoded = encode(name);
            assert!(
                encoded.bytes().all(|b| (0x20..0x7f).contains(&b)),
                "{encoded}"
            );
            assert_eq!(decode(&encoded).as_deref(), Some(name), "{encoded}");
        }
        assert_eq!(encode("Entwürfe"), "Entw&APw-rfe");
        assert_eq!(encode("Tom & Jerry"), "Tom &- Jerry");
        assert_eq!(encode("日本語"), "&ZeVnLIqe-");
    }

    #[test]
    fn created_names_cannot_escape_or_nest() {
        assert_eq!(
            created_name(&["Campbooks", "Updates"], Some('.'), None).as_deref(),
            Some("Campbooks.Updates")
        );
        assert_eq!(
            created_name(&["Campbooks", "Updates"], Some('.'), Some("INBOX")).as_deref(),
            Some("INBOX.Campbooks.Updates")
        );
        assert_eq!(
            created_name(&["Campbooks", "Updates"], None, None).as_deref(),
            Some("Campbooks/Updates")
        );
        // Non-ASCII letters: one part, encoded.
        assert_eq!(
            created_name(&["Campbooks", "Reçus"], Some('/'), None).as_deref(),
            Some("Campbooks/Re&AOc-us")
        );
        let long = "x".repeat(MAX_PART + 1);
        for hostile in [
            "Up.dates",
            "Up/dates",
            "Up*",
            "%",
            "..",
            ".",
            "",
            " Updates",
            "Line\r\nBreak",
            "Bell\u{7}",
            "Right\u{202E}to-left",
            "Quote\"d",
            "Back\\slash",
            long.as_str(),
        ] {
            assert_eq!(
                created_name(&["Campbooks", hostile], Some('.'), None),
                None,
                "{hostile:?}"
            );
        }
        // Another delimiter is refused too, whatever the server's.
        assert_eq!(created_name(&["Campbooks", "A|B"], Some('|'), None), None);
        assert_eq!(
            created_name(&["Campbooks", "A.B"], Some('/'), None).as_deref(),
            Some("Campbooks/A.B")
        );
        // A whole name past the bound.
        let parts: Vec<String> = (0..5).map(|_| "y".repeat(MAX_PART)).collect();
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        assert_eq!(created_name(&parts, Some('.'), None), None);
        assert_eq!(created_name(&[], Some('.'), None), None);
    }

    #[test]
    fn names_are_shown_without_controls_or_direction_overrides() {
        assert_eq!(display("Bakery\u{202E}gpj.exe", None), "Bakerygpj.exe");
        assert_eq!(display("Ba\u{7}k\u{2066}ery\u{200F}", None), "Bakery");
        assert_eq!(display("Caf&AOk-", None), "Caf\u{e9}");
    }
}
