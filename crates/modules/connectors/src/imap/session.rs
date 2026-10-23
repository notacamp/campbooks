//! One conversation with an IMAP server, signed in: Campbooks' own IMAP
//! client (what reading, write-back and sending grow on).
//!
//! Opening one: connect to the address the guard checked; TLS from the
//! first byte, or STARTTLS before anything else is said (refused when the
//! server does not offer it: no password ever goes in the clear); then
//! `CAPABILITY`, and `AUTHENTICATE PLAIN` when offered (its response in the
//! command with SASL-IR, else after the server's go-ahead), else `LOGIN`
//! (refused, nothing sent, when `LOGINDISABLED` persists). Nothing the
//! server says is kept, returned or logged, except the response code of a
//! refused sign-in.
//!
//! Every response, from the sign-in on, is framed here within the caller's
//! [`Limits`] (its size, every literal's size before a byte of it is read,
//! the number of responses, the time), then parsed (`syntax`): a server
//! that lies or never stops talking ends the command with a [`WireError`],
//! and never sizes a buffer by its own word. Before signing in, a response
//! may take at most [`SIGN_IN_RESPONSE`] bytes.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use secrecy::{ExposeSecret as _, SecretString, zeroize::Zeroizing};
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

/// The largest response read while signing in (the server is not signed in
/// to, and trusted with no memory, yet).
pub const SIGN_IN_RESPONSE: usize = 16 * 1024;

/// The most responses the sign-in reads.
const SIGN_IN_RESPONSES: usize = 64;

/// The response codes of a server that is busy rather than refusing
/// (RFC 5530): `UNAVAILABLE`, `INUSE`, `LIMIT`.
pub const BUSY_CODES: [&str; 3] = ["UNAVAILABLE", "INUSE", "LIMIT"];

/// A signed-in IMAP session.
#[derive(Debug)]
pub struct Session {
    stream: TlsStream<TcpStream>,
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

/// How much reading the socket asks for at once while in IDLE: a session
/// waits with this buffer for up to the refresh window.
const IDLE_CHUNK: usize = 4 * 1024;

/// What waiting in IDLE came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleWait {
    /// The server said something: read it with [`Session::idle_read`].
    Ready,
    /// Nothing came by the time given.
    TimedOut,
    /// The caller asked to stop.
    Stopped,
}

/// What a response heard in IDLE says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// The mailbox changed: `EXISTS`, `EXPUNGE`, `FETCH` or `VANISHED`.
    Changed,
    /// Anything else (`* OK Still here`).
    Other,
}

/// Whether `untagged` says the selected folder changed.
pub fn is_change(untagged: &Untagged<'_>) -> bool {
    matches!(
        untagged,
        Untagged::Exists(_) | Untagged::Expunge(_) | Untagged::Fetch(_) | Untagged::Vanished(_)
    )
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
        let mut session = Self {
            stream,
            pending: Vec::new(),
            start: 0,
            tags: 0,
            received: 0,
            read_timeout: imap.timeout(),
            broken: false,
        };
        session.sign_in(&capabilities, username, password).await?;
        session.received = 0;
        Ok(session)
    }

    /// Sign in: `AUTHENTICATE PLAIN` when offered, else `LOGIN` (none when
    /// `LOGINDISABLED`). A busy server is told from a refusal.
    async fn sign_in(
        &mut self,
        capabilities: &[String],
        username: &str,
        password: &SecretString,
    ) -> Result<(), Refusal> {
        let has = |name: &str| capabilities.iter().any(|c| c == name);
        let tag = self.next_tag();
        let steps = if has("AUTH=PLAIN") {
            let response = plain(username, password);
            if has("SASL-IR") {
                vec![line(&[
                    tag.as_str(),
                    " AUTHENTICATE PLAIN ",
                    response.as_str(),
                ])]
            } else {
                vec![
                    line(&[tag.as_str(), " AUTHENTICATE PLAIN"]),
                    line(&[response.as_str()]),
                ]
            }
        } else if has("LOGINDISABLED") {
            return Err(Refusal::ImapLoginRefused);
        } else {
            login(&tag, username, password)
        };
        let deadline = Instant::now() + self.read_timeout;
        match self.steps(&tag, &steps, deadline).await {
            Ok(done) if done.is_ok() => Ok(()),
            Ok(done) => Err(sign_in_refusal(&done)),
            Err(err) => {
                self.broken = true;
                Err(match err {
                    WireError::TimedOut | WireError::Closed => Refusal::ImapUnreachable,
                    WireError::TooLarge | WireError::TooMany | WireError::Malformed => {
                        Refusal::ImapNotAnImapServer
                    }
                })
            }
        }
    }

    /// Send the sign-in's `steps` under `tag`: the first, then each next
    /// one once the server asks for more (`+`). How the server ended it,
    /// every answer within [`SIGN_IN_RESPONSE`].
    async fn steps(
        &mut self,
        tag: &str,
        steps: &[Zeroizing<Vec<u8>>],
        deadline: Instant,
    ) -> Result<Done, WireError> {
        let mut count = 0;
        for (n, step) in steps.iter().enumerate() {
            self.write(step, deadline).await?;
            let last = n + 1 == steps.len();
            loop {
                let response = self.response(SIGN_IN_RESPONSE, deadline).await?;
                count += 1;
                if count > SIGN_IN_RESPONSES {
                    return Err(WireError::TooMany);
                }
                match syntax::parse(&response).map_err(|_| WireError::Malformed)? {
                    Response::Continue if !last => break,
                    Response::Tagged {
                        tag: done,
                        status,
                        code,
                    } if done == tag.as_bytes() => {
                        return Ok(Done { status, code });
                    }
                    Response::Continue | Response::Tagged { .. } => {
                        return Err(WireError::Malformed);
                    }
                    Response::Untagged(Untagged::Status {
                        status: Status::Bye,
                        ..
                    }) => return Err(WireError::Closed),
                    Response::Untagged(_) => {}
                }
            }
        }
        Err(WireError::Malformed)
    }

    /// The next command's tag.
    fn next_tag(&mut self) -> String {
        self.tags = self.tags.wrapping_add(1);
        format!("w{}", self.tags)
    }

    /// Write `bytes` and flush, by `deadline`.
    async fn write(&mut self, bytes: &[u8], deadline: Instant) -> Result<(), WireError> {
        let stream = &mut self.stream;
        let written = timeout_at(deadline, async {
            stream.write_all(bytes).await?;
            stream.flush().await?;
            Ok::<(), std::io::Error>(())
        })
        .await;
        match written {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(WireError::Closed),
            Err(_) => Err(WireError::TimedOut),
        }
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
        let tag = self.next_tag();
        let deadline = Instant::now() + limits.time;
        let line = format!("{tag} {command}\r\n");
        self.write(line.as_bytes(), deadline).await?;
        self.read_tagged(&tag, limits, deadline, &mut each).await
    }

    /// Read the answer to the command tagged `tag` up to its tagged end,
    /// handing each untagged response to `each`, within `limits` and by
    /// `deadline`.
    async fn read_tagged<F>(
        &mut self,
        tag: &str,
        limits: Limits,
        deadline: Instant,
        each: &mut F,
    ) -> Result<Done, WireError>
    where
        F: FnMut(Untagged<'_>),
    {
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

    /// Read more of the socket into the pending bytes, waiting at most the
    /// read timeout.
    async fn fill(&mut self, deadline: Instant) -> Result<(), WireError> {
        let until = deadline.min(Instant::now() + self.read_timeout);
        self.read_some(until, CHUNK).await
    }

    /// One read of at most `size` bytes of the socket into the pending
    /// bytes, waiting until `until`. Cancel-safe: nothing is taken from the
    /// connection unless the read completes.
    async fn read_some(&mut self, until: Instant, size: usize) -> Result<(), WireError> {
        if self.start > 0 && self.start == self.pending.len() {
            self.pending.clear();
            self.start = 0;
        } else if self.start > CHUNK {
            self.pending.drain(..self.start);
            self.start = 0;
        }
        let mut chunk = vec![0_u8; size];
        let read = timeout_at(until, self.stream.read(&mut chunk))
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

    /// Start IDLE (RFC 2177) on the selected folder and wait for the
    /// server's go-ahead (`+`), within `limits`. The tag that `DONE` ends
    /// ([`idle_done`](Self::idle_done)); `None` when the server refused the
    /// command (the session is still in step).
    pub async fn idle(&mut self, limits: Limits) -> Result<Option<String>, WireError> {
        if self.broken {
            return Err(WireError::Closed);
        }
        let started = self.start_idle(limits).await;
        self.broken = started.is_err();
        started
    }

    /// [`idle`](Self::idle), on a session in step.
    async fn start_idle(&mut self, limits: Limits) -> Result<Option<String>, WireError> {
        let tag = self.next_tag();
        let deadline = Instant::now() + limits.time;
        self.write(format!("{tag} IDLE\r\n").as_bytes(), deadline)
            .await?;
        let mut total = 0_usize;
        let mut count = 0_usize;
        loop {
            let response = self.response(limits.response, deadline).await?;
            total = total.saturating_add(response.len());
            if total > limits.total {
                return Err(WireError::TooMany);
            }
            match syntax::parse(&response).map_err(|_| WireError::Malformed)? {
                Response::Continue => return Ok(Some(tag)),
                Response::Tagged { tag: done, .. } if done == tag.as_bytes() => return Ok(None),
                Response::Tagged { .. } => return Err(WireError::Malformed),
                Response::Untagged(Untagged::Status {
                    status: Status::Bye,
                    ..
                }) => return Err(WireError::Closed),
                Response::Untagged(_) => {
                    count += 1;
                    if count > limits.responses {
                        return Err(WireError::TooMany);
                    }
                }
            }
        }
    }

    /// In IDLE: wait until the server says something, `until` comes, or
    /// `stop` resolves, whichever is first. No read timeout applies but
    /// `until`; nothing is taken from the connection unless a read
    /// completes, so a stop or a time-out leaves the session in step.
    pub async fn idle_wait(
        &mut self,
        until: Instant,
        stop: impl Future<Output = ()>,
    ) -> Result<IdleWait, WireError> {
        if self.broken {
            return Err(WireError::Closed);
        }
        if self.start < self.pending.len() {
            return Ok(IdleWait::Ready);
        }
        let read = tokio::select! {
            biased;
            () = stop => None,
            read = self.read_some(until, IDLE_CHUNK) => Some(read),
        };
        match read {
            None => Ok(IdleWait::Stopped),
            Some(Ok(())) => Ok(IdleWait::Ready),
            Some(Err(WireError::TimedOut)) => Ok(IdleWait::TimedOut),
            Some(Err(err)) => {
                self.broken = true;
                Err(err)
            }
        }
    }

    /// In IDLE, once [`idle_wait`](Self::idle_wait) found something: one
    /// whole response, at most `max` bytes, read within the read timeout.
    /// What it says, and its size. A goodbye or a tagged answer (the server
    /// ended IDLE on its own) ends the session.
    pub async fn idle_read(&mut self, max: usize) -> Result<(Heard, usize), WireError> {
        if self.broken {
            return Err(WireError::Closed);
        }
        let heard = self.read_heard(max).await;
        self.broken = heard.is_err();
        heard
    }

    /// [`idle_read`](Self::idle_read), on a session in step.
    async fn read_heard(&mut self, max: usize) -> Result<(Heard, usize), WireError> {
        let deadline = Instant::now() + self.read_timeout;
        let response = self.response(max, deadline).await?;
        let heard = match syntax::parse(&response).map_err(|_| WireError::Malformed)? {
            Response::Untagged(Untagged::Status {
                status: Status::Bye,
                ..
            })
            | Response::Tagged { .. } => return Err(WireError::Closed),
            Response::Continue => return Err(WireError::Malformed),
            Response::Untagged(untagged) => {
                if is_change(&untagged) {
                    Heard::Changed
                } else {
                    Heard::Other
                }
            }
        };
        Ok((heard, response.len()))
    }

    /// End IDLE (`DONE`) and read the IDLE command's answer up to its tag,
    /// handing each untagged response to `each`, within `limits`.
    pub async fn idle_done<F>(
        &mut self,
        tag: &str,
        limits: Limits,
        mut each: F,
    ) -> Result<Done, WireError>
    where
        F: FnMut(Untagged<'_>),
    {
        if self.broken {
            return Err(WireError::Closed);
        }
        let deadline = Instant::now() + limits.time;
        let done = match self.write(b"DONE\r\n", deadline).await {
            Ok(()) => self.read_tagged(tag, limits, deadline, &mut each).await,
            Err(err) => Err(err),
        };
        self.broken = done.is_err();
        done
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
        let limits = Limits {
            response: SIGN_IN_RESPONSE,
            responses: SIGN_IN_RESPONSES,
            total: 256 * 1024,
            time: self.read_timeout,
        };
        let done = self
            .command(r#"LIST "" INBOX"#, limits, |_| {})
            .await
            .map_err(|err| match err {
                WireError::TimedOut | WireError::Closed => Refusal::ImapUnreachable,
                WireError::TooLarge | WireError::TooMany | WireError::Malformed => {
                    Refusal::ImapNotAnImapServer
                }
            })?;
        if done.is_ok() {
            Ok(())
        } else {
            Err(sign_in_refusal(&done))
        }
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

/// `AUTHENTICATE PLAIN`'s response (RFC 4616), base64: no authorization
/// identity, the username and the password. Wiped when dropped.
fn plain(username: &str, password: &SecretString) -> Zeroizing<String> {
    let secret = password.expose_secret();
    let mut raw = Zeroizing::new(Vec::with_capacity(username.len() + secret.len() + 2));
    raw.push(0);
    raw.extend_from_slice(username.as_bytes());
    raw.push(0);
    raw.extend_from_slice(secret.as_bytes());
    Zeroizing::new(STANDARD.encode(raw.as_slice()))
}

/// `LOGIN`'s steps: one line of quoted strings, or (for a username or
/// password a quoted string cannot carry) literals, each sent once the
/// server asks for it. Wiped when dropped.
fn login(tag: &str, username: &str, password: &SecretString) -> Vec<Zeroizing<Vec<u8>>> {
    let secret = password.expose_secret();
    let quoted_username = quoted(username).map(Zeroizing::new);
    let quoted_password = quoted(secret).map(Zeroizing::new);
    if let (Some(user), Some(pass)) = (quoted_username, quoted_password) {
        return vec![line(&[tag, " LOGIN ", user.as_str(), " ", pass.as_str()])];
    }
    let first = format!(" LOGIN {{{}}}", username.len());
    let second = format!(" {{{}}}", secret.len());
    vec![
        line(&[tag, first.as_str()]),
        line(&[username, second.as_str()]),
        line(&[secret]),
    ]
}

/// `parts` and a line ending, wiped when dropped.
fn line(parts: &[&str]) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::new());
    for part in parts {
        out.extend_from_slice(part.as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out
}

/// The response codes a log may name (RFC 3501 §7.1 and RFC 5530): a
/// server's own atom (which could be anything it likes) is logged as
/// `other`.
const LOGGED_CODES: [&str; 27] = [
    "ALERT",
    "ALREADYEXISTS",
    "AUTHENTICATIONFAILED",
    "AUTHORIZATIONFAILED",
    "BADCHARSET",
    "CANNOT",
    "CAPABILITY",
    "CLIENTBUG",
    "CONTACTADMIN",
    "CORRUPTION",
    "EXPIRED",
    "EXPUNGEISSUED",
    "INUSE",
    "LIMIT",
    "NONEXISTENT",
    "NOPERM",
    "OVERQUOTA",
    "PARSE",
    "PERMANENTFLAGS",
    "PRIVACYREQUIRED",
    "READ-ONLY",
    "READ-WRITE",
    "SERVERBUG",
    "TRYCREATE",
    "UIDNEXT",
    "UIDVALIDITY",
    "UNAVAILABLE",
];

/// `code` as a log may carry it: one of [`LOGGED_CODES`], `other`, or empty
/// when there is none.
fn logged_code(code: Option<&Code>) -> &'static str {
    match code {
        None => "",
        Some(code) => LOGGED_CODES
            .iter()
            .find(|known| **known == code.name)
            .copied()
            .unwrap_or("other"),
    }
}

/// What a sign-in the server ended without `OK` means: busy (RFC 5530's
/// `UNAVAILABLE`, `INUSE`, `LIMIT`), or a refusal. Logs the response code
/// only when it is a known one, never the server's text.
fn sign_in_refusal(done: &Done) -> Refusal {
    let code = logged_code(done.code.as_ref());
    if done.is_busy() {
        tracing::info!(code, "the IMAP server is busy");
        Refusal::ImapBusy
    } else {
        tracing::info!(code, "the IMAP server refused the sign-in");
        Refusal::ImapLoginRefused
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
#[path = "session_tests.rs"]
mod tests;
