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

use serde::{Deserialize, Serialize};

/// What a folder is to the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The Inbox.
    Inbox,
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
/// separated by `/`.
pub fn display(name: &str, delimiter: Option<char>) -> String {
    let decoded = decode(name).unwrap_or_else(|| name.to_owned());
    let shown = without_inbox(&decoded, delimiter);
    match delimiter {
        Some(d) if d != '/' => shown.replace(d, "/"),
        _ => shown.to_owned(),
    }
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
    for folder in listed {
        if !readable(&folder.name) || folders.iter().any(|f| f.name == folder.name) {
            continue;
        }
        let Some(role) = role(folder) else {
            continue;
        };
        if role == Role::Inbox && folders.iter().any(|f| f.role == Role::Inbox) {
            continue;
        }
        folders.push(Tracked {
            name: folder.name.clone(),
            display: display(&folder.name, folder.delimiter),
            role,
        });
    }
    if !folders.iter().any(|f| f.role == Role::Inbox) {
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
}
