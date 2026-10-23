//! The fake IMAP server's mailbox: folders of messages with UIDs, flags,
//! MODSEQs and UIDVALIDITY, changed by the tests as "another mail app"
//! would (deliver, mark read, move, copy, delete, reset), and the commands
//! the adapter reads them with (`LIST`, `STATUS`, `EXAMINE`, `UID SEARCH`,
//! `UID FETCH` with CONDSTORE and QRESYNC), answered as a server would.
//!
//! What the fake saw (folders opened, bodies sent, any change asked of it)
//! is recorded for the tests to check. It can also misbehave: say it is
//! busy, lie about a message's size, announce a literal larger than any
//! memory, or never stop answering.

use std::fmt::Write as _;

use chrono::{DateTime, NaiveDate, Utc};

/// A message in a fake folder.
#[derive(Debug, Clone)]
pub struct FakeMail {
    pub uid: u32,
    pub raw: Vec<u8>,
    pub seen: bool,
    pub flagged: bool,
    pub modseq: u64,
    pub internal_date: DateTime<Utc>,
    pub email_id: String,
    /// The size the server states, when it lies about it.
    pub stated_size: Option<u64>,
}

/// A fake folder.
#[derive(Debug, Clone)]
pub struct FakeFolder {
    pub name: String,
    /// Its SPECIAL-USE attributes (`\Sent`), shown when the server offers
    /// SPECIAL-USE.
    pub attributes: Vec<String>,
    pub uidvalidity: u32,
    pub uidnext: u32,
    pub mails: Vec<FakeMail>,
    /// UIDs expunged, with the MODSEQ of their expunge (QRESYNC).
    pub expunged: Vec<(u32, u64)>,
}

impl FakeFolder {
    fn highest_modseq(&self) -> u64 {
        self.mails
            .iter()
            .map(|m| m.modseq)
            .chain(self.expunged.iter().map(|(_, m)| *m))
            .max()
            .unwrap_or(1)
    }
}

/// The fake's whole mailbox.
#[derive(Debug, Clone)]
pub struct Store {
    pub folders: Vec<FakeFolder>,
    modseq: u64,
    ids: u64,
    uidvalidities: u32,
}

impl Default for Store {
    fn default() -> Self {
        let mut store = Self {
            folders: Vec::new(),
            modseq: 1,
            ids: 0,
            uidvalidities: 1000,
        };
        store.add_folder("INBOX", &[]);
        store
    }
}

impl Store {
    fn next_modseq(&mut self) -> u64 {
        self.modseq += 1;
        self.modseq
    }

    /// Add folder `name` with `attributes`.
    pub fn add_folder(&mut self, name: &str, attributes: &[&str]) {
        self.uidvalidities += 1;
        self.folders.push(FakeFolder {
            name: name.to_owned(),
            attributes: attributes.iter().map(|a| (*a).to_owned()).collect(),
            uidvalidity: self.uidvalidities,
            uidnext: 1,
            mails: Vec::new(),
            expunged: Vec::new(),
        });
    }

    /// Folder `name`.
    pub fn folder(&mut self, name: &str) -> &mut FakeFolder {
        self.folders
            .iter_mut()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no fake folder {name}"))
    }

    /// Put `raw` in folder `name`, received `at`; its UID.
    pub fn deliver(&mut self, name: &str, raw: &[u8], at: DateTime<Utc>) -> u32 {
        self.ids += 1;
        let email_id = format!("M{:08}", self.ids);
        self.append(name, raw.to_vec(), at, email_id, false, false)
    }

    fn append(
        &mut self,
        name: &str,
        raw: Vec<u8>,
        at: DateTime<Utc>,
        email_id: String,
        seen: bool,
        flagged: bool,
    ) -> u32 {
        let modseq = self.next_modseq();
        let folder = self.folder(name);
        let uid = folder.uidnext;
        folder.uidnext += 1;
        folder.mails.push(FakeMail {
            uid,
            raw,
            seen,
            flagged,
            modseq,
            internal_date: at,
            email_id,
            stated_size: None,
        });
        uid
    }

    /// Mark message `uid` of `name` read and flagged, or not.
    pub fn set_flags(&mut self, name: &str, uid: u32, seen: bool, flagged: bool) {
        let modseq = self.next_modseq();
        let mail = self.mail(name, uid);
        mail.seen = seen;
        mail.flagged = flagged;
        mail.modseq = modseq;
    }

    /// Message `uid` of `name`.
    pub fn mail(&mut self, name: &str, uid: u32) -> &mut FakeMail {
        self.folder(name)
            .mails
            .iter_mut()
            .find(|m| m.uid == uid)
            .unwrap_or_else(|| panic!("no fake message {uid}"))
    }

    /// Delete message `uid` of `name` for good.
    pub fn expunge(&mut self, name: &str, uid: u32) -> FakeMail {
        let modseq = self.next_modseq();
        let folder = self.folder(name);
        let at = folder
            .mails
            .iter()
            .position(|m| m.uid == uid)
            .unwrap_or_else(|| panic!("no fake message {uid}"));
        folder.expunged.push((uid, modseq));
        folder.mails.remove(at)
    }

    /// Copy message `uid` of `from` to `to` (same message, new UID).
    pub fn copy(&mut self, from: &str, uid: u32, to: &str) -> u32 {
        let mail = self.mail(from, uid).clone();
        self.append(
            to,
            mail.raw,
            mail.internal_date,
            mail.email_id,
            mail.seen,
            mail.flagged,
        )
    }

    /// Move message `uid` of `from` to `to`.
    pub fn move_mail(&mut self, from: &str, uid: u32, to: &str) -> u32 {
        let new = self.copy(from, uid, to);
        self.expunge(from, uid);
        new
    }

    /// Give folder `name` a new UIDVALIDITY and renumber its messages.
    pub fn reset_uidvalidity(&mut self, name: &str) {
        self.uidvalidities += 1;
        let uidvalidity = self.uidvalidities;
        let folder = self.folder(name);
        folder.uidvalidity = uidvalidity;
        folder.expunged.clear();
        let mut uid = 0;
        for mail in &mut folder.mails {
            uid += 1;
            mail.uid = uid;
        }
        folder.uidnext = uid + 1;
    }

    /// Remove folder `name` and its messages.
    pub fn remove_folder(&mut self, name: &str) {
        self.folders.retain(|f| f.name != name);
    }

    /// Rename folder `from` to `to` (its messages get new UIDs).
    pub fn rename_folder(&mut self, from: &str, to: &str) {
        self.uidvalidities += 1;
        let uidvalidity = self.uidvalidities;
        let folder = self.folder(from);
        to.clone_into(&mut folder.name);
        folder.uidvalidity = uidvalidity;
    }
}

/// What a command made the fake do, besides answering.
#[derive(Debug, Default, Clone)]
pub struct Effects {
    /// A folder opened (`EXAMINE` or `SELECT`), and whether read-write.
    pub opened: Option<(String, bool)>,
    /// Bodies sent, and how many of them marked their message read.
    pub bodies: usize,
    pub marked_read: usize,
    /// It was asked to change something (`STORE`, `EXPUNGE`, …).
    pub changes: usize,
}

/// How the fake behaves past signing in.
#[derive(Debug, Clone, Copy, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one independent behaviour a test turns on"
)]
pub struct Offers {
    pub special_use: bool,
    pub condstore: bool,
    pub qresync: bool,
    pub objectid: bool,
    /// Every `UID FETCH` is answered with endless `EXISTS` lines first.
    pub chatty: bool,
    /// Every body is announced as a literal larger than any memory.
    pub huge_literal: bool,
}

/// The answer to one signed-in command (`verb` and its `args`), on a
/// session with `selected` open.
pub fn answer(
    store: &mut Store,
    offers: Offers,
    tag: &str,
    verb: &str,
    args: &str,
    selected: &mut Option<String>,
    effects: &mut Effects,
) -> Vec<u8> {
    let args = tokens(args);
    let text = match verb {
        "ENABLE" if offers.qresync => format!("* ENABLED QRESYNC\r\n{tag} OK enabled\r\n"),
        "ENABLE" => format!("{tag} OK nothing enabled\r\n"),
        "NOOP" | "CHECK" => format!("{tag} OK done\r\n"),
        "LIST" => list(store, offers, tag, &args),
        "STATUS" => status(store, tag, &args),
        "EXAMINE" | "SELECT" => {
            let answer = open(store, offers, tag, &args, verb == "SELECT");
            if answer.contains(" OK ") {
                *selected = args.first().cloned();
                effects.opened =
                    Some((args.first().cloned().unwrap_or_default(), verb == "SELECT"));
            }
            answer
        }
        "UID" => {
            let Some(folder) = selected.clone() else {
                return format!("{tag} BAD nothing selected\r\n").into_bytes();
            };
            let rest = &args[1.min(args.len())..];
            match args.first().map(|v| v.to_ascii_uppercase()).as_deref() {
                Some("SEARCH") => search(store, tag, &folder, rest),
                Some("FETCH") => return fetch(store, offers, tag, &folder, rest, effects),
                _ => {
                    effects.changes += 1;
                    format!("{tag} NO not here\r\n")
                }
            }
        }
        _ => {
            effects.changes += 1;
            format!("{tag} BAD not here\r\n")
        }
    };
    text.into_bytes()
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

fn list(store: &Store, offers: Offers, tag: &str, args: &[String]) -> String {
    let pattern = args.get(1).map_or("", String::as_str);
    let mut out = String::new();
    for folder in &store.folders {
        if pattern != "*" && !folder.name.eq_ignore_ascii_case(pattern) {
            continue;
        }
        let mut attributes = vec!["\\HasNoChildren".to_owned()];
        if offers.special_use {
            attributes.extend(folder.attributes.iter().cloned());
        }
        let _ = write!(
            out,
            "* LIST ({}) \".\" {}\r\n",
            attributes.join(" "),
            quoted(&folder.name)
        );
    }
    let _ = write!(out, "{tag} OK LIST done\r\n");
    out
}

fn status(store: &mut Store, tag: &str, args: &[String]) -> String {
    let name = args.first().cloned().unwrap_or_default();
    let Some(folder) = store.folders.iter().find(|f| f.name == name) else {
        return format!("{tag} NO [NONEXISTENT] no such folder\r\n");
    };
    let asked = args
        .get(1)
        .map(|a| a.to_ascii_uppercase())
        .unwrap_or_default();
    let mut items = format!(
        "MESSAGES {} UIDNEXT {} UIDVALIDITY {}",
        folder.mails.len(),
        folder.uidnext,
        folder.uidvalidity
    );
    if asked.contains("HIGHESTMODSEQ") {
        let _ = write!(items, " HIGHESTMODSEQ {}", folder.highest_modseq());
    }
    format!(
        "* STATUS {} ({items})\r\n{tag} OK STATUS done\r\n",
        quoted(&folder.name)
    )
}

fn open(store: &Store, offers: Offers, tag: &str, args: &[String], write: bool) -> String {
    let name = args.first().cloned().unwrap_or_default();
    let Some(folder) = store.folders.iter().find(|f| f.name == name) else {
        return format!("{tag} NO [NONEXISTENT] no such folder\r\n");
    };
    let mut out = format!(
        "* {} EXISTS\r\n* OK [UIDVALIDITY {}] UIDs valid\r\n* OK [UIDNEXT {}] next\r\n",
        folder.mails.len(),
        folder.uidvalidity,
        folder.uidnext
    );
    if offers.condstore {
        let _ = write!(
            out,
            "* OK [HIGHESTMODSEQ {}] modseq\r\n",
            folder.highest_modseq()
        );
    }
    let mode = if write { "READ-WRITE" } else { "READ-ONLY" };
    let _ = write!(out, "{tag} OK [{mode}] opened\r\n");
    out
}

/// Whether `uid` is in sequence set `set` (`*` is `max`).
fn in_set(set: &str, uid: u32, max: u32) -> bool {
    let number = |n: &str| {
        if n == "*" {
            Some(max)
        } else {
            n.parse::<u32>().ok()
        }
    };
    set.split(',').any(|part| {
        let (a, b) = part.split_once(':').unwrap_or((part, part));
        match (number(a), number(b)) {
            (Some(a), Some(b)) => (a.min(b)..=a.max(b)).contains(&uid),
            _ => false,
        }
    })
}

/// The value of header `name` in `raw` (unfolded), if any.
fn header(raw: &[u8], name: &str) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let mut found: Option<String> = None;
    for line in text.split("\r\n") {
        if line.is_empty() {
            break;
        }
        if let Some(value) = found.as_mut() {
            if line.starts_with([' ', '\t']) {
                value.push_str(line);
                continue;
            }
            break;
        }
        if let Some((field, value)) = line.split_once(':')
            && field.eq_ignore_ascii_case(name)
        {
            found = Some(value.to_owned());
        }
    }
    found
}

fn matches(tokens: &[String], at: &mut usize, mail: &FakeMail, max: u32) -> bool {
    let Some(key) = tokens.get(*at).map(|k| k.to_ascii_uppercase()) else {
        return false;
    };
    *at += 1;
    match key.as_str() {
        "UID" => {
            let set = tokens.get(*at).cloned().unwrap_or_default();
            *at += 1;
            in_set(&set, mail.uid, max)
        }
        "SINCE" => {
            let date = tokens.get(*at).cloned().unwrap_or_default();
            *at += 1;
            NaiveDate::parse_from_str(&date, "%d-%b-%Y")
                .is_ok_and(|since| mail.internal_date.date_naive() >= since)
        }
        "HEADER" => {
            let name = tokens.get(*at).cloned().unwrap_or_default();
            let value = tokens.get(*at + 1).cloned().unwrap_or_default();
            *at += 2;
            header(&mail.raw, &name).is_some_and(|v| v.contains(&value))
        }
        "OR" => {
            let a = matches(tokens, at, mail, max);
            let b = matches(tokens, at, mail, max);
            a || b
        }
        "ALL" => true,
        _ => false,
    }
}

fn search(store: &mut Store, tag: &str, folder: &str, criteria: &[String]) -> String {
    let folder = store.folder(folder);
    let max = folder.mails.iter().map(|m| m.uid).max().unwrap_or(0);
    let found: Vec<String> = folder
        .mails
        .iter()
        .filter(|mail| {
            let mut at = 0;
            let mut all = true;
            while at < criteria.len() {
                all &= matches(criteria, &mut at, mail, max);
            }
            all
        })
        .map(|mail| mail.uid.to_string())
        .collect();
    let mut out = "* SEARCH".to_owned();
    for uid in found {
        out.push(' ');
        out.push_str(&uid);
    }
    format!("{out}\r\n{tag} OK SEARCH done\r\n")
}

/// The header lines of `raw` named in `names`, then a blank line.
fn header_fields(raw: &[u8], names: &[String]) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    let mut out = String::new();
    let mut keep = false;
    for line in text.split("\r\n") {
        if line.is_empty() {
            break;
        }
        if !line.starts_with([' ', '\t']) {
            keep = line
                .split_once(':')
                .is_some_and(|(name, _)| names.iter().any(|n| n.eq_ignore_ascii_case(name)));
        }
        if keep {
            out.push_str(line);
            out.push_str("\r\n");
        }
    }
    out.push_str("\r\n");
    out.into_bytes()
}

fn literal(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(format!("{{{}}}\r\n", bytes.len()).as_bytes());
    out.extend_from_slice(bytes);
}

#[allow(
    clippy::too_many_lines,
    reason = "one FETCH answer, item by item, as a server builds it"
)]
fn fetch(
    store: &mut Store,
    offers: Offers,
    tag: &str,
    name: &str,
    args: &[String],
    effects: &mut Effects,
) -> Vec<u8> {
    let set = args.first().cloned().unwrap_or_default();
    let items = args
        .get(1)
        .map(|i| tokens(i.trim_start_matches('(').trim_end_matches(')')))
        .unwrap_or_default();
    let modifiers = args
        .get(2)
        .map(|m| tokens(m.trim_start_matches('(').trim_end_matches(')')))
        .unwrap_or_default();
    let changed_since = modifiers
        .iter()
        .position(|m| m.eq_ignore_ascii_case("CHANGEDSINCE"))
        .and_then(|at| modifiers.get(at + 1))
        .and_then(|n| n.parse::<u64>().ok());
    let vanished = modifiers.iter().any(|m| m.eq_ignore_ascii_case("VANISHED"));
    let mut out = Vec::new();
    if offers.chatty {
        for _ in 0..50_000 {
            out.extend_from_slice(b"* 1 EXISTS\r\n");
        }
    }
    let folder = store.folder(name);
    let max = folder.mails.iter().map(|m| m.uid).max().unwrap_or(0);
    if let (Some(since), true, true) = (changed_since, vanished, offers.qresync) {
        let gone: Vec<String> = folder
            .expunged
            .iter()
            .filter(|(uid, modseq)| *modseq > since && in_set(&set, *uid, u32::MAX))
            .map(|(uid, _)| uid.to_string())
            .collect();
        if !gone.is_empty() {
            out.extend_from_slice(
                format!("* VANISHED (EARLIER) {}\r\n", gone.join(",")).as_bytes(),
            );
        }
    }
    for (seq, mail) in folder.mails.iter_mut().enumerate() {
        if !in_set(&set, mail.uid, max) || changed_since.is_some_and(|since| mail.modseq <= since) {
            continue;
        }
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let mut uid_sent = false;
        for item in &items {
            let upper = item.to_ascii_uppercase();
            match upper.as_str() {
                "UID" => {
                    uid_sent = true;
                    parts.push(format!("UID {}", mail.uid).into_bytes());
                }
                "FLAGS" => {
                    let mut flags = Vec::new();
                    if mail.seen {
                        flags.push("\\Seen");
                    }
                    if mail.flagged {
                        flags.push("\\Flagged");
                    }
                    parts.push(format!("FLAGS ({})", flags.join(" ")).into_bytes());
                }
                "RFC822.SIZE" => parts.push(
                    format!(
                        "RFC822.SIZE {}",
                        mail.stated_size
                            .unwrap_or(u64::try_from(mail.raw.len()).unwrap_or(u64::MAX))
                    )
                    .into_bytes(),
                ),
                "INTERNALDATE" => parts.push(
                    format!(
                        "INTERNALDATE \"{}\"",
                        mail.internal_date.format("%d-%b-%Y %H:%M:%S +0000")
                    )
                    .into_bytes(),
                ),
                "EMAILID" if offers.objectid => {
                    parts.push(format!("EMAILID ({})", mail.email_id).into_bytes());
                }
                _ if upper.starts_with("BODY.PEEK[HEADER.FIELDS")
                    || upper.starts_with("BODY[HEADER.FIELDS") =>
                {
                    let names = upper
                        .split_once('(')
                        .map(|(_, rest)| tokens(rest.trim_end_matches([')', ']'])))
                        .unwrap_or_default();
                    let mut part =
                        format!("BODY[HEADER.FIELDS ({})] ", names.join(" ")).into_bytes();
                    literal(&mut part, &header_fields(&mail.raw, &names));
                    parts.push(part);
                }
                "BODY.PEEK[]" | "BODY[]" | "RFC822" => {
                    effects.bodies += 1;
                    if upper != "BODY.PEEK[]" {
                        effects.marked_read += 1;
                        mail.seen = true;
                    }
                    if offers.huge_literal {
                        out.extend_from_slice(
                            format!(
                                "* {} FETCH (UID {} BODY[] {{4000000000}}\r\nFrom: x",
                                seq + 1,
                                mail.uid
                            )
                            .as_bytes(),
                        );
                        return out;
                    }
                    let mut part = b"BODY[] ".to_vec();
                    literal(&mut part, &mail.raw);
                    parts.push(part);
                }
                _ => {}
            }
        }
        if !uid_sent {
            parts.insert(0, format!("UID {}", mail.uid).into_bytes());
        }
        if changed_since.is_some() {
            parts.push(format!("MODSEQ ({})", mail.modseq).into_bytes());
        }
        out.extend_from_slice(format!("* {} FETCH (", seq + 1).as_bytes());
        for (n, part) in parts.iter().enumerate() {
            if n > 0 {
                out.push(b' ');
            }
            out.extend_from_slice(part);
        }
        out.extend_from_slice(b")\r\n");
    }
    out.extend_from_slice(format!("{tag} OK FETCH done\r\n").as_bytes());
    out
}

/// The arguments of a command: quoted strings (unquoted), parenthesised
/// groups (whole), and atoms (a `[…]` section whole).
pub fn tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ' ' => i += 1,
            '"' => {
                let mut token = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    if let Some(c) = chars.get(i) {
                        token.push(*c);
                    }
                    i += 1;
                }
                i += 1;
                out.push(token);
            }
            '(' => {
                let start = i;
                let mut depth = 0;
                while i < chars.len() {
                    match chars[i] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                out.push(chars[start..i].iter().collect());
            }
            _ => {
                let start = i;
                let mut bracket = 0;
                while i < chars.len() {
                    match chars[i] {
                        '[' => bracket += 1,
                        ']' => bracket -= 1,
                        ' ' if bracket == 0 => break,
                        _ => {}
                    }
                    i += 1;
                }
                out.push(chars[start..i].iter().collect());
            }
        }
    }
    out
}
