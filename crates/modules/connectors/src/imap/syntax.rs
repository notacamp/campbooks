//! What an IMAP server says, parsed: just what reading a mailbox needs
//! (RFC 3501 §7 and §9, with CONDSTORE and QRESYNC, RFC 7162, and OBJECTID,
//! RFC 8474). One complete response at a time, as the session framed it
//! (its literals already bounded and inline), borrowed, never copied
//! except where a quoted string has escapes.
//!
//! The server is not trusted: lists nest at most [`MAX_DEPTH`] deep, every
//! number is checked, and anything unexpected is [`Malformed`] rather than
//! a panic. Responses the adapter has no use for are [`Untagged::Other`],
//! read no further.

use std::borrow::Cow;

use chrono::{DateTime, Utc};

/// Deepest nesting of parenthesised lists read (a FETCH's own list is 1).
pub const MAX_DEPTH: usize = 8;

/// Most entries of one capability list kept.
const MAX_CAPABILITIES: usize = 256;

/// Most ranges of one sequence set read.
const MAX_RANGES: usize = 100_000;

/// Most values one response is read into (a value takes some 30 bytes of
/// memory for as little as two of input): past it, the response is
/// [`Malformed`].
pub const MAX_VALUES: usize = 100_000;

/// Most items of one parenthesised list.
pub const MAX_ITEMS: usize = 10_000;

/// Most UIDs one `SEARCH` response is read into.
pub const MAX_SEARCH: usize = 1_000_000;

/// The response could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed;

/// One value of a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value<'a> {
    /// An atom or a number (`\Seen`, `42`, `BODY[HEADER]`).
    Atom(&'a [u8]),
    /// A quoted string or a literal.
    Str(Cow<'a, [u8]>),
    /// `NIL`.
    Nil,
    /// A parenthesised list.
    List(Vec<Value<'a>>),
}

impl<'a> Value<'a> {
    /// The atom's bytes.
    fn atom(&self) -> Option<&'a [u8]> {
        match self {
            Self::Atom(atom) => Some(*atom),
            _ => None,
        }
    }

    /// An atom's or a string's bytes.
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Atom(atom) => Some(*atom),
            Self::Str(text) => Some(text.as_ref()),
            Self::Nil | Self::List(_) => None,
        }
    }

    /// The number an atom spells (at most 20 digits).
    fn number(&self) -> Option<u64> {
        number(self.atom()?)
    }
}

/// A number of 1 to 20 ASCII digits.
pub fn number(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() || bytes.len() > 20 || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

/// A non-zero 32-bit number (a UID, a UIDVALIDITY).
fn nz32(bytes: &[u8]) -> Option<u32> {
    u32::try_from(number(bytes)?).ok().filter(|n| *n > 0)
}

/// How a command ended, or what an untagged status says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    No,
    Bad,
    Bye,
    Preauth,
}

/// Most UIDs a response code's set may name to be read (`COPYUID`,
/// `MODIFIED`): past it, the set is left unread.
pub const MAX_CODE_UIDS: u64 = 10_000;

/// A response code (`[UIDVALIDITY 3857529045]`): its name, upper-cased,
/// and the number after it, if any. Never the text around it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Code {
    pub name: String,
    pub number: Option<u64>,
    /// The UID sets of `COPYUID` (source, then destination; RFC 4315) or
    /// `MODIFIED` (RFC 7162), each as the UIDs it names in order; empty for
    /// any other code, or a set that names more than [`MAX_CODE_UIDS`].
    pub sets: Vec<Vec<u32>>,
}

/// One response.
#[derive(Debug, Clone, PartialEq)]
pub enum Response<'a> {
    /// `+ …`: the server waits for more of a command.
    Continue,
    /// The end of a command.
    Tagged {
        tag: &'a [u8],
        status: Status,
        code: Option<Code>,
    },
    /// Data, or a status, not tied to one command.
    Untagged(Untagged<'a>),
}

/// An untagged response the adapter reads.
#[derive(Debug, Clone, PartialEq)]
pub enum Untagged<'a> {
    /// `* OK [UIDNEXT 4392] …`, `* BYE …`.
    Status { status: Status, code: Option<Code> },
    /// `* CAPABILITY …` or `* ENABLED …`, upper-cased.
    Capabilities(Vec<String>),
    /// `* LIST (\HasNoChildren \Sent) "/" Sent`.
    List(ListEntry<'a>),
    /// `* STATUS INBOX (UIDNEXT 44 MESSAGES 3)`: the items, upper-cased.
    MailboxStatus {
        name: Cow<'a, [u8]>,
        items: Vec<(String, u64)>,
    },
    /// `* SEARCH 2 84 882`: non-zero numbers only.
    Search(Vec<u32>),
    /// `* 23 EXISTS`.
    Exists(u32),
    /// `* 7 EXPUNGE`: the message at that sequence number is gone.
    Expunge(u32),
    /// `* VANISHED (EARLIER) 41,43:116`: the UID ranges, inclusive.
    Vanished(Vec<(u32, u32)>),
    /// `* 12 FETCH (…)`.
    Fetch(Box<Fetch<'a>>),
    /// Anything else.
    Other,
}

/// A folder as `LIST` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry<'a> {
    /// Its attributes, lower-cased (`\noselect`, `\sent`).
    pub attributes: Vec<String>,
    /// The hierarchy delimiter, when it has one.
    pub delimiter: Option<char>,
    /// Its name as the server spells it.
    pub name: Cow<'a, [u8]>,
}

/// What a `FETCH` response holds, of what the adapter asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fetch<'a> {
    pub uid: Option<u32>,
    /// `\Seen` and `\Flagged`, when `FLAGS` came.
    pub flags: Option<(bool, bool)>,
    pub size: Option<u64>,
    pub internal_date: Option<DateTime<Utc>>,
    /// `BODY[HEADER…]`, `RFC822.HEADER`.
    pub header: Option<Cow<'a, [u8]>>,
    /// `BODY[]`, `RFC822`.
    pub body: Option<Cow<'a, [u8]>>,
    pub modseq: Option<u64>,
    /// RFC 8474 `EMAILID`.
    pub email_id: Option<String>,
}

/// Parse one complete response (with its line ending).
pub fn parse(response: &[u8]) -> Result<Response<'_>, Malformed> {
    let response = response
        .strip_suffix(b"\r\n")
        .or_else(|| response.strip_suffix(b"\n"))
        .unwrap_or(response);
    if response.starts_with(b"+") {
        return Ok(Response::Continue);
    }
    if let Some(rest) = response.strip_prefix(b"* ") {
        return untagged(rest).map(Response::Untagged);
    }
    let space = response.iter().position(|b| *b == b' ').ok_or(Malformed)?;
    let (tag, rest) = response.split_at(space);
    if tag.is_empty() {
        return Err(Malformed);
    }
    let (status, code) = status_and_code(&rest[1..])?;
    Ok(Response::Tagged { tag, status, code })
}

/// `OK [CODE n] text`: the status and the code.
fn status_and_code(text: &[u8]) -> Result<(Status, Option<Code>), Malformed> {
    let (word, rest) = split_word(text);
    let status = match word.to_ascii_uppercase().as_slice() {
        b"OK" => Status::Ok,
        b"NO" => Status::No,
        b"BAD" => Status::Bad,
        b"BYE" => Status::Bye,
        b"PREAUTH" => Status::Preauth,
        _ => return Err(Malformed),
    };
    Ok((status, code(rest)))
}

/// The response code at the start of `text`, if any.
fn code(text: &[u8]) -> Option<Code> {
    let inner = text.strip_prefix(b"[")?;
    let end = inner.iter().position(|b| *b == b']')?;
    let (name, argument) = split_word(&inner[..end]);
    let valid = !name.is_empty()
        && name.len() <= 32
        && name
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'));
    if !valid {
        return None;
    }
    let name = String::from_utf8_lossy(name).to_ascii_uppercase();
    let words: Vec<&[u8]> = argument
        .split(|b| *b == b' ')
        .filter(|w| !w.is_empty())
        .take(3)
        .collect();
    // Which words are UID sets: COPYUID's after its UIDVALIDITY, MODIFIED's
    // first.
    let set_words: &[&[u8]] = match name.as_str() {
        "COPYUID" if words.len() == 3 => &words[1..],
        "MODIFIED" if !words.is_empty() => &words[..1],
        _ => &[],
    };
    let sets = set_words
        .iter()
        .copied()
        .map(uid_list)
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    Some(Code {
        number: number(split_word(argument).0),
        name,
        sets,
    })
}

/// The UIDs a set names, in order (`3:5,9` → 3, 4, 5, 9; `5:3` → 5, 4, 3);
/// `None` when it is not a set or names more than [`MAX_CODE_UIDS`].
fn uid_list(word: &[u8]) -> Option<Vec<u32>> {
    let mut ranges = Vec::new();
    let mut count: u64 = 0;
    for part in word.split(|b| *b == b',') {
        let (a, b) = match part.iter().position(|b| *b == b':') {
            Some(at) => (nz32(&part[..at])?, nz32(&part[at + 1..])?),
            None => (nz32(part)?, nz32(part)?),
        };
        count = count.saturating_add(u64::from(a.abs_diff(b)) + 1);
        if count > MAX_CODE_UIDS {
            return None;
        }
        ranges.push((a, b));
    }
    let mut uids = Vec::with_capacity(usize::try_from(count).ok()?);
    for (a, b) in ranges {
        if a <= b {
            uids.extend(a..=b);
        } else {
            uids.extend((b..=a).rev());
        }
    }
    Some(uids)
}

/// The bytes up to the first space, and those after it.
fn split_word(text: &[u8]) -> (&[u8], &[u8]) {
    match text.iter().position(|b| *b == b' ') {
        Some(at) => (&text[..at], &text[at + 1..]),
        None => (text, &[]),
    }
}

fn untagged(text: &[u8]) -> Result<Untagged<'_>, Malformed> {
    let (word, rest) = split_word(text);
    if let Some(n) = number(word) {
        let n = u32::try_from(n).map_err(|_| Malformed)?;
        let (kind, rest) = split_word(rest);
        return match kind.to_ascii_uppercase().as_slice() {
            b"EXISTS" => Ok(Untagged::Exists(n)),
            b"EXPUNGE" => Ok(Untagged::Expunge(n)),
            b"FETCH" => fetch(rest).map(|f| Untagged::Fetch(Box::new(f))),
            _ => Ok(Untagged::Other),
        };
    }
    match word.to_ascii_uppercase().as_slice() {
        b"OK" | b"NO" | b"BAD" | b"BYE" | b"PREAUTH" => {
            let (status, code) = status_and_code(text)?;
            Ok(Untagged::Status { status, code })
        }
        b"CAPABILITY" | b"ENABLED" => Ok(Untagged::Capabilities(
            rest.split(|b| *b == b' ')
                .filter(|w| !w.is_empty())
                .take(MAX_CAPABILITIES)
                .map(|w| String::from_utf8_lossy(w).to_ascii_uppercase())
                .collect(),
        )),
        b"LIST" | b"LSUB" => list(rest).map(Untagged::List),
        b"STATUS" => mailbox_status(rest),
        b"SEARCH" => {
            let mut uids = Vec::new();
            for word in rest
                .split(|b| *b == b' ')
                .take_while(|w| !w.starts_with(b"("))
            {
                if uids.len() >= MAX_SEARCH {
                    return Err(Malformed);
                }
                uids.extend(nz32(word));
            }
            Ok(Untagged::Search(uids))
        }
        b"VANISHED" => {
            let rest = rest
                .strip_prefix(b"(EARLIER) ")
                .or_else(|| rest.strip_prefix(b"(earlier) "))
                .unwrap_or(rest);
            sequence_set(split_word(rest).0).map(Untagged::Vanished)
        }
        _ => Ok(Untagged::Other),
    }
}

/// A sequence set of UIDs (`1:5,7,9:12`); `*` is never sent by a server.
fn sequence_set(text: &[u8]) -> Result<Vec<(u32, u32)>, Malformed> {
    let mut ranges = Vec::new();
    for part in text.split(|b| *b == b',') {
        if ranges.len() >= MAX_RANGES {
            return Err(Malformed);
        }
        let (a, b) = match part.iter().position(|b| *b == b':') {
            Some(at) => (nz32(&part[..at]), nz32(&part[at + 1..])),
            None => (nz32(part), nz32(part)),
        };
        let (Some(a), Some(b)) = (a, b) else {
            return Err(Malformed);
        };
        ranges.push((a.min(b), a.max(b)));
    }
    Ok(ranges)
}

fn list(text: &[u8]) -> Result<ListEntry<'_>, Malformed> {
    let mut parser = Parser::new(text);
    let Value::List(attributes) = parser.value(0)? else {
        return Err(Malformed);
    };
    parser.space()?;
    let delimiter = match parser.value(0)? {
        Value::Nil => None,
        Value::Str(d) if d.len() == 1 && d[0].is_ascii() && d[0] != b'\r' && d[0] != b'\n' => {
            Some(char::from(d[0]))
        }
        _ => return Err(Malformed),
    };
    parser.space()?;
    let name = match parser.value(0)? {
        Value::Atom(atom) => Cow::Borrowed(atom),
        Value::Str(name) => name,
        Value::Nil | Value::List(_) => return Err(Malformed),
    };
    Ok(ListEntry {
        attributes: attributes
            .iter()
            .filter_map(Value::atom)
            .take(MAX_CAPABILITIES)
            .map(|a| String::from_utf8_lossy(a).to_ascii_lowercase())
            .collect(),
        delimiter,
        name,
    })
}

fn mailbox_status(text: &[u8]) -> Result<Untagged<'_>, Malformed> {
    let mut parser = Parser::new(text);
    let name = match parser.value(0)? {
        Value::Atom(atom) => Cow::Borrowed(atom),
        Value::Str(name) => name,
        Value::Nil | Value::List(_) => return Err(Malformed),
    };
    parser.space()?;
    let Value::List(values) = parser.value(0)? else {
        return Err(Malformed);
    };
    let items = values
        .chunks(2)
        .filter_map(|pair| match pair {
            [key, value] => Some((
                String::from_utf8_lossy(key.atom()?).to_ascii_uppercase(),
                value.number()?,
            )),
            _ => None,
        })
        .collect();
    Ok(Untagged::MailboxStatus { name, items })
}

fn fetch(text: &[u8]) -> Result<Fetch<'_>, Malformed> {
    let mut parser = Parser::new(text);
    let Value::List(values) = parser.value(0)? else {
        return Err(Malformed);
    };
    if values.len() % 2 != 0 {
        return Err(Malformed);
    }
    let mut fetch = Fetch::default();
    let mut pairs = values.into_iter();
    while let (Some(key), Some(value)) = (pairs.next(), pairs.next()) {
        let key = key.atom().ok_or(Malformed)?.to_ascii_uppercase();
        fetch_item(&mut fetch, &key, value)?;
    }
    Ok(fetch)
}

/// Read one `FETCH` item into `fetch`.
fn fetch_item<'a>(fetch: &mut Fetch<'a>, key: &[u8], value: Value<'a>) -> Result<(), Malformed> {
    match key {
        b"UID" => fetch.uid = Some(value.atom().and_then(nz32).ok_or(Malformed)?),
        b"FLAGS" => {
            let Value::List(flags) = value else {
                return Err(Malformed);
            };
            let has = |flag: &[u8]| {
                flags
                    .iter()
                    .filter_map(Value::atom)
                    .any(|f| f.eq_ignore_ascii_case(flag))
            };
            fetch.flags = Some((has(b"\\Seen"), has(b"\\Flagged")));
        }
        b"RFC822.SIZE" => fetch.size = Some(value.number().ok_or(Malformed)?),
        b"INTERNALDATE" => {
            fetch.internal_date = value.bytes().and_then(internal_date);
        }
        b"MODSEQ" => {
            fetch.modseq = match value {
                Value::List(items) => items.first().and_then(Value::number),
                _ => None,
            };
        }
        b"EMAILID" => {
            fetch.email_id = match value {
                Value::List(items) => items.first().and_then(Value::atom).and_then(object_id),
                _ => None,
            };
        }
        b"RFC822" => fetch.body = string(value),
        b"RFC822.HEADER" => fetch.header = string(value),
        _ if key.starts_with(b"BODY[") || key.starts_with(b"BINARY[") => {
            let section = key
                .iter()
                .position(|b| *b == b'[')
                .and_then(|open| {
                    let close = key.iter().rposition(|b| *b == b']')?;
                    key.get(open + 1..close)
                })
                .unwrap_or_default();
            if section.is_empty() {
                fetch.body = string(value);
            } else if section.starts_with(b"HEADER") {
                fetch.header = string(value);
            }
        }
        _ => {}
    }
    Ok(())
}

/// A string value's bytes (`NIL` is none).
fn string(value: Value<'_>) -> Option<Cow<'_, [u8]>> {
    match value {
        Value::Str(bytes) => Some(bytes),
        Value::Atom(atom) => Some(Cow::Borrowed(atom)),
        Value::Nil | Value::List(_) => None,
    }
}

/// An RFC 8474 object id: 1 to 255 letters, digits, `_` and `-`.
fn object_id(atom: &[u8]) -> Option<String> {
    let valid = !atom.is_empty()
        && atom.len() <= 255
        && atom
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'));
    valid.then(|| String::from_utf8_lossy(atom).into_owned())
}

/// An `INTERNALDATE` (`17-Jul-1996 02:44:25 -0700`).
fn internal_date(bytes: &[u8]) -> Option<DateTime<Utc>> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    // The day may be one digit (` 7-Jul-…`).
    let text = if text.as_bytes().get(1) == Some(&b'-') {
        format!("0{text}")
    } else {
        text.to_owned()
    };
    DateTime::parse_from_str(&text, "%d-%b-%Y %H:%M:%S %z")
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// Reads values off a response, bounded by its length and [`MAX_DEPTH`].
struct Parser<'a> {
    input: &'a [u8],
    at: usize,
    /// Values still allowed ([`MAX_VALUES`] for the response).
    budget: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            at: 0,
            budget: MAX_VALUES,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.at).copied()
    }

    /// One or more spaces.
    fn space(&mut self) -> Result<(), Malformed> {
        if self.peek() != Some(b' ') {
            return Err(Malformed);
        }
        while self.peek() == Some(b' ') {
            self.at += 1;
        }
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<Value<'a>, Malformed> {
        self.budget = self.budget.checked_sub(1).ok_or(Malformed)?;
        match self.peek().ok_or(Malformed)? {
            b'(' => self.list(depth + 1),
            b'"' => self.quoted(),
            b'{' => self.literal(),
            b')' | b' ' | b'\r' | b'\n' => Err(Malformed),
            _ => Ok(self.atom()),
        }
    }

    fn list(&mut self, depth: usize) -> Result<Value<'a>, Malformed> {
        if depth > MAX_DEPTH {
            return Err(Malformed);
        }
        self.at += 1;
        let mut items = Vec::new();
        loop {
            while self.peek() == Some(b' ') {
                self.at += 1;
            }
            match self.peek().ok_or(Malformed)? {
                b')' => {
                    self.at += 1;
                    return Ok(Value::List(items));
                }
                _ if items.len() >= MAX_ITEMS => return Err(Malformed),
                _ => items.push(self.value(depth)?),
            }
        }
    }

    fn quoted(&mut self) -> Result<Value<'a>, Malformed> {
        self.at += 1;
        let start = self.at;
        let mut owned: Option<Vec<u8>> = None;
        loop {
            let byte = self.peek().ok_or(Malformed)?;
            self.at += 1;
            match byte {
                b'"' => {
                    let end = self.at - 1;
                    return Ok(Value::Str(owned.map_or_else(
                        || Cow::Borrowed(&self.input[start..end]),
                        Cow::Owned,
                    )));
                }
                b'\\' => {
                    let escaped = self.peek().ok_or(Malformed)?;
                    let buffer =
                        owned.get_or_insert_with(|| self.input[start..self.at - 1].to_vec());
                    buffer.push(escaped);
                    self.at += 1;
                }
                b'\r' | b'\n' => return Err(Malformed),
                other => {
                    if let Some(buffer) = owned.as_mut() {
                        buffer.push(other);
                    }
                }
            }
        }
    }

    fn literal(&mut self) -> Result<Value<'a>, Malformed> {
        let rest = &self.input[self.at + 1..];
        let close = rest
            .iter()
            .take(22)
            .position(|b| *b == b'}')
            .ok_or(Malformed)?;
        let digits = rest[..close].strip_suffix(b"+").unwrap_or(&rest[..close]);
        let size = usize::try_from(number(digits).ok_or(Malformed)?).map_err(|_| Malformed)?;
        let after = &rest[close + 1..];
        let skip = if after.starts_with(b"\r\n") {
            2
        } else if after.starts_with(b"\n") {
            1
        } else {
            return Err(Malformed);
        };
        let start = self.at + 1 + close + 1 + skip;
        let end = start.checked_add(size).ok_or(Malformed)?;
        let bytes = self.input.get(start..end).ok_or(Malformed)?;
        self.at = end;
        Ok(Value::Str(Cow::Borrowed(bytes)))
    }

    /// An atom: up to a space, a parenthesis or the end; a `[` section is
    /// read whole (it may hold spaces and lists), then any `<origin>`.
    fn atom(&mut self) -> Value<'a> {
        let start = self.at;
        while let Some(byte) = self.peek() {
            match byte {
                b' ' | b'(' | b')' | b'\r' | b'\n' | b'"' | b'{' => break,
                b'[' => {
                    self.at += 1;
                    let mut quoted = false;
                    while let Some(inner) = self.peek() {
                        self.at += 1;
                        match inner {
                            b'"' => quoted = !quoted,
                            b']' if !quoted => break,
                            _ => {}
                        }
                    }
                }
                _ => self.at += 1,
            }
        }
        let atom = &self.input[start..self.at];
        if atom.eq_ignore_ascii_case(b"NIL") {
            Value::Nil
        } else {
            Value::Atom(atom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn untagged_of(line: &str) -> Untagged<'_> {
        match parse(line.as_bytes()).unwrap() {
            Response::Untagged(u) => u,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tagged_lines_keep_only_their_status_and_code() {
        assert_eq!(
            parse(b"w3 NO [UNAVAILABLE] ana@example.com is busy\r\n").unwrap(),
            Response::Tagged {
                tag: b"w3",
                status: Status::No,
                code: Some(Code {
                    name: "UNAVAILABLE".into(),
                    number: None,
                    sets: Vec::new(),
                }),
            }
        );
        assert_eq!(
            parse(b"w4 OK done\r\n").unwrap(),
            Response::Tagged {
                tag: b"w4",
                status: Status::Ok,
                code: None
            }
        );
        assert!(parse(b"w5 MAYBE\r\n").is_err());
        assert_eq!(parse(b"+ go on\r\n").unwrap(), Response::Continue);
    }

    #[test]
    fn statuses_lists_and_searches_are_read() {
        assert_eq!(
            untagged_of("* OK [UIDVALIDITY 3857529045] UIDs valid\r\n"),
            Untagged::Status {
                status: Status::Ok,
                code: Some(Code {
                    name: "UIDVALIDITY".into(),
                    number: Some(3_857_529_045),
                    sets: Vec::new(),
                })
            }
        );
        let Untagged::List(entry) =
            untagged_of("* LIST (\\HasNoChildren \\Sent) \"/\" \"Sent Items\"\r\n")
        else {
            panic!()
        };
        assert_eq!(entry.attributes, ["\\hasnochildren", "\\sent"]);
        assert_eq!(entry.delimiter, Some('/'));
        assert_eq!(entry.name.as_ref(), b"Sent Items");
        let Untagged::List(entry) = untagged_of("* LIST () NIL {5}\r\nINBOX\r\n") else {
            panic!()
        };
        assert_eq!(
            (entry.delimiter, entry.name.as_ref()),
            (None, &b"INBOX"[..])
        );
        assert_eq!(
            untagged_of("* STATUS \"A \\\"b\\\"\" (MESSAGES 3 UIDNEXT 44 HIGHESTMODSEQ 9)\r\n"),
            Untagged::MailboxStatus {
                name: Cow::Owned(b"A \"b\"".to_vec()),
                items: vec![
                    ("MESSAGES".into(), 3),
                    ("UIDNEXT".into(), 44),
                    ("HIGHESTMODSEQ".into(), 9)
                ]
            }
        );
        assert_eq!(
            untagged_of("* SEARCH 2 84 0 x 882 (MODSEQ 917162500)\r\n"),
            Untagged::Search(vec![2, 84, 882])
        );
        assert_eq!(
            untagged_of("* VANISHED (EARLIER) 41,43:116,200:190\r\n"),
            Untagged::Vanished(vec![(41, 41), (43, 116), (190, 200)])
        );
        assert_eq!(untagged_of("* 7 EXISTS\r\n"), Untagged::Exists(7));
        assert_eq!(untagged_of("* 7 RECENT\r\n"), Untagged::Other);
        assert_eq!(untagged_of("* 3 EXPUNGE\r\n"), Untagged::Expunge(3));
        assert_eq!(
            untagged_of("* CAPABILITY IMAP4rev1 condstore QRESYNC\r\n"),
            Untagged::Capabilities(vec![
                "IMAP4REV1".into(),
                "CONDSTORE".into(),
                "QRESYNC".into()
            ])
        );
    }

    #[test]
    fn a_fetch_is_read_with_its_literals() {
        let header = "Message-ID: <a@example.com>\r\n\r\n";
        let line = format!(
            "* 3 FETCH (UID 41 FLAGS (\\Seen $Junk) RFC822.SIZE 1200 \
             INTERNALDATE \" 7-Jul-2026 02:44:25 -0700\" MODSEQ (12) EMAILID (M6d99ac3275bb4e) \
             BODY[HEADER.FIELDS (MESSAGE-ID DATE)] {{{}}}\r\n{header} BODY[] NIL)\r\n",
            header.len()
        );
        let Untagged::Fetch(fetch) = untagged_of(&line) else {
            panic!()
        };
        assert_eq!(fetch.uid, Some(41));
        assert_eq!(fetch.flags, Some((true, false)));
        assert_eq!(fetch.size, Some(1200));
        assert_eq!(
            fetch.internal_date.unwrap().to_rfc3339(),
            "2026-07-07T09:44:25+00:00"
        );
        assert_eq!(fetch.modseq, Some(12));
        assert_eq!(fetch.email_id.as_deref(), Some("M6d99ac3275bb4e"));
        assert_eq!(fetch.header.as_deref(), Some(header.as_bytes()));
        assert_eq!(fetch.body, None);
    }

    #[test]
    fn copyuid_and_modified_keep_their_uid_sets() {
        let Response::Tagged { code, .. } =
            parse(b"w7 OK [COPYUID 38505 304,319:320 3956:3958] Done\r\n").unwrap()
        else {
            panic!()
        };
        let code = code.unwrap();
        assert_eq!(code.name, "COPYUID");
        assert_eq!(code.number, Some(38_505));
        assert_eq!(code.sets, [vec![304, 319, 320], vec![3956, 3957, 3958]]);
        let Untagged::Status { code, .. } = untagged_of("* OK [COPYUID 7 9:7 1,2,3] Moved\r\n")
        else {
            panic!()
        };
        assert_eq!(code.unwrap().sets, [vec![9, 8, 7], vec![1, 2, 3]]);
        let Response::Tagged { code, .. } =
            parse(b"w8 OK [MODIFIED 7,9] Conditional STORE failed\r\n").unwrap()
        else {
            panic!()
        };
        assert_eq!(code.unwrap().sets, [vec![7, 9]]);
        // A set naming more UIDs than are ever asked about is not read.
        let Response::Tagged { code, .. } =
            parse(b"w9 OK [COPYUID 1 1:4294967295 1:4294967295] Done\r\n").unwrap()
        else {
            panic!()
        };
        assert!(code.unwrap().sets.is_empty());
        // Other codes keep none; a broken set none either.
        let Response::Tagged { code, .. } = parse(b"w1 OK [UIDNEXT 4 5] x\r\n").unwrap() else {
            panic!()
        };
        assert!(code.unwrap().sets.is_empty());
        let Response::Tagged { code, .. } = parse(b"w2 OK [COPYUID 1 a:b 1] x\r\n").unwrap() else {
            panic!()
        };
        assert!(code.unwrap().sets.is_empty());
    }

    #[test]
    fn hostile_responses_are_refused_not_followed() {
        // Nesting past the limit.
        let deep = format!("* 1 FETCH (X {}{})\r\n", "(".repeat(64), ")".repeat(64));
        assert!(parse(deep.as_bytes()).is_err());
        // A literal longer than what came.
        assert!(parse(b"* 1 FETCH (BODY[] {99}\r\nshort)\r\n").is_err());
        // A literal size past any memory.
        assert!(parse(b"* 1 FETCH (BODY[] {99999999999999999999999}\r\n)\r\n").is_err());
        // An odd item list, a UID of zero, an unterminated string.
        assert!(parse(b"* 1 FETCH (UID)\r\n").is_err());
        assert!(parse(b"* 1 FETCH (UID 0)\r\n").is_err());
        assert!(parse(b"* LIST () \"/ INBOX\r\n").is_err());
        assert!(parse(b"* VANISHED 1:*\r\n").is_err());
        // A message number past 32 bits.
        assert!(parse(b"* 99999999999 EXISTS\r\n").is_err());
        // A list of more items than any answer has, or more values than
        // the response is allowed: refused before they take memory.
        let wide = format!("* 1 FETCH (X ({}))\r\n", "a ".repeat(MAX_ITEMS + 1));
        assert!(parse(wide.as_bytes()).is_err());
        let many = format!(
            "* 1 FETCH (X ({}) Y ({}))\r\n",
            "a ".repeat(MAX_ITEMS - 1),
            "(a a a a a a a a a) ".repeat(MAX_VALUES / 10)
        );
        assert!(parse(many.as_bytes()).is_err());
        let search = format!("* SEARCH {}\r\n", "7 ".repeat(MAX_SEARCH + 1));
        assert!(parse(search.as_bytes()).is_err());
    }
}
