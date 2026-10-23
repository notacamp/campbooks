//! Fake mail servers for the IMAP adapter's tests: an IMAP server and an
//! SMTP submission server, in process on 127.0.0.1, scripted (the password
//! they accept, what they offer), over TLS with a test certificate the
//! tests trust (`fixtures/imap/`: a test root and a certificate for
//! `imap.example.com`, `smtp.example.com` and `mail.example.com`, valid
//! until 2126, made with `openssl`; no real host has them). A fake resolver
//! names the fakes; fake DNS and a fake web serve discovery. The IMAP fake
//! holds a mailbox (`testkit_imap_mailbox`) the tests change as another mail
//! app would.
//!
//! No test talks to a real mail server, resolver or web server.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::net::guard::{Resolve, ResolveFuture};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::{
    io::{AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader},
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
}

#[derive(Debug)]
struct Shared {
    script: Mutex<Script>,
    seen: Mutex<Seen>,
    store: Mutex<Store>,
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
                if encrypted {
                    let offers = script.offers;
                    caps.push("ENABLE");
                    for (offered, name) in [
                        (offers.special_use, "SPECIAL-USE"),
                        (offers.condstore, "CONDSTORE"),
                        (offers.qresync, "QRESYNC"),
                        (offers.objectid, "OBJECTID"),
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
                let response = match initial.filter(|r| !r.is_empty() && script.sasl_ir) {
                    Some(response) => response.to_owned(),
                    None => {
                        write(conn.get_mut(), "+ \r\n").await;
                        read(conn).await.unwrap_or_default()
                    }
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
                write(
                    conn.get_mut(),
                    &format!("* BYE see you\r\n{tag} OK done\r\n"),
                )
                .await;
                return Next::Done;
            }
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
    shared.see(|seen| {
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
    while let Some(line) = read(conn).await {
        let (verb, args) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let reply = match verb.to_ascii_uppercase().as_str() {
            "EHLO" | "HELO" => {
                let mut lines = vec!["smtp.example.com"];
                if !encrypted && script.starttls {
                    lines.push("STARTTLS");
                }
                if encrypted && script.auth {
                    lines.push("AUTH PLAIN LOGIN");
                }
                lines.push("8BITMIME");
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
                if accepted {
                    "235 2.7.0 accepted\r\n".to_owned()
                } else {
                    "535 5.7.8 credentials refused\r\n".to_owned()
                }
            }
            "MAIL" => {
                shared.see(|seen| seen.mail_from = true);
                "250 ok\r\n".to_owned()
            }
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
    }
}

/// The module with IMAP on over fresh fakes.
pub async fn kit(db: PgPool) -> (Kit, Fakes) {
    kit_with(db, |_| {}).await
}

/// [`kit`], its IMAP settings changed by `change`.
pub async fn kit_with(db: PgPool, change: impl FnOnce(&mut ImapConfig)) -> (Kit, Fakes) {
    let fakes = Fakes {
        resolver: Arc::new(FakeResolver::default()),
        dns: Arc::new(FakeDns::default()),
        web: Arc::new(FakeWeb::default()),
    };
    let mut imap = adapter(&fakes);
    change(&mut imap.config);
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
