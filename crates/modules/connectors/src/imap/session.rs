//! One conversation with an IMAP server, signed in: Campbooks' own wrapper
//! over the `async-imap` client (what reading, write-back and sending will
//! grow, so the client crate stays swappable).
//!
//! Opening one: connect to the address the guard checked; TLS from the
//! first byte, or STARTTLS before anything else is said (refused when the
//! server does not offer it: no password ever goes in the clear); then
//! `CAPABILITY`, and `AUTHENTICATE PLAIN` when offered, else `LOGIN`
//! (refused when `LOGINDISABLED` persists). Nothing the server says is kept,
//! returned or logged, except the response code of a refused sign-in.
//!
//! Signed in, commands are Campbooks' own ([`Session::command`]): the
//! client crate buffers a response whole before it parses it, up to
//! 512 MiB, sized by the server's own word, so reading never goes through
//! it. Every response is framed here within the caller's [`Limits`] (its
//! size, every literal's size before a byte of it is read, the number of
//! responses, the time), then parsed (`syntax`). A server that lies or
//! never stops talking ends the command with a [`WireError`].

use std::time::Duration;

use async_imap::error::Error as ImapError;
use futures_util::TryStreamExt as _;
use secrecy::{ExposeSecret as _, SecretString};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::TcpStream,
    time::{Instant, timeout, timeout_at},
};
use tokio_rustls::{TlsConnector, client::TlsStream};

use super::{
    Imap, Refusal, Security, Server,
    syntax::{self, Code, Response, Status, Untagged},
    tls,
};

/// The longest line read before signing in.
const MAX_LINE: usize = 8 * 1024;

/// The most lines one command's answer may take before signing in.
const MAX_LINES: usize = 64;

/// How much reading the socket asks for at once.
const CHUNK: usize = 64 * 1024;

/// The response codes of a server that is busy rather than refusing
/// (RFC 5530): `UNAVAILABLE`, `INUSE`, `LIMIT`.
pub const BUSY_CODES: [&str; 3] = ["UNAVAILABLE", "INUSE", "LIMIT"];

/// A signed-in IMAP session.
#[derive(Debug)]
pub struct Session {
    inner: async_imap::Session<TlsStream<TcpStream>>,
    /// Bytes read from the socket and not yet framed, from `start`.
    pending: Vec<u8>,
    start: usize,
    /// The last tag sent.
    tags: u32,
    /// Every byte read since signing in.
    received: u64,
    /// How long one read of the socket may wait.
    read_timeout: Duration,
    /// A command was cut short: the session is mid-answer and is never
    /// used again (not even to log out).
    broken: bool,
}

/// The bounds of one command's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest single response, line and literals, in bytes.
    pub response: usize,
    /// The most untagged responses.
    pub responses: usize,
    /// The most bytes of all its responses.
    pub total: usize,
    /// How long the whole command may take.
    pub time: Duration,
}

/// Why a command was cut short: a stable code, never the server's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// The server did not answer in time.
    TimedOut,
    /// The connection closed, or the server said goodbye.
    Closed,
    /// A response (or a literal in it) larger than the command allows.
    TooLarge,
    /// More responses, or bytes, than the command allows.
    TooMany,
    /// A response that is not IMAP.
    Malformed,
}

impl WireError {
    /// The stable code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::TimedOut => "imap_timed_out",
            Self::Closed => "imap_closed",
            Self::TooLarge => "imap_too_large",
            Self::TooMany => "imap_too_many_responses",
            Self::Malformed => "imap_malformed",
        }
    }
}

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    /// `OK`, `NO` or `BAD`.
    pub status: Status,
    /// Its response code, if any.
    pub code: Option<Code>,
}

impl Done {
    /// Whether it succeeded.
    pub fn is_ok(&self) -> bool {
        self.status == Status::Ok
    }

    /// Whether the server refused it for being busy ([`BUSY_CODES`]).
    pub fn is_busy(&self) -> bool {
        self.status != Status::Ok
            && self
                .code
                .as_ref()
                .is_some_and(|code| BUSY_CODES.contains(&code.name.as_str()))
    }
}

impl Session {
    /// Connect to `server` and sign in as `username` with `password`.
    pub async fn open(
        imap: &Imap,
        server: &Server,
        username: &str,
        password: &SecretString,
    ) -> Result<Self, Refusal> {
        let address = imap
            .reach(server)
            .await
            .map_err(|refused| Refusal::of(refused, Refusal::ImapUnreachable))?;
        let name = tls::server_name(&server.host).ok_or(Refusal::HostRefused)?;
        let connector = TlsConnector::from(
            tls::client_config(&imap.trust).map_err(|_| Refusal::ImapTlsFailed)?,
        );
        let mut tcp = timeout(imap.connect_timeout(), TcpStream::connect(address))
            .await
            .map_err(|_| Refusal::ImapUnreachable)?
            .map_err(|_| Refusal::ImapUnreachable)?;
        let mut stream = match server.security {
            Security::Tls => {
                let mut stream = connector
                    .connect(name, tcp)
                    .await
                    .map_err(|e| handshake(&e))?;
                greeting(&mut stream).await?;
                stream
            }
            Security::Starttls => {
                greeting(&mut tcp).await?;
                let capabilities = capabilities(&mut tcp, "c1").await?;
                if !capabilities.iter().any(|c| c == "STARTTLS") {
                    // Nothing more is said: no password in the clear.
                    let _ = tcp.write_all(b"c2 LOGOUT\r\n").await;
                    return Err(Refusal::StarttlsMissing);
                }
                command(&mut tcp, "c2", "STARTTLS").await?;
                connector
                    .connect(name, tcp)
                    .await
                    .map_err(|e| handshake(&e))?
            }
        };
        let capabilities = capabilities(&mut stream, "c3").await?;
        let client = async_imap::Client::new(stream);
        let signed_in = if capabilities.iter().any(|c| c == "AUTH=PLAIN") {
            client
                .authenticate("PLAIN", Plain { username, password })
                .await
        } else if capabilities.iter().any(|c| c == "LOGINDISABLED") {
            return Err(Refusal::ImapLoginRefused);
        } else {
            client.login(username, password.expose_secret()).await
        };
        let inner = signed_in.map_err(|(err, _)| refused(&err))?;
        Ok(Self {
            inner,
            pending: Vec::new(),
            start: 0,
            tags: 0,
            received: 0,
            read_timeout: imap.timeout(),
            broken: false,
        })
    }

    /// Every byte read from the server since signing in.
    pub fn received(&self) -> u64 {
        self.received
    }

    /// Send `command` (built by the caller from numbers, atoms and
    /// [`quoted`] strings only) and hand each untagged response to `each`,
    /// within `limits`. How it ended; `Err` when it was cut short, after
    /// which the session must not be used again.
    pub async fn command<F>(
        &mut self,
        command: &str,
        limits: Limits,
        each: F,
    ) -> Result<Done, WireError>
    where
        F: FnMut(Untagged<'_>),
    {
        if self.broken {
            return Err(WireError::Closed);
        }
        let done = self.exchange(command, limits, each).await;
        self.broken = done.is_err();
        done
    }

    /// [`command`](Self::command), on a session in step.
    async fn exchange<F>(
        &mut self,
        command: &str,
        limits: Limits,
        mut each: F,
    ) -> Result<Done, WireError>
    where
        F: FnMut(Untagged<'_>),
    {
        self.tags = self.tags.wrapping_add(1);
        let tag = format!("w{}", self.tags);
        let deadline = Instant::now() + limits.time;
        let line = format!("{tag} {command}\r\n");
        let stream = self.inner.get_mut();
        let written = timeout_at(deadline, async {
            stream.write_all(line.as_bytes()).await?;
            stream.flush().await?;
            Ok::<(), std::io::Error>(())
        })
        .await;
        match written {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(WireError::Closed),
            Err(_) => return Err(WireError::TimedOut),
        }
        let mut total = 0_usize;
        let mut count = 0_usize;
        loop {
            let response = self.response(limits.response, deadline).await?;
            total = total.saturating_add(response.len());
            if total > limits.total {
                return Err(WireError::TooMany);
            }
            match syntax::parse(&response).map_err(|_| WireError::Malformed)? {
                Response::Tagged {
                    tag: done,
                    status,
                    code,
                } if done == tag.as_bytes() => return Ok(Done { status, code }),
                Response::Tagged { .. } | Response::Continue => {
                    return Err(WireError::Malformed);
                }
                Response::Untagged(Untagged::Status {
                    status: Status::Bye,
                    ..
                }) => return Err(WireError::Closed),
                Response::Untagged(untagged) => {
                    count += 1;
                    if count > limits.responses {
                        return Err(WireError::TooMany);
                    }
                    each(untagged);
                }
            }
        }
    }

    /// One whole response: its line, and each literal it announces (its
    /// size checked against `max` before any of it is read).
    async fn response(&mut self, max: usize, deadline: Instant) -> Result<Vec<u8>, WireError> {
        let mut out = Vec::new();
        loop {
            // How far past `start` the line ending was looked for (`fill`
            // may move `start`, never what lies after it).
            let mut scanned = 0;
            let end = loop {
                let from = self.start + scanned;
                if let Some(at) = self.pending[from..].iter().position(|b| *b == b'\n') {
                    break from + at;
                }
                scanned = self.pending.len() - self.start;
                if out.len().saturating_add(scanned) > max {
                    return Err(WireError::TooLarge);
                }
                self.fill(deadline).await?;
            };
            out.extend_from_slice(&self.pending[self.start..=end]);
            self.start = end + 1;
            if out.len() > max {
                return Err(WireError::TooLarge);
            }
            let Some(size) = literal_size(&out) else {
                return Ok(out);
            };
            if size > max.saturating_sub(out.len()) {
                return Err(WireError::TooLarge);
            }
            out.reserve(size);
            let mut left = size;
            while left > 0 {
                if self.start == self.pending.len() {
                    self.fill(deadline).await?;
                }
                let take = left.min(self.pending.len() - self.start);
                out.extend_from_slice(&self.pending[self.start..self.start + take]);
                self.start += take;
                left -= take;
            }
        }
    }

    /// Read more of the socket into the pending bytes.
    async fn fill(&mut self, deadline: Instant) -> Result<(), WireError> {
        if self.start > 0 && self.start == self.pending.len() {
            self.pending.clear();
            self.start = 0;
        } else if self.start > CHUNK {
            self.pending.drain(..self.start);
            self.start = 0;
        }
        let mut chunk = vec![0_u8; CHUNK];
        let until = deadline.min(Instant::now() + self.read_timeout);
        let read = timeout_at(until, self.inner.get_mut().read(&mut chunk))
            .await
            .map_err(|_| WireError::TimedOut)?
            .map_err(|_| WireError::Closed)?;
        if read == 0 {
            return Err(WireError::Closed);
        }
        self.pending.extend_from_slice(&chunk[..read]);
        self.received = self
            .received
            .saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        Ok(())
    }

    /// The capabilities the server offers now that it is signed in to,
    /// upper-cased.
    pub async fn capabilities(&mut self, limits: Limits) -> Result<Vec<String>, WireError> {
        let mut capabilities = Vec::new();
        let done = self
            .command("CAPABILITY", limits, |untagged| {
                if let Untagged::Capabilities(list) = untagged {
                    capabilities.extend(list);
                }
            })
            .await?;
        if !done.is_ok() {
            return Err(WireError::Malformed);
        }
        Ok(capabilities)
    }

    /// Whether the mailbox lists its inbox (`LIST "" INBOX` answers).
    pub async fn list_inbox(&mut self) -> Result<(), Refusal> {
        let names = self
            .inner
            .list(Some(""), Some("INBOX"))
            .await
            .map_err(|err| refused(&err))?;
        let _names: Vec<_> = names.try_collect().await.map_err(|err| refused(&err))?;
        Ok(())
    }

    /// Say goodbye, briefly; whatever the server answers. A session cut
    /// short is only closed.
    pub async fn logout(mut self) {
        let limits = Limits {
            response: 4 * 1024,
            responses: 16,
            total: 64 * 1024,
            time: Duration::from_secs(5),
        };
        let _ = self.command("LOGOUT", limits, |_| {}).await;
    }
}

/// Sign in to `server` as `username`, list the inbox and log out, within
/// the configured time.
pub async fn verify(
    imap: &Imap,
    server: &Server,
    username: &str,
    password: &SecretString,
) -> Result<(), Refusal> {
    timeout(imap.timeout(), async {
        let mut session = Session::open(imap, server, username, password).await?;
        session.list_inbox().await?;
        session.logout().await;
        Ok::<(), Refusal>(())
    })
    .await
    .unwrap_or(Err(Refusal::ImapUnreachable))
}

/// `AUTHENTICATE PLAIN` (RFC 4616): no authorization identity, the
/// username and the password.
struct Plain<'a> {
    username: &'a str,
    password: &'a SecretString,
}

impl async_imap::Authenticator for Plain<'_> {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Vec<u8> {
        let mut response = Vec::with_capacity(self.username.len() + 64);
        response.push(0);
        response.extend_from_slice(self.username.as_bytes());
        response.push(0);
        response.extend_from_slice(self.password.expose_secret().as_bytes());
        response
    }
}

/// The size of the literal `line` ends with (`{12}` or `{12+}`, then the
/// line ending), if it announces one.
fn literal_size(line: &[u8]) -> Option<usize> {
    let line = line
        .strip_suffix(b"\r\n")
        .or_else(|| line.strip_suffix(b"\n"))?
        .strip_suffix(b"}")?;
    let open = line.iter().rposition(|b| *b == b'{')?;
    let digits = &line[open + 1..];
    let digits = digits.strip_suffix(b"+").unwrap_or(digits);
    usize::try_from(syntax::number(digits)?).ok()
}

/// `text` as an IMAP quoted string; `None` when it cannot be one (a line
/// break, a NUL, or anything outside ASCII: such names and ids are left
/// alone rather than sent as literals).
pub fn quoted(text: &str) -> Option<String> {
    if text.len() > 2048 || !text.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    Some(out)
}

/// What a failed TLS handshake means: a certificate (or TLS) problem, or a
/// server that went away.
fn handshake(err: &std::io::Error) -> Refusal {
    if tls::is_tls_failure(err) {
        Refusal::ImapTlsFailed
    } else {
        Refusal::ImapUnreachable
    }
}

/// What an error of the IMAP client means. A refused sign-in logs its
/// response code only, never the server's text.
fn refused(err: &ImapError) -> Refusal {
    match err {
        ImapError::No(answer) | ImapError::Bad(answer) => {
            let code = response_code(answer);
            if BUSY_CODES
                .iter()
                .any(|busy| code.eq_ignore_ascii_case(busy))
            {
                tracing::info!(code, "the IMAP server is busy");
                return Refusal::ImapBusy;
            }
            tracing::info!(code, "the IMAP server refused the sign-in");
            Refusal::ImapLoginRefused
        }
        ImapError::Validate(_) => Refusal::ImapLoginRefused,
        ImapError::Parse(_) => Refusal::ImapNotAnImapServer,
        ImapError::Io(io) if tls::is_tls_failure(io) => Refusal::ImapTlsFailed,
        _ => Refusal::ImapUnreachable,
    }
}

/// The response code of a refused command as the client reports it: one
/// it knows (`code: Some(Alert)`), else one left at the start of the text
/// (`[AUTHENTICATIONFAILED]`, RFC 5530). Letters only, never the text.
fn response_code(answer: &str) -> &str {
    const UNKNOWN: &str = "info: Some(\"[";
    let code = answer
        .strip_prefix("code: Some(")
        .or_else(|| answer.find(UNKNOWN).map(|at| &answer[at + UNKNOWN.len()..]))
        .unwrap_or_default();
    let end = code
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(code.len())
        .min(40);
    &code[..end]
}

/// Read the server's greeting: `* OK` (or `* PREAUTH`); `* BYE` means it
/// will not talk now.
async fn greeting<S: AsyncRead + Unpin>(stream: &mut S) -> Result<(), Refusal> {
    let line = read_line(stream).await?;
    let upper = line.to_ascii_uppercase();
    if upper.starts_with("* OK") || upper.starts_with("* PREAUTH") {
        Ok(())
    } else if upper.starts_with("* BYE") {
        Err(Refusal::ImapUnreachable)
    } else {
        Err(Refusal::ImapNotAnImapServer)
    }
}

/// Send `CAPABILITY` tagged `tag`; the capabilities, upper-cased.
async fn capabilities<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    tag: &str,
) -> Result<Vec<String>, Refusal> {
    let lines = command(stream, tag, "CAPABILITY").await?;
    Ok(lines
        .iter()
        .flat_map(|line| {
            let upper = line.to_ascii_uppercase();
            upper
                .strip_prefix("* CAPABILITY ")
                .map(|rest| {
                    rest.split_ascii_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect())
}

/// Send `command` tagged `tag` and read its answer up to the tagged line,
/// which must be `OK`. The untagged lines before it.
async fn command<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    tag: &str,
    command: &str,
) -> Result<Vec<String>, Refusal> {
    stream
        .write_all(format!("{tag} {command}\r\n").as_bytes())
        .await
        .map_err(|_| Refusal::ImapUnreachable)?;
    stream.flush().await.map_err(|_| Refusal::ImapUnreachable)?;
    let mut untagged = Vec::new();
    for _ in 0..MAX_LINES {
        let line = read_line(stream).await?;
        if let Some(status) = line.strip_prefix(tag).and_then(|r| r.strip_prefix(' ')) {
            return if status.to_ascii_uppercase().starts_with("OK") {
                Ok(untagged)
            } else {
                Err(Refusal::ImapNotAnImapServer)
            };
        }
        untagged.push(line);
    }
    Err(Refusal::ImapNotAnImapServer)
}

/// One line, without its CRLF, read a byte at a time (so nothing past it is
/// taken from the stream before a TLS upgrade).
async fn read_line<S: AsyncRead + Unpin>(stream: &mut S) -> Result<String, Refusal> {
    let mut line = Vec::with_capacity(128);
    loop {
        let byte = stream
            .read_u8()
            .await
            .map_err(|_| Refusal::ImapUnreachable)?;
        if byte == b'\n' {
            break;
        }
        if line.len() >= MAX_LINE {
            return Err(Refusal::ImapNotAnImapServer);
        }
        line.push(byte);
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(String::from_utf8_lossy(&line).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_response_code_is_kept() {
        assert_eq!(
            response_code(
                r#"code: Some(AuthenticationFailed), info: Some("ana@example.com: bad")"#
            ),
            "AuthenticationFailed"
        );
        assert_eq!(
            response_code(r#"code: None, info: Some("[AUTHENTICATIONFAILED] ana@example.com")"#),
            "AUTHENTICATIONFAILED"
        );
        assert_eq!(response_code(r#"code: None, info: Some("secret")"#), "");
    }

    #[test]
    fn literals_are_found_at_the_end_of_a_line() {
        assert_eq!(literal_size(b"* 1 FETCH (BODY[] {12}\r\n"), Some(12));
        assert_eq!(literal_size(b"* 1 FETCH (BODY[] {12+}\n"), Some(12));
        assert_eq!(literal_size(b"* LIST () \"/\" \"a{1}\"\r\n"), None);
        assert_eq!(literal_size(b"* 1 FETCH (BODY[] {}\r\n"), None);
        assert_eq!(
            literal_size(b"* 1 FETCH (BODY[] {123456789012345678901}\r\n"),
            None
        );
    }

    #[test]
    fn only_plain_ascii_is_quoted() {
        assert_eq!(quoted("Sent Items").as_deref(), Some("\"Sent Items\""));
        assert_eq!(quoted("a\"b\\c").as_deref(), Some("\"a\\\"b\\\\c\""));
        assert_eq!(quoted("line\r\nA1 DELETE INBOX"), None);
        assert_eq!(quoted("Entw\u{fc}rfe"), None);
    }

    #[test]
    fn a_busy_server_is_told_from_a_refusal() {
        let busy = ImapError::No(r#"code: None, info: Some("[UNAVAILABLE] try later")"#.into());
        assert_eq!(refused(&busy), Refusal::ImapBusy);
        let refusal =
            ImapError::No(r#"code: None, info: Some("[AUTHENTICATIONFAILED] no")"#.into());
        assert_eq!(refused(&refusal), Refusal::ImapLoginRefused);
    }

    #[test]
    fn plain_carries_the_username_and_password_once() {
        let password = SecretString::from("app-password");
        let mut plain = Plain {
            username: "ana@example.com",
            password: &password,
        };
        assert_eq!(
            async_imap::Authenticator::process(&mut plain, b""),
            b"\0ana@example.com\0app-password"
        );
    }
}
