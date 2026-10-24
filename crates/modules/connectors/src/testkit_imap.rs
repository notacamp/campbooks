//! Fake mail servers for the IMAP adapter's tests: an IMAP server and an
//! SMTP submission server, in process on 127.0.0.1, scripted (the password
//! they accept, what they offer), over TLS with a test certificate the
//! tests trust (`fixtures/imap/`: a test root and a certificate for
//! `imap.example.com`, `smtp.example.com` and `mail.example.com`, valid
//! until 2126, made with `openssl`; no real host has them). A fake resolver
//! names the fakes; fake DNS and a fake web serve discovery. The IMAP fake
//! holds a mailbox (`testkit_imap_mailbox`) the tests change as another mail
//! app would. The SMTP fake takes mail as a submission server does
//! (`MAIL`, `RCPT`, `DATA`), keeps what it received, and can be busy, refuse,
//! announce a `SIZE`, hang up after the message or never stop talking.
//!
//! No test talks to a real mail server, resolver or web server.

use std::{
    collections::HashMap,
    fmt::Write as _,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::net::guard::{Resolve, ResolveFuture};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::{
    io::{
        AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _,
        BufReader,
    },
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        crypto::aws_lc_rs,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    },
};
use url::Url;

use crate::{
    config::ImapConfig,
    imap::{
        Imap, Security, Server, Servers, Trust,
        net::{Dns, NetFuture, SrvTarget, Web},
    },
    testkit::Kit,
    testkit_imap_mailbox::{self, Effects, Offers, Store},
};

/// The test root the fakes' certificate is issued by.
const ROOT: &[u8] = include_bytes!("../fixtures/imap/ca.der");
/// The fakes' certificate.
const CERTIFICATE: &[u8] = include_bytes!("../fixtures/imap/server.der");
/// Its key (PKCS #8): a test key, for these fakes only.
const KEY: &[u8] = include_bytes!("../fixtures/imap/server-key.der");

/// The fakes' names (on the certificate).
pub const IMAP_HOST: &str = "imap.example.com";
pub const SMTP_HOST: &str = "smtp.example.com";

/// How a fake server behaves.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one independent behaviour a test turns on"
)]
pub struct Script {
    /// The username it accepts.
    pub username: String,
    /// The password it accepts.
    pub password: String,
    /// It offers STARTTLS on its plain port.
    pub starttls: bool,
    /// IMAP: it offers `AUTH=PLAIN`; SMTP: it offers `AUTH PLAIN LOGIN`.
    pub auth: bool,
    /// IMAP: it says `LOGINDISABLED`.
    pub login_disabled: bool,
    /// It greets like a web server, not a mail server.
    pub not_mail: bool,
    /// IMAP: it refuses sign-ins as busy (`[UNAVAILABLE]`).
    pub busy: bool,
    /// IMAP: it offers SASL-IR (`AUTHENTICATE PLAIN` with its response).
    pub sasl_ir: bool,
    /// IMAP: it answers a sign-in with a literal larger than any memory.
    pub hostile_sign_in: bool,
    /// IMAP, signed in: what it offers, and how it misbehaves.
    pub offers: Offers,
    /// SMTP: how it takes mail.
    pub smtp: SmtpScript,
}

/// How the SMTP fake takes mail.
#[derive(Debug, Clone, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one independent behaviour a test turns on"
)]
pub struct SmtpScript {
    /// The `SIZE` its EHLO announces.
    pub size: Option<u64>,
    /// `RCPT TO` commands answered 451 (busy) before it takes any, across
    /// sessions.
    pub busy_rcpts: usize,
    /// Messages answered 451 (busy) after their final dot before it takes
    /// one, across sessions.
    pub busy_messages: usize,
    /// Every message is answered 554 after its final dot.
    pub refuse_messages: bool,
    /// `AUTH` is answered 454 (busy).
    pub busy_auth: bool,
    /// `RCPT TO` this address is answered 550.
    pub refuse_rcpt: Option<String>,
    /// The connection is closed once a message arrived, before any answer.
    pub hang_up_after_message: bool,
    /// EHLO is answered with bytes and no line ending, without end.
    pub endless_ehlo: bool,
}

impl Script {
    /// A server that takes `username` and `password`.
    pub fn accepting(username: &str, password: &str) -> Self {
        Self {
            username: username.to_owned(),
            password: password.to_owned(),
            starttls: true,
            auth: true,
            login_disabled: false,
            not_mail: false,
            busy: false,
            sasl_ir: false,
            hostile_sign_in: false,
            offers: Offers {
                special_use: true,
                condstore: true,
                ..Offers::default()
            },
            smtp: SmtpScript::default(),
        }
    }
}

/// What a fake saw.
#[derive(Debug, Default, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one independent thing a test checks it saw"
)]
pub struct Seen {
    /// Connections accepted.
    pub connections: usize,
    /// Sign-ins tried (LOGIN, AUTHENTICATE, AUTH).
    pub sign_ins: usize,
    /// A password arrived before TLS.
    pub password_in_clear: bool,
    /// The inbox was listed.
    pub listed: bool,
    /// SMTP: a `MAIL FROM` arrived (nothing may be sent).
    pub mail_from: bool,
    /// IMAP: the folders opened, in order, and whether any read-write.
    pub opened: Vec<String>,
    pub selected: bool,
    /// IMAP: bodies sent, those that marked their message read, and
    /// changes asked for (`STORE`, `EXPUNGE`, …).
    pub bodies: usize,
    pub marked_read: usize,
    pub changes: usize,
    /// IMAP: changes reported in IDLE.
    pub reported: usize,
    /// IMAP: `IDLE` commands, `DONE`s, `NOOP`s and `LOGOUT`s received.
    pub idles: usize,
    pub dones: usize,
    pub noops: usize,
    pub logouts: usize,
    /// IMAP, signed in: each command's verb (`UID MOVE`, `EXPUNGE`), and
    /// its whole line past the tag, in order.
    pub commands: Vec<String>,
    pub lines: Vec<String>,
    /// IMAP: the messages appended (`APPEND`), by folder.
    pub appended: Vec<(String, Vec<u8>)>,
    /// SMTP: each `MAIL FROM` and `RCPT TO` address, `DATA` commands, every
    /// message received (whatever the answer), and those taken (250).
    pub mail_froms: Vec<String>,
    pub rcpts: Vec<String>,
    pub datas: usize,
    pub received: Vec<Vec<u8>>,
    pub delivered: Vec<Vec<u8>>,
    /// SMTP: whether each `MAIL FROM` came after STARTTLS or over TLS.
    pub mail_in_clear: bool,
}

impl Seen {
    /// How many commands with verb `verb` it received.
    pub fn count(&self, verb: &str) -> usize {
        self.commands.iter().filter(|c| *c == verb).count()
    }
}

#[derive(Debug)]
struct Shared {
    script: Mutex<Script>,
    seen: Mutex<Seen>,
    store: Mutex<Store>,
    /// Bumped to make every session in IDLE say goodbye and hang up.
    hangups: AtomicUsize,
}

impl Shared {
    fn script(&self) -> Script {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn see(&self, f: impl FnOnce(&mut Seen)) {
        f(&mut self.seen.lock().unwrap_or_else(PoisonError::into_inner));
    }
}

/// A fake mail server (IMAP or SMTP) on 127.0.0.1.
#[derive(Debug, Clone)]
pub struct FakeServer {
    /// Where it listens.
    pub address: SocketAddr,
    /// How its connections are encrypted.
    pub security: Security,
    shared: Arc<Shared>,
}

impl FakeServer {
    /// Start a fake IMAP server.
    pub async fn imap(security: Security, script: Script) -> Self {
        Self::start(security, script, Protocol::Imap).await
    }

    /// Start a fake SMTP submission server.
    pub async fn smtp(security: Security, script: Script) -> Self {
        Self::start(security, script, Protocol::Smtp).await
    }

    async fn start(security: Security, script: Script, protocol: Protocol) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shared = Arc::new(Shared {
            script: Mutex::new(script),
            seen: Mutex::new(Seen::default()),
            store: Mutex::new(Store::default()),
            hangups: AtomicUsize::new(0),
        });
        let serving = shared.clone();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                serving.see(|seen| seen.connections += 1);
                let shared = serving.clone();
                tokio::spawn(async move {
                    match protocol {
                        Protocol::Imap => serve_imap(tcp, security, &shared).await,
                        Protocol::Smtp => serve_smtp(tcp, security, &shared).await,
                    }
                });
            }
        });
        Self {
            address,
            security,
            shared,
        }
    }

    /// The server as settings name it: `host` (resolved to the fake by
    /// [`FakeResolver`]) on its port.
    pub fn at(&self, host: &str) -> Server {
        Server {
            host: host.to_owned(),
            port: self.address.port(),
            security: self.security,
        }
    }

    /// Accept `password` from now on.
    pub fn set_password(&self, password: &str) {
        password.clone_into(
            &mut self
                .shared
                .script
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .password,
        );
    }

    /// What it saw so far.
    pub fn seen(&self) -> Seen {
        self.shared
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Change how it behaves from its next session on.
    pub fn script(&self, change: impl FnOnce(&mut Script)) {
        change(
            &mut self
                .shared
                .script
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// Change its mailbox, as another mail app would; what `change` returns.
    pub fn mailbox<T>(&self, change: impl FnOnce(&mut Store) -> T) -> T {
        change(
            &mut self
                .shared
                .store
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Put `raw` in `folder`, received `at`; its UID.
    pub fn deliver(&self, folder: &str, raw: &[u8], at: DateTime<Utc>) -> u32 {
        self.mailbox(|store| store.deliver(folder, raw, at))
    }

    /// Whether message `uid` of `folder` is read at the server.
    pub fn is_read(&self, folder: &str, uid: u32) -> bool {
        self.mailbox(|store| store.mail(folder, uid).seen)
    }

    /// Make every session in IDLE say goodbye and hang up (a restart).
    pub fn hang_up(&self) {
        self.shared.hangups.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy)]
enum Protocol {
    Imap,
    Smtp,
}

/// The fakes' TLS side.
fn acceptor() -> TlsAcceptor {
    let config = ServerConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(CERTIFICATE.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec())),
        )
        .unwrap();
    TlsAcceptor::from(Arc::new(config))
}

/// What a plain phase ended with.
enum Next {
    /// The client asked for STARTTLS.
    Starttls,
    /// The conversation is over.
    Done,
}

async fn serve_imap(tcp: TcpStream, security: Security, shared: &Shared) {
    match security {
        Security::Tls => {
            let Ok(tls) = acceptor().accept(tcp).await else {
                return;
            };
            imap_session(&mut BufReader::new(tls), true, true, shared).await;
        }
        Security::Starttls => {
            let mut plain = BufReader::new(tcp);
            if let Next::Starttls = imap_session(&mut plain, false, true, shared).await {
                let Ok(tls) = acceptor().accept(plain.into_inner()).await else {
                    return;
                };
                imap_session(&mut BufReader::new(tls), true, false, shared).await;
            }
        }
    }
}

async fn write<S: AsyncWrite + Unpin>(stream: &mut S, text: &str) -> bool {
    stream.write_all(text.as_bytes()).await.is_ok() && stream.flush().await.is_ok()
}

async fn read<S: AsyncRead + Unpin>(stream: &mut BufReader<S>) -> Option<String> {
    let mut line = String::new();
    match stream.read_line(&mut line).await {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim_end_matches(['\r', '\n']).to_owned()),
    }
}

/// An IMAP argument: a quoted string (with `\\` and `\"`) or an atom; the
/// rest after it.
fn argument(text: &str) -> (String, &str) {
    let text = text.trim_start();
    if let Some(quoted) = text.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = quoted.char_indices();
        while let Some((at, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((_, escaped)) = chars.next() {
                        out.push(escaped);
                    }
                }
                '"' => return (out, &quoted[at + 1..]),
                other => out.push(other),
            }
        }
        (out, "")
    } else {
        let end = text.find(' ').unwrap_or(text.len());
        (text[..end].to_owned(), &text[end..])
    }
}

/// `AUTHENTICATE PLAIN`'s response: (username, password).
fn plain(response: &str) -> Option<(String, String)> {
    let bytes = STANDARD.decode(response.trim()).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let mut parts = text.split('\0');
    let _authorization = parts.next()?;
    Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
}

#[allow(
    clippy::too_many_lines,
    reason = "one session of the fake, command by command"
)]
async fn imap_session<S: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut BufReader<S>,
    encrypted: bool,
    greet: bool,
    shared: &Shared,
) -> Next {
    let script = shared.script();
    let mut selected = None;
    if greet {
        let greeting = if script.not_mail {
            "HTTP/1.1 400 Bad Request\r\n"
        } else {
            "* OK fake IMAP ready\r\n"
        };
        if !write(conn.get_mut(), greeting).await || script.not_mail {
            return Next::Done;
        }
    }
    while let Some(line) = read(conn).await {
        let (tag, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let (verb, args) = rest.split_once(' ').unwrap_or((rest, ""));
        let tag = tag.to_owned();
        let reply = match verb.to_ascii_uppercase().as_str() {
            "CAPABILITY" => {
                let mut caps = vec!["IMAP4rev1"];
                if !encrypted && script.starttls {
                    caps.push("STARTTLS");
                }
                if encrypted && script.auth {
                    caps.push("AUTH=PLAIN");
                }
                if encrypted && script.sasl_ir {
                    caps.push("SASL-IR");
                }
                if script.login_disabled || !encrypted {
                    caps.push("LOGINDISABLED");
                }
                let limit = script
                    .offers
                    .append_limit
                    .map(|limit| format!("APPENDLIMIT={limit}"));
                if let (true, Some(limit)) = (encrypted, limit.as_deref()) {
                    caps.push(limit);
                }
                if encrypted {
                    let offers = script.offers;
                    caps.push("ENABLE");
                    for (offered, name) in [
                        (offers.special_use, "SPECIAL-USE"),
                        (offers.condstore, "CONDSTORE"),
                        (offers.qresync, "QRESYNC"),
                        (offers.objectid, "OBJECTID"),
                        (offers.idle, "IDLE"),
                        (offers.moves, "MOVE"),
                        (offers.uidplus, "UIDPLUS"),
                        (offers.unselect, "UNSELECT"),
                    ] {
                        if offered {
                            caps.push(name);
                        }
                    }
                }
                format!("* CAPABILITY {}\r\n{tag} OK done\r\n", caps.join(" "))
            }
            "STARTTLS" if !encrypted && script.starttls => {
                write(conn.get_mut(), &format!("{tag} OK begin TLS\r\n")).await;
                return Next::Starttls;
            }
            "LOGIN" | "AUTHENTICATE" if script.hostile_sign_in => {
                shared.see(|seen| seen.sign_ins += 1);
                // Five hundred megabytes announced, a few bytes sent.
                "* LIST () \"/\" {500000000}\r\nINBOX".to_owned()
            }
            "LOGIN" => {
                shared.see(|seen| {
                    seen.sign_ins += 1;
                    seen.password_in_clear |= !encrypted;
                });
                let credentials = if args.trim_end().ends_with('}') {
                    login_literals(conn, args).await
                } else {
                    let (username, rest) = argument(args);
                    let (password, _) = argument(rest);
                    Some((username, password))
                };
                match credentials {
                    Some((username, password)) => login(&script, &tag, &username, &password),
                    None => format!("{tag} BAD not LOGIN\r\n"),
                }
            }
            "AUTHENTICATE" => {
                shared.see(|seen| {
                    seen.sign_ins += 1;
                    seen.password_in_clear |= !encrypted;
                });
                let initial = args.split_once(' ').map(|(_, response)| response.trim());
                let response =
                    if let Some(response) = initial.filter(|r| !r.is_empty() && script.sasl_ir) {
                        response.to_owned()
                    } else {
                        write(conn.get_mut(), "+ \r\n").await;
                        read(conn).await.unwrap_or_default()
                    };
                match plain(&response) {
                    Some((username, password)) => login(&script, &tag, &username, &password),
                    None => format!("{tag} BAD not PLAIN\r\n"),
                }
            }
            "LIST" if encrypted => {
                shared.see(|seen| seen.listed = true);
                let answer = signed_in(shared, &script, &tag, verb, args, &mut selected);
                if !write_bytes(conn.get_mut(), &answer).await {
                    break;
                }
                continue;
            }
            "LOGOUT" => {
                shared.see(|seen| seen.logouts += 1);
                write(
                    conn.get_mut(),
                    &format!("* BYE see you\r\n{tag} OK done\r\n"),
                )
                .await;
                return Next::Done;
            }
            "IDLE" if encrypted && script.offers.idle => {
                shared.see(|seen| seen.idles += 1);
                if !write(conn.get_mut(), "+ idling\r\n").await {
                    break;
                }
                let folder = selected.clone().unwrap_or_else(|| "INBOX".to_owned());
                match idle(conn, shared, script.offers, &folder).await {
                    Idled::Done => {
                        shared.see(|seen| seen.dones += 1);
                        format!("{tag} OK IDLE terminated\r\n")
                    }
                    Idled::Gone => break,
                }
            }
            "NOOP" if encrypted => {
                shared.see(|seen| seen.noops += 1);
                format!("{tag} OK done\r\n")
            }
            "APPEND" if encrypted => match append(conn, shared, &script, &tag, args).await {
                Some(reply) => reply,
                None => break,
            },
            _ if encrypted => {
                let answer = signed_in(shared, &script, &tag, verb, args, &mut selected);
                if !write_bytes(conn.get_mut(), &answer).await {
                    break;
                }
                continue;
            }
            _ => format!("{tag} BAD unknown command\r\n"),
        };
        if !write(conn.get_mut(), &reply).await {
            break;
        }
    }
    Next::Done
}

/// How an IDLE ended.
enum Idled {
    /// The client said `DONE`.
    Done,
    /// The connection closed, or the fake hung up.
    Gone,
}

/// What woke the fake in IDLE.
enum Woke {
    /// The client sent a line.
    Line,
    /// The client closed the connection.
    Closed,
    /// Time to look at the mailbox again.
    Tick,
}

/// IDLE on `folder` until the client says `DONE`: the folder's changes are
/// reported as they happen (`EXISTS` when it grew, an `EXPUNGE` per
/// message gone, `FETCH` when only flags moved), or, with `idle_flood`,
/// `EXISTS` without end.
async fn idle<S: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut BufReader<S>,
    shared: &Shared,
    offers: Offers,
    folder: &str,
) -> Idled {
    let hangups = shared.hangups.load(Ordering::SeqCst);
    let state = || {
        shared
            .store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .idle_state(folder)
    };
    let mut last = state();
    let flood = "* 1 EXISTS\r\n".repeat(500);
    loop {
        if offers.idle_flood && !write(conn.get_mut(), &flood).await {
            return Idled::Gone;
        }
        let woke = tokio::select! {
            ready = conn.fill_buf() => match ready {
                Ok(buf) if !buf.is_empty() => Woke::Line,
                _ => Woke::Closed,
            },
            () = tokio::time::sleep(Duration::from_millis(10)) => Woke::Tick,
        };
        match woke {
            Woke::Closed => return Idled::Gone,
            Woke::Line => {
                let line = read(conn).await;
                return if line.is_some_and(|l| l.eq_ignore_ascii_case("DONE")) {
                    Idled::Done
                } else {
                    Idled::Gone
                };
            }
            Woke::Tick => {
                if shared.hangups.load(Ordering::SeqCst) != hangups {
                    write(conn.get_mut(), "* BYE going away\r\n").await;
                    return Idled::Gone;
                }
                let now = state();
                let mut report = String::new();
                if let (Some((before, modseq_before)), Some((after, modseq_after))) = (last, now) {
                    if after > before {
                        let _ = write!(report, "* {after} EXISTS\r\n");
                    } else if after < before {
                        for _ in after..before {
                            report.push_str("* 1 EXPUNGE\r\n");
                        }
                    } else if modseq_after != modseq_before {
                        report.push_str("* 1 FETCH (FLAGS (\\Seen))\r\n");
                    }
                }
                if !report.is_empty() {
                    if !write(conn.get_mut(), &report).await {
                        return Idled::Gone;
                    }
                    shared.see(|seen| seen.reported += 1);
                }
                last = now;
            }
        }
    }
}

/// A command past signing in, answered from the mailbox.
fn signed_in(
    shared: &Shared,
    script: &Script,
    tag: &str,
    verb: &str,
    args: &str,
    selected: &mut Option<String>,
) -> Vec<u8> {
    let mut effects = Effects::default();
    let answer = testkit_imap_mailbox::answer(
        &mut shared.store.lock().unwrap_or_else(PoisonError::into_inner),
        script.offers,
        tag,
        &verb.to_ascii_uppercase(),
        args,
        selected,
        &mut effects,
    );
    let named = if verb.eq_ignore_ascii_case("UID") {
        format!(
            "UID {}",
            args.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase()
        )
    } else {
        verb.to_ascii_uppercase()
    };
    shared.see(|seen| {
        seen.commands.push(named);
        seen.lines.push(format!("{verb} {args}"));
        if let Some((folder, write)) = effects.opened {
            seen.opened.push(folder);
            seen.selected |= write;
        }
        seen.bodies += effects.bodies;
        seen.marked_read += effects.marked_read;
        seen.changes += effects.changes;
    });
    answer
}

/// `APPEND <folder> [(<flags>)] {n}` (its arguments `args`): the
/// go-ahead, the literal, then the answer, as the script has it. `None` when
/// the connection is to end.
async fn append<S: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut BufReader<S>,
    shared: &Shared,
    script: &Script,
    tag: &str,
    args: &str,
) -> Option<String> {
    let offers = script.offers;
    let (folder, rest) = argument(args);
    let size: usize = rest
        .trim()
        .rsplit_once('{')?
        .1
        .trim_end_matches('}')
        .trim_end_matches('+')
        .parse()
        .ok()?;
    let read_flag = rest.to_ascii_uppercase().contains("\\SEEN");
    shared.see(|seen| {
        seen.commands.push("APPEND".to_owned());
        seen.lines.push(format!("APPEND {args}"));
    });
    if offers.append_busy {
        return Some(format!("{tag} NO [UNAVAILABLE] try again later\r\n"));
    }
    if offers
        .append_limit
        .is_some_and(|limit| u64::try_from(size).unwrap_or(u64::MAX) > limit)
    {
        return Some(format!("{tag} NO [TOOBIG] too big\r\n"));
    }
    if !write(conn.get_mut(), "+ go on\r\n").await {
        return None;
    }
    let mut raw = vec![0; size];
    conn.read_exact(&mut raw).await.ok()?;
    read(conn).await?;
    shared.see(|seen| seen.appended.push((folder.clone(), raw.clone())));
    if offers.append_hang {
        tokio::time::sleep(Duration::from_hours(1)).await;
        return None;
    }
    if offers.append_refused {
        return Some(format!("{tag} NO [OVERQUOTA] the mailbox is full\r\n"));
    }
    let appended = shared
        .store
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .append_mail(&folder, raw, Utc::now(), read_flag);
    Some(match appended {
        None => format!("{tag} NO [TRYCREATE] no such folder\r\n"),
        Some((uidvalidity, uid)) if offers.uidplus => {
            format!("{tag} OK [APPENDUID {uidvalidity} {uid}] APPEND completed\r\n")
        }
        Some(_) => format!("{tag} OK APPEND completed\r\n"),
    })
}

async fn write_bytes<S: AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) -> bool {
    stream.write_all(bytes).await.is_ok() && stream.flush().await.is_ok()
}

/// `LOGIN {n}` (its arguments `first`): the username and password sent as
/// synchronising literals, each after the go-ahead.
async fn login_literals<S: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut BufReader<S>,
    first: &str,
) -> Option<(String, String)> {
    let size = |text: &str| -> Option<usize> {
        text.trim()
            .strip_prefix('{')?
            .strip_suffix('}')?
            .parse()
            .ok()
    };
    let username_len = size(first)?;
    write(conn.get_mut(), "+ go on\r\n").await;
    let line = read(conn).await?;
    let username = line.get(..username_len)?.to_owned();
    let password_len = size(line.get(username_len..)?)?;
    write(conn.get_mut(), "+ go on\r\n").await;
    let password = read(conn).await?;
    (password.len() == password_len).then_some((username, password))
}

fn login(script: &Script, tag: &str, username: &str, password: &str) -> String {
    if script.busy {
        return format!("{tag} NO [UNAVAILABLE] {username} cannot sign in now\r\n");
    }
    if username == script.username && password == script.password && !script.login_disabled {
        format!("{tag} OK signed in\r\n")
    } else {
        // What a server says may hold the username: Campbooks never repeats
        // it.
        format!("{tag} NO [AUTHENTICATIONFAILED] {username} may not sign in\r\n")
    }
}

async fn serve_smtp(tcp: TcpStream, security: Security, shared: &Shared) {
    match security {
        Security::Tls => {
            let Ok(tls) = acceptor().accept(tcp).await else {
                return;
            };
            smtp_session(&mut BufReader::new(tls), true, true, shared).await;
        }
        Security::Starttls => {
            let mut plain = BufReader::new(tcp);
            if let Next::Starttls = smtp_session(&mut plain, false, true, shared).await {
                let Ok(tls) = acceptor().accept(plain.into_inner()).await else {
                    return;
                };
                smtp_session(&mut BufReader::new(tls), true, false, shared).await;
            }
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one session of the fake, command by command"
)]
async fn smtp_session<S: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut BufReader<S>,
    encrypted: bool,
    greet: bool,
    shared: &Shared,
) -> Next {
    let script = shared.script();
    if greet {
        let greeting = if script.not_mail {
            "HTTP/1.1 400 Bad Request\r\n"
        } else {
            "220 smtp.example.com ESMTP fake\r\n"
        };
        if !write(conn.get_mut(), greeting).await || script.not_mail {
            return Next::Done;
        }
    }
    let mut signed_in = false;
    while let Some(line) = read(conn).await {
        let (verb, args) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let reply = match verb.to_ascii_uppercase().as_str() {
            "EHLO" | "HELO" if script.smtp.endless_ehlo => {
                // Bytes without a line ending, until the client stops reading.
                let chunk = "x".repeat(64 * 1024);
                write(conn.get_mut(), "250-").await;
                for _ in 0..64 {
                    if !write(conn.get_mut(), &chunk).await {
                        break;
                    }
                }
                return Next::Done;
            }
            "EHLO" | "HELO" => {
                let mut lines = vec!["smtp.example.com".to_owned()];
                if !encrypted && script.starttls {
                    lines.push("STARTTLS".to_owned());
                }
                if encrypted && script.auth {
                    lines.push("AUTH PLAIN LOGIN".to_owned());
                }
                lines.push("8BITMIME".to_owned());
                if let Some(size) = script.smtp.size {
                    lines.push(format!("SIZE {size}"));
                }
                let last = lines.len() - 1;
                let mut reply = String::new();
                for (i, line) in lines.iter().enumerate() {
                    reply.push_str("250");
                    reply.push(if i == last { ' ' } else { '-' });
                    reply.push_str(line);
                    reply.push_str("\r\n");
                }
                reply
            }
            "STARTTLS" if !encrypted && script.starttls => {
                write(conn.get_mut(), "220 go ahead\r\n").await;
                return Next::Starttls;
            }
            "AUTH" => {
                shared.see(|seen| {
                    seen.sign_ins += 1;
                    seen.password_in_clear |= !encrypted;
                });
                let (mechanism, response) = args.split_once(' ').unwrap_or((args, ""));
                let accepted = mechanism.eq_ignore_ascii_case("PLAIN")
                    && plain(response).is_some_and(|(username, password)| {
                        username == script.username && password == script.password
                    });
                if script.smtp.busy_auth {
                    "454 4.7.0 try again later\r\n".to_owned()
                } else if accepted {
                    signed_in = true;
                    "235 2.7.0 accepted\r\n".to_owned()
                } else {
                    "535 5.7.8 credentials refused\r\n".to_owned()
                }
            }
            "MAIL" => {
                let address = envelope_address(args);
                shared.see(|seen| {
                    seen.mail_from = true;
                    seen.mail_in_clear |= !encrypted;
                    seen.mail_froms.push(address);
                });
                if signed_in {
                    "250 ok\r\n".to_owned()
                } else {
                    "530 5.7.0 sign in first\r\n".to_owned()
                }
            }
            "RCPT" => {
                let address = envelope_address(args);
                let mut busy = false;
                shared.see(|seen| {
                    busy = seen.rcpts.len() < script.smtp.busy_rcpts;
                    seen.rcpts.push(address.clone());
                });
                if busy {
                    "451 4.3.0 try again later\r\n".to_owned()
                } else if script.smtp.refuse_rcpt.as_deref() == Some(address.as_str()) {
                    "550 5.1.1 no such mailbox\r\n".to_owned()
                } else {
                    "250 ok\r\n".to_owned()
                }
            }
            "DATA" => {
                shared.see(|seen| seen.datas += 1);
                if !write(conn.get_mut(), "354 end with a dot\r\n").await {
                    break;
                }
                let Some(message) = smtp_message(conn).await else {
                    break;
                };
                let mut busy = false;
                shared.see(|seen| {
                    busy = seen.received.len() < script.smtp.busy_messages;
                    seen.received.push(message.clone());
                });
                if script.smtp.hang_up_after_message {
                    return Next::Done;
                }
                if busy {
                    "451 4.3.0 try again later\r\n".to_owned()
                } else if script.smtp.refuse_messages {
                    "554 5.6.0 refused\r\n".to_owned()
                } else {
                    shared.see(|seen| seen.delivered.push(message));
                    "250 2.0.0 queued\r\n".to_owned()
                }
            }
            "RSET" | "NOOP" => "250 ok\r\n".to_owned(),
            "QUIT" => {
                write(conn.get_mut(), "221 bye\r\n").await;
                return Next::Done;
            }
            _ => "500 unknown command\r\n".to_owned(),
        };
        if !write(conn.get_mut(), &reply).await {
            break;
        }
    }
    Next::Done
}

/// The address in `MAIL FROM:<a> …` or `RCPT TO:<a> …`.
fn envelope_address(args: &str) -> String {
    args.split_once('<')
        .and_then(|(_, rest)| rest.split_once('>'))
        .map_or_else(String::new, |(address, _)| address.to_owned())
}

/// A message after `DATA`, up to its final dot, dot-stuffing undone.
async fn smtp_message<S: AsyncRead + Unpin>(conn: &mut BufReader<S>) -> Option<Vec<u8>> {
    let mut message = Vec::new();
    loop {
        let mut line = Vec::new();
        if conn.read_until(b'\n', &mut line).await.ok()? == 0 {
            return None;
        }
        if line == b".\r\n" || line == b".\n" {
            return Some(message);
        }
        let line = line.strip_prefix(b".").unwrap_or(&line);
        message.extend_from_slice(line);
    }
}

/// A resolver that knows the names it was told, and nothing else.
#[derive(Debug, Default)]
pub struct FakeResolver {
    names: Mutex<HashMap<String, Vec<IpAddr>>>,
}

impl FakeResolver {
    /// `host` resolves to `ips`.
    pub fn name(&self, host: &str, ips: &[&str]) {
        self.names
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                host.to_owned(),
                ips.iter().map(|ip| ip.parse().unwrap()).collect(),
            );
    }
}

impl Resolve for FakeResolver {
    fn resolve<'a>(&'a self, host: &'a str, port: u16) -> ResolveFuture<'a> {
        let found = self
            .names
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(host)
            .cloned();
        Box::pin(async move {
            found
                .map(|ips| {
                    ips.into_iter()
                        .map(|ip| SocketAddr::new(ip, port))
                        .collect()
                })
                .ok_or_else(|| std::io::Error::other("no such name"))
        })
    }
}

/// DNS records told in advance.
#[derive(Debug, Default)]
pub struct FakeDns {
    srv: Mutex<HashMap<String, Vec<SrvTarget>>>,
    mx: Mutex<HashMap<String, Vec<String>>>,
    asked: Mutex<Vec<String>>,
}

impl FakeDns {
    /// `domain`'s MX hosts.
    pub fn set_mx(&self, domain: &str, hosts: &[&str]) {
        self.mx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                domain.to_owned(),
                hosts.iter().map(|h| (*h).to_owned()).collect(),
            );
    }

    /// `name`'s SRV target.
    pub fn set_srv(&self, name: &str, host: &str, port: u16) {
        self.srv
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                name.to_owned(),
                vec![SrvTarget {
                    host: host.to_owned(),
                    port,
                    priority: 0,
                    weight: 0,
                }],
            );
    }

    /// The names asked about, in order.
    pub fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Dns for FakeDns {
    fn srv<'a>(&'a self, name: &'a str) -> NetFuture<'a, Vec<SrvTarget>> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(name.to_owned());
        let found = self
            .srv
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(name)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move { found })
    }

    fn mx<'a>(&'a self, domain: &'a str) -> NetFuture<'a, Vec<String>> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!("MX {domain}"));
        let found = self
            .mx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(domain)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move { found })
    }
}

/// Documents served at URLs told in advance; records every URL asked for.
#[derive(Debug, Default)]
pub struct FakeWeb {
    documents: Mutex<HashMap<String, Vec<u8>>>,
    asked: Mutex<Vec<String>>,
}

impl FakeWeb {
    /// Serve `body` at `url`.
    pub fn serve(&self, url: &str, body: &str) {
        self.documents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(url.to_owned(), body.as_bytes().to_vec());
    }

    /// The URLs asked for, in order.
    pub fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Web for FakeWeb {
    fn fetch<'a>(&'a self, url: &'a Url) -> NetFuture<'a, Option<Vec<u8>>> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.to_string());
        let found = self
            .documents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(url.as_str())
            .cloned();
        Box::pin(async move { found })
    }
}

/// The fakes behind one test's IMAP adapter.
#[derive(Debug, Clone)]
pub struct Fakes {
    pub resolver: Arc<FakeResolver>,
    pub dns: Arc<FakeDns>,
    pub web: Arc<FakeWeb>,
}

/// An IMAP adapter over the fakes: on, the test root trusted, and (the
/// fakes being on 127.0.0.1 on any port) the instance's own network and
/// any port allowed, as a self-hosted instance may.
pub fn adapter(fakes: &Fakes) -> Imap {
    Imap {
        config: ImapConfig {
            enabled: true,
            allow_private_hosts: true,
            allow_any_port: true,
            connect_timeout_seconds: 5,
            timeout_seconds: 10,
            ..ImapConfig::default()
        },
        resolver: fakes.resolver.clone(),
        dns: fakes.dns.clone(),
        web: fakes.web.clone(),
        trust: Trust::Roots(vec![CertificateDer::from(ROOT.to_vec())]),
        sent_keepers: Vec::new(),
    }
}

/// The module with IMAP on over fresh fakes.
pub async fn kit(db: PgPool) -> (Kit, Fakes) {
    kit_with(db, |_| {}).await
}

/// [`kit`], its IMAP settings changed by `change`.
pub async fn kit_with(db: PgPool, change: impl FnOnce(&mut ImapConfig)) -> (Kit, Fakes) {
    kit_with_adapter(db, |imap| change(&mut imap.config)).await
}

/// [`kit`], its IMAP adapter changed by `change` (its settings, the SMTP
/// hosts known to keep a copy of what they send).
pub async fn kit_with_adapter(db: PgPool, change: impl FnOnce(&mut Imap)) -> (Kit, Fakes) {
    let fakes = Fakes {
        resolver: Arc::new(FakeResolver::default()),
        dns: Arc::new(FakeDns::default()),
        web: Arc::new(FakeWeb::default()),
    };
    let mut imap = adapter(&fakes);
    change(&mut imap);
    let kit = Kit::with_imap(db, imap).await;
    (kit, fakes)
}

/// An IMAP server (TLS) and an SMTP server (STARTTLS) taking `username`
/// and `password`, named `imap.example.com` and `smtp.example.com` by the
/// fake resolver: the servers to connect with.
pub async fn mailbox(
    fakes: &Fakes,
    username: &str,
    password: &str,
) -> (FakeServer, FakeServer, Servers) {
    let imap = FakeServer::imap(Security::Tls, Script::accepting(username, password)).await;
    let smtp = FakeServer::smtp(Security::Starttls, Script::accepting(username, password)).await;
    fakes.resolver.name(IMAP_HOST, &["127.0.0.1"]);
    fakes.resolver.name(SMTP_HOST, &["127.0.0.1"]);
    let servers = Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp.at(SMTP_HOST),
    };
    (imap, smtp, servers)
}
