//! The fake IMAP server's mailbox: folders of messages with UIDs, flags,
//! MODSEQs and UIDVALIDITY, changed by the tests as "another mail app"
//! would (deliver, mark read, move, copy, delete, reset), and the commands
//! the adapter reads them with (`LIST`, `STATUS`, `EXAMINE`, `UID SEARCH`,
//! `UID FETCH` with CONDSTORE and QRESYNC), answered as a server would.
//!
//! What the fake saw (folders opened, bodies sent, any change asked of it)
//! is recorded for the tests to check. In IDLE, it reports the selected
//! folder's changes as a server does (`EXISTS`, `EXPUNGE`, `FETCH`). It can also misbehave: say it is
//! busy, lie about a message's size, announce a literal larger than any
//! memory, or never stop answering.
//!
//! It takes writes as a server does (campbooks-cloud#312): `SELECT`, `UID
//! STORE` (CONDSTORE's `UNCHANGEDSINCE` and `[MODIFIED]`), `UID MOVE`
//! (MOVE), `UID COPY`, `UID EXPUNGE` and `COPYUID` (UIDPLUS), `EXPUNGE`
//! and `CLOSE` (expunging every `\Deleted` message of the folder, as a
//! server would), `CREATE`, `SUBSCRIBE`, `UNSELECT`; and it can be busy
//! for writes only, or accept them without making them.

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
    /// Marked `\Deleted`, not yet expunged.
    pub deleted: bool,
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
    /// Folders `EXAMINE` and `SELECT` refuse to open.
    pub refuse_open: Vec<String>,
    /// Messages (folder, UID) whose body is never sent: the command gets
    /// no answer at all.
    pub stalled: Vec<(String, u32)>,
    /// Folders created (`CREATE`) and subscribed to (`SUBSCRIBE`).
    pub created: Vec<String>,
    pub subscribed: Vec<String>,
    modseq: u64,
    ids: u64,
    uidvalidities: u32,
}

impl Default for Store {
    fn default() -> Self {
        let mut store = Self {
            folders: Vec::new(),
            refuse_open: Vec::new(),
            stalled: Vec::new(),
            created: Vec::new(),
            subscribed: Vec::new(),
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

    /// Append `raw` to folder `name` (`APPEND`), `\Seen` when `seen`: its
    /// folder's UIDVALIDITY and its UID; `None` when there is no such
    /// folder.
    pub fn append_mail(
        &mut self,
        name: &str,
        raw: Vec<u8>,
        at: DateTime<Utc>,
        seen: bool,
    ) -> Option<(u32, u32)> {
        let uidvalidity = self.folders.iter().find(|f| f.name == name)?.uidvalidity;
        self.ids += 1;
        let email_id = format!("M{:08}", self.ids);
        Some((
            uidvalidity,
            self.append(name, raw, at, email_id, seen, false),
        ))
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
            deleted: false,
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

    /// Mark message `uid` of `name` `\Deleted`, as another mail app would
    /// before it expunges.
    pub fn set_deleted(&mut self, name: &str, uid: u32) {
        let modseq = self.next_modseq();
        let mail = self.mail(name, uid);
        mail.deleted = true;
        mail.modseq = modseq;
    }

    /// The UIDs of folder `name`, in order.
    pub fn uids(&self, name: &str) -> Vec<u32> {
        self.folders
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.mails.iter().map(|m| m.uid).collect())
            .unwrap_or_default()
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

    /// What IDLE reports of folder `name` when it changes: its message
    /// count and its highest MODSEQ (none when it is gone).
    pub fn idle_state(&self, name: &str) -> Option<(usize, u64)> {
        self.folders
            .iter()
            .find(|f| f.name == name)
            .map(|f| (f.mails.len(), f.highest_modseq()))
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

impl Store {
    /// Expunge the messages of `name` marked `\Deleted` whose UIDs `only`
    /// lets through; the `EXPUNGE` lines a server sends for them.
    fn expunge_deleted(&mut self, name: &str, only: impl Fn(u32) -> bool) -> String {
        let doomed: Vec<u32> = self
            .folder(name)
            .mails
            .iter()
            .filter(|m| m.deleted && only(m.uid))
            .map(|m| m.uid)
            .collect();
        let mut out = String::new();
        for uid in doomed {
            let seq = self
                .folder(name)
                .mails
                .iter()
                .position(|m| m.uid == uid)
                .unwrap_or_default()
                + 1;
            self.expunge(name, uid);
            let _ = write!(out, "* {seq} EXPUNGE\r\n");
        }
        out
    }
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
    /// Every `CHANGEDSINCE … VANISHED` is answered with more vanished
    /// ranges than anything holds (QRESYNC).
    pub vanish_flood: bool,
    /// It offers IDLE (RFC 2177), and reports its selected folder's changes
    /// while a session is in it.
    pub idle: bool,
    /// In IDLE, it repeats `EXISTS` without end.
    pub idle_flood: bool,
    /// It offers MOVE (RFC 6851).
    pub moves: bool,
    /// It offers UIDPLUS (RFC 4315): `UID EXPUNGE` and `COPYUID`.
    pub uidplus: bool,
    /// It offers UNSELECT (RFC 3691).
    pub unselect: bool,
    /// It answers every write `NO [UNAVAILABLE]`.
    pub busy_writes: bool,
    /// It answers every write `OK` and makes none of them.
    pub inert_writes: bool,
    /// UIDPLUS, but `COPY` answers without `COPYUID`.
    pub copyuid_missing: bool,
    /// `COPYUID` names the first copy only.
    pub copyuid_partial: bool,
    /// `COPYUID` names UID 1 of the destination for every copy.
    pub copyuid_lies: bool,
    /// Every expunge it reports comes with one `EXPUNGE` more.
    pub extra_expunges: bool,
    /// `UID EXPUNGE` is refused.
    pub refuse_uid_expunge: bool,
    /// `UID FETCH … (UID FLAGS)` answers with no message.
    pub hide_flags: bool,
    /// It announces `APPENDLIMIT=<n>` (RFC 7889), and refuses a larger
    /// `APPEND` before its bytes.
    pub append_limit: Option<u64>,
    /// `APPEND` is answered `NO [UNAVAILABLE]` before its bytes.
    pub append_busy: bool,
    /// `APPEND` is answered `NO [OVERQUOTA]` after its bytes.
    pub append_refused: bool,
    /// `APPEND` takes the bytes and never answers.
    pub append_hang: bool,
    /// `APPEND` makes it hang up at once.
    pub append_drop: bool,
    /// `UID SEARCH` answers with every UID of the folder, whatever it asks.
    pub lying_search: bool,
}

/// The answer to one signed-in command (`verb` and its `args`), on a
/// session with `selected` open.
#[allow(
    clippy::too_many_lines,
    reason = "one fake server's commands, verb by verb, as a server answers them"
)]
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
            let verb = args.first().map(|v| v.to_ascii_uppercase());
            match verb.as_deref() {
                Some("SEARCH") if offers.lying_search => {
                    search(store, tag, &folder, &["ALL".to_owned()])
                }
                Some("SEARCH") => search(store, tag, &folder, rest),
                Some("FETCH") => return fetch(store, offers, tag, &folder, rest, effects),
                Some(write) => {
                    effects.changes += 1;
                    if offers.busy_writes {
                        format!("{tag} NO [UNAVAILABLE] try again later\r\n")
                    } else if offers.inert_writes {
                        format!("{tag} OK done (or so it says)\r\n")
                    } else {
                        match write {
                            "STORE" => store_flags(store, offers, tag, &folder, rest),
                            "MOVE" if offers.moves => copy(store, offers, tag, &folder, rest, true),
                            "COPY" => copy(store, offers, tag, &folder, rest, false),
                            "EXPUNGE" if offers.refuse_uid_expunge => {
                                format!("{tag} NO cannot expunge now\r\n")
                            }
                            "EXPUNGE" if offers.uidplus => {
                                let set = rest.first().cloned().unwrap_or_default();
                                let mut lines = store
                                    .expunge_deleted(&folder, |uid| in_set(&set, uid, u32::MAX));
                                if offers.extra_expunges {
                                    lines.push_str("* 1 EXPUNGE\r\n");
                                }
                                format!("{lines}{tag} OK expunged\r\n")
                            }
                            _ => format!("{tag} BAD not here\r\n"),
                        }
                    }
                }
                None => format!("{tag} BAD not here\r\n"),
            }
        }
        "EXPUNGE" | "CLOSE" => {
            // What Campbooks must never send: every `\Deleted` message of
            // the folder goes for good.
            effects.changes += 1;
            let Some(folder) = selected.clone() else {
                return format!("{tag} BAD nothing selected\r\n").into_bytes();
            };
            let lines = store.expunge_deleted(&folder, |_| true);
            if verb == "CLOSE" {
                *selected = None;
                format!("{tag} OK closed\r\n")
            } else {
                format!("{lines}{tag} OK expunged\r\n")
            }
        }
        "UNSELECT" if offers.unselect => {
            *selected = None;
            format!("{tag} OK unselected\r\n")
        }
        "CREATE" => {
            effects.changes += 1;
            let name = args.first().cloned().unwrap_or_default();
            if store
                .folders
                .iter()
                .any(|f| f.name.eq_ignore_ascii_case(&name))
            {
                format!("{tag} NO [ALREADYEXISTS] it is there\r\n")
            } else if offers.busy_writes {
                format!("{tag} NO [UNAVAILABLE] try again later\r\n")
            } else {
                store.add_folder(&name, &[]);
                store.created.push(name);
                format!("{tag} OK created\r\n")
            }
        }
        "SUBSCRIBE" => {
            store
                .subscribed
                .push(args.first().cloned().unwrap_or_default());
            format!("{tag} OK subscribed\r\n")
        }
        _ => {
            effects.changes += 1;
            format!("{tag} BAD not here\r\n")
        }
    };
    text.into_bytes()
}

/// `UID STORE set [(UNCHANGEDSINCE n)] ±FLAGS[.SILENT] (flags)` in `name`.
fn store_flags(
    store: &mut Store,
    offers: Offers,
    tag: &str,
    name: &str,
    args: &[String],
) -> String {
    let set = args.first().cloned().unwrap_or_default();
    let unchanged = args.get(1).and_then(|a| {
        a.trim_start_matches('(')
            .trim_end_matches(')')
            .to_ascii_uppercase()
            .strip_prefix("UNCHANGEDSINCE ")
            .and_then(|n| n.trim().parse::<u64>().ok())
    });
    let at = if unchanged.is_some() { 2 } else { 1 };
    let op = args
        .get(at)
        .map(|o| o.to_ascii_uppercase())
        .unwrap_or_default();
    let flags = args
        .get(at + 1)
        .map(|f| tokens(f.trim_start_matches('(').trim_end_matches(')')))
        .unwrap_or_default();
    let has = |flag: &str| flags.iter().any(|f| f.eq_ignore_ascii_case(flag));
    let on = !op.starts_with('-');
    let silent = op.ends_with(".SILENT");
    let uids: Vec<u32> = store
        .folder(name)
        .mails
        .iter()
        .filter(|m| in_set(&set, m.uid, u32::MAX))
        .map(|m| m.uid)
        .collect();
    let mut out = String::new();
    let mut modified = Vec::new();
    for uid in uids {
        if unchanged.is_some_and(|m| store.mail(name, uid).modseq > m) {
            modified.push(uid.to_string());
            continue;
        }
        let modseq = store.next_modseq();
        let seq = store
            .folder(name)
            .mails
            .iter()
            .position(|m| m.uid == uid)
            .unwrap_or_default()
            + 1;
        let mail = store.mail(name, uid);
        if has("\\Seen") {
            mail.seen = on;
        }
        if has("\\Flagged") {
            mail.flagged = on;
        }
        if has("\\Deleted") {
            mail.deleted = on;
        }
        mail.modseq = modseq;
        if !silent || (unchanged.is_some() && offers.condstore) {
            let _ = write!(out, "* {seq} FETCH (UID {uid} MODSEQ ({modseq}))\r\n");
        }
    }
    if modified.is_empty() {
        format!("{out}{tag} OK stored\r\n")
    } else {
        format!(
            "{out}{tag} OK [MODIFIED {}] conditional store failed\r\n",
            modified.join(",")
        )
    }
}

/// `UID COPY set dest` (or `UID MOVE`, the originals expunged): with
/// UIDPLUS, the new UIDs in `COPYUID`.
fn copy(
    store: &mut Store,
    offers: Offers,
    tag: &str,
    name: &str,
    args: &[String],
    moving: bool,
) -> String {
    let set = args.first().cloned().unwrap_or_default();
    let dest = args.get(1).cloned().unwrap_or_default();
    if !store.folders.iter().any(|f| f.name == dest) {
        return format!("{tag} NO [TRYCREATE] no such folder\r\n");
    }
    let uids: Vec<u32> = store
        .folder(name)
        .mails
        .iter()
        .filter(|m| in_set(&set, m.uid, u32::MAX))
        .map(|m| m.uid)
        .collect();
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for uid in &uids {
        let new = store.copy(name, *uid, &dest);
        pairs.push((*uid, new));
    }
    let uidvalidity = store.folder(&dest).uidvalidity;
    let reported = if offers.copyuid_partial {
        &pairs[..pairs.len().min(1)]
    } else {
        &pairs[..]
    };
    let from: Vec<String> = reported.iter().map(|p| p.0.to_string()).collect();
    let to: Vec<String> = reported
        .iter()
        .map(|p| if offers.copyuid_lies { 1 } else { p.1 }.to_string())
        .collect();
    let copyuid = if offers.uidplus && !offers.copyuid_missing && !pairs.is_empty() {
        format!(
            "[COPYUID {uidvalidity} {} {}] ",
            from.join(","),
            to.join(",")
        )
    } else {
        String::new()
    };
    if !moving {
        return format!("{tag} OK {copyuid}copied\r\n");
    }
    let mut out = String::new();
    if !copyuid.is_empty() {
        let _ = write!(out, "* OK {copyuid}moved\r\n");
    }
    for uid in uids {
        let seq = store
            .folder(name)
            .mails
            .iter()
            .position(|m| m.uid == uid)
            .unwrap_or_default()
            + 1;
        store.expunge(name, uid);
        let _ = write!(out, "* {seq} EXPUNGE\r\n");
    }
    if offers.extra_expunges {
        out.push_str("* 1 EXPUNGE\r\n");
    }
    format!("{out}{tag} OK moved\r\n")
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

fn list(store: &Store, offers: Offers, tag: &str, args: &[String]) -> String {
    let pattern = args.get(1).map_or("", String::as_str);
    if pattern.is_empty() {
        // The hierarchy delimiter, and the root.
        return format!("* LIST (\\Noselect) \".\" \"\"\r\n{tag} OK LIST done\r\n");
    }
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
    if store.refuse_open.contains(&name) {
        return format!("{tag} NO [SERVERBUG] cannot open it now\r\n");
    }
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
        "DELETED" => mail.deleted,
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
    let flags_only = items.len() == 2
        && items.iter().any(|i| i.eq_ignore_ascii_case("UID"))
        && items.iter().any(|i| i.eq_ignore_ascii_case("FLAGS"));
    if offers.hide_flags && flags_only && modifiers.is_empty() {
        return format!("{tag} OK FETCH done\r\n").into_bytes();
    }
    let mut out = Vec::new();
    if offers.chatty {
        for _ in 0..50_000 {
            out.extend_from_slice(b"* 1 EXISTS\r\n");
        }
    }
    if offers.vanish_flood && vanished && changed_since.is_some() {
        let line = format!("* VANISHED (EARLIER) {}1\r\n", "1,".repeat(4_999));
        for _ in 0..210 {
            out.extend_from_slice(line.as_bytes());
        }
    }
    let wants_body = items.iter().any(|item| {
        let upper = item.to_ascii_uppercase();
        upper == "BODY.PEEK[]" || upper == "BODY[]"
    });
    let stalled = store.stalled.clone();
    let folder = store.folder(name);
    let max = folder.mails.iter().map(|m| m.uid).max().unwrap_or(0);
    if wants_body
        && folder.mails.iter().any(|mail| {
            in_set(&set, mail.uid, max) && stalled.contains(&(name.to_owned(), mail.uid))
        })
    {
        return Vec::new();
    }
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
