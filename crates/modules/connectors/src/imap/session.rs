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

use async_imap::error::Error as ImapError;
use futures_util::TryStreamExt as _;
use secrecy::{ExposeSecret as _, SecretString};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::TcpStream,
    time::timeout,
};
use tokio_rustls::{TlsConnector, client::TlsStream};

use super::{Imap, Refusal, Security, Server, tls};

/// The longest line read before signing in.
const MAX_LINE: usize = 8 * 1024;

/// The most lines one command's answer may take before signing in.
const MAX_LINES: usize = 64;

/// A signed-in IMAP session.
#[derive(Debug)]
pub struct Session {
    inner: async_imap::Session<TlsStream<TcpStream>>,
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
        Ok(Self { inner })
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

    /// Say goodbye; whatever the server answers.
    pub async fn logout(mut self) {
        let _ = self.inner.logout().await;
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
            tracing::info!(
                code = response_code(answer),
                "the IMAP server refused the sign-in"
            );
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
