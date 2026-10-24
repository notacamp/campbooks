//! A mailbox's SMTP submission server: checking it at connect ([`verify`]:
//! authenticate and quit, no `MAIL FROM`), and sending through it
//! ([`Submission`], campbooks-cloud#313).
//!
//! Both connect to the address the guard checked, never to the name again;
//! TLS from the first byte (465) or STARTTLS (587, refused when it is not
//! offered: no password, no `AUTH` and no `MAIL` ever go in the clear), the
//! certificate checked by the platform verifier against the host name; then
//! `AUTH PLAIN` or `AUTH LOGIN`.
//!
//! A submission speaks over a connection Campbooks opens itself and bounds:
//! at most [`MAX_RECEIVED`] bytes come from the server in one session (the
//! client reads a reply's lines before it measures them), every step has a
//! time limit, and the message goes out as its own step after `DATA` was
//! accepted, so a failure says whether the message can have been sent
//! ([`Failed`]). Nothing the server says is kept or logged but its reply
//! code.

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use lettre::{
    Address,
    transport::smtp::{
        Error as SmtpError,
        authentication::{Credentials, Mechanism},
        client::{AsyncSmtpConnection, AsyncTokioStream, TlsParameters},
        commands::{Data, Ehlo, Mail, Quit, Rcpt},
        extension::{ClientId, MailBodyParameter, MailParameter},
    },
};
use secrecy::{ExposeSecret as _, SecretString};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    time::{Instant, timeout, timeout_at},
};
use tokio_rustls::TlsConnector;

use super::{Imap, Refusal, Security, Server, tls};

/// The mechanisms Campbooks authenticates with, preferred first.
const MECHANISMS: [Mechanism; 2] = [Mechanism::Plain, Mechanism::Login];

/// Authenticate to `server` as `username` with `password`, within the
/// configured time, and quit.
pub async fn verify(
    imap: &Imap,
    server: &Server,
    username: &str,
    password: &SecretString,
) -> Result<(), Refusal> {
    let address = imap
        .reach(server)
        .await
        .map_err(|refused| Refusal::of(refused, Refusal::SmtpUnreachable))?;
    tls::server_name(&server.host).ok_or(Refusal::HostRefused)?;
    let parameters =
        tls::smtp_parameters(&imap.trust, &server.host).map_err(|_| Refusal::SmtpTlsFailed)?;
    let attempt = Attempt {
        address,
        security: server.security,
        parameters,
        connect_timeout: imap.connect_timeout(),
    };
    timeout(imap.timeout(), attempt.run(username, password))
        .await
        .unwrap_or(Err(Refusal::SmtpUnreachable))
}

/// One authentication, to the checked address.
struct Attempt {
    address: SocketAddr,
    security: Security,
    parameters: TlsParameters,
    connect_timeout: Duration,
}

impl Attempt {
    async fn run(self, username: &str, password: &SecretString) -> Result<(), Refusal> {
        // An address literal, not this machine's name: nothing about the
        // instance is told to the server.
        let hello = ClientId::Ipv4(Ipv4Addr::LOCALHOST);
        let connect_timeout = Some(self.connect_timeout);
        let mut connection = match self.security {
            Security::Tls => AsyncSmtpConnection::connect_tokio1(
                self.address,
                connect_timeout,
                &hello,
                Some(self.parameters),
                None,
            )
            .await
            .map_err(|e| failed(&e))?,
            Security::Starttls => {
                let mut connection = AsyncSmtpConnection::connect_tokio1(
                    self.address,
                    connect_timeout,
                    &hello,
                    None,
                    None,
                )
                .await
                .map_err(|_| Refusal::SmtpUnreachable)?;
                if !connection.can_starttls() {
                    // Nothing more is said: no password in the clear.
                    connection.abort().await;
                    return Err(Refusal::StarttlsMissing);
                }
                connection
                    .starttls(self.parameters, &hello)
                    .await
                    .map_err(|e| failed(&e))?;
                connection
            }
        };
        if !connection.is_encrypted() {
            connection.abort().await;
            return Err(Refusal::SmtpTlsFailed);
        }
        let offered = MECHANISMS
            .iter()
            .any(|&m| connection.server_info().supports_auth_mechanism(m));
        if !offered {
            // A submission server that takes mail from anyone is not one
            // Campbooks sends through.
            connection.abort().await;
            return Err(Refusal::SmtpLoginRefused);
        }
        let credentials =
            Credentials::new(username.to_owned(), password.expose_secret().to_owned());
        let outcome = match connection.auth(&MECHANISMS, &credentials).await {
            Ok(response) if response.is_positive() => Ok(()),
            Ok(_) => Err(Refusal::SmtpLoginRefused),
            Err(err) if err.is_permanent() || err.is_transient() => {
                tracing::info!(
                    code = err.status().map(u16::from),
                    "the SMTP server refused the sign-in"
                );
                Err(Refusal::SmtpLoginRefused)
            }
            Err(_) => Err(Refusal::SmtpUnreachable),
        };
        let _ = connection.quit().await;
        outcome
    }
}

/// What a failed connection or upgrade means: a certificate (or TLS)
/// problem, or a server that is not there.
fn failed(err: &lettre::transport::smtp::Error) -> Refusal {
    if err.is_tls() || tls::is_tls_failure(err) {
        Refusal::SmtpTlsFailed
    } else {
        Refusal::SmtpUnreachable
    }
}

/// The most bytes a server may send in one submission session: its replies
/// are a few lines each.
pub const MAX_RECEIVED: u64 = 1024 * 1024;

/// How long saying goodbye may take.
const QUIT_WAIT: Duration = Duration::from_secs(5);

/// The address literal Campbooks greets with: nothing about the instance is
/// told to the server.
fn hello() -> ClientId {
    ClientId::Ipv4(Ipv4Addr::LOCALHOST)
}

/// A stream whose reads fail once [`MAX_RECEIVED`] bytes came through it.
#[derive(Debug)]
struct Bounded<S> {
    inner: S,
    peer: SocketAddr,
    received: u64,
}

impl<S> Bounded<S> {
    fn new(inner: S, peer: SocketAddr) -> Self {
        Self {
            inner,
            peer,
            received: 0,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Bounded<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = &mut *self;
        if this.received >= MAX_RECEIVED {
            return Poll::Ready(Err(io::Error::other("the server sent too much")));
        }
        let before = buf.filled().len();
        let polled = Pin::new(&mut this.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &polled {
            let read = buf.filled().len().saturating_sub(before);
            this.received = this
                .received
                .saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        }
        polled
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Bounded<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> AsyncTokioStream for Bounded<S>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + std::fmt::Debug,
{
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.peer)
    }
}

/// Why a submission did not send a message, or may have: a stable answer,
/// never the server's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failed {
    /// Nothing was sent, and a later attempt may work: the server is busy
    /// (its 4xx reply code), could not be reached, timed out or broke off
    /// before the message.
    Retry(Option<u16>),
    /// Nothing was sent, and trying again will not help: the server refused
    /// it (its 5xx reply code), or it needs what the server does not offer.
    Refused(Option<u16>),
    /// Nothing was sent: the message is larger than the server takes.
    TooLarge,
    /// The server refused the password (a 5xx answer to `AUTH`), or offers
    /// no sign-in at all.
    LoginRefused,
    /// The server cannot be used safely (a stable code): no STARTTLS, a
    /// certificate not valid for its name, a host the guard refuses.
    Unsafe(&'static str),
    /// The message went out and no final answer came: it may have been
    /// sent. Never send it again.
    Unclear,
}

/// The reply code of `err`, when it is the server's answer.
fn code(err: &SmtpError) -> Option<u16> {
    err.status().map(u16::from)
}

/// What a failure before the message means: nothing was sent; a 5xx is a
/// refusal, anything else may pass later.
fn before_message(err: &SmtpError) -> Failed {
    if err.is_permanent() {
        Failed::Refused(code(err))
    } else {
        Failed::Retry(code(err))
    }
}

/// What a TLS handshake that failed means.
fn tls_failed(err: &(dyn std::error::Error + 'static)) -> Failed {
    if tls::is_tls_failure(err) {
        Failed::Unsafe("smtp_tls_failed")
    } else {
        Failed::Retry(None)
    }
}

/// A signed-in SMTP submission session, for one message.
pub struct Submission {
    connection: AsyncSmtpConnection,
    /// The server announced `SIZE` (and its limit, when it named one).
    size_offered: bool,
    size_limit: Option<u64>,
    eight_bit: bool,
    utf8: bool,
    /// How long one step may take.
    step: Duration,
    /// When the whole submission must be over.
    deadline: Instant,
}

impl std::fmt::Debug for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Submission")
            .field("size_limit", &self.size_limit)
            .finish_non_exhaustive()
    }
}

/// `connection`, closed: `QUIT` within [`QUIT_WAIT`], whatever the answer.
async fn close(mut connection: AsyncSmtpConnection) {
    let _ = timeout(QUIT_WAIT, connection.command(Quit)).await;
}

impl Submission {
    /// Connect to `server` and sign in as `username` with `password`; the
    /// whole submission must end within `total` from now.
    pub async fn open(
        imap: &Imap,
        server: &Server,
        username: &str,
        password: &SecretString,
        total: Duration,
    ) -> Result<Self, Failed> {
        let deadline = Instant::now() + total;
        let step = imap.timeout();
        // Every step within its own time and the whole submission's.
        let within = |limit: Duration| deadline.min(Instant::now() + limit);
        let address = timeout_at(within(step), imap.reach(server))
            .await
            .map_err(|_| Failed::Retry(None))?
            .map_err(
                |refused| match Refusal::of(refused, Refusal::SmtpUnreachable) {
                    Refusal::SmtpUnreachable => Failed::Retry(None),
                    _ => Failed::Unsafe("host_refused"),
                },
            )?;
        let name = tls::server_name(&server.host).ok_or(Failed::Unsafe("host_refused"))?;
        let tcp = timeout_at(within(imap.connect_timeout()), TcpStream::connect(address))
            .await
            .map_err(|_| Failed::Retry(None))?
            .map_err(|_| Failed::Retry(None))?;
        let hello = hello();
        let connection = match server.security {
            Security::Tls => {
                let config = tls::client_config(&imap.trust)
                    .map_err(|_| Failed::Unsafe("smtp_tls_failed"))?;
                let tls = timeout_at(within(step), TlsConnector::from(config).connect(name, tcp))
                    .await
                    .map_err(|_| Failed::Retry(None))?
                    .map_err(|e| tls_failed(&e))?;
                let stream: Box<dyn AsyncTokioStream> = Box::new(Bounded::new(tls, address));
                timeout_at(
                    within(step),
                    AsyncSmtpConnection::connect_with_transport(stream, &hello),
                )
                .await
                .map_err(|_| Failed::Retry(None))?
                .map_err(|e| Failed::Retry(code(&e)))?
            }
            Security::Starttls => {
                let stream: Box<dyn AsyncTokioStream> = Box::new(Bounded::new(tcp, address));
                let mut connection = timeout_at(
                    within(step),
                    AsyncSmtpConnection::connect_with_transport(stream, &hello),
                )
                .await
                .map_err(|_| Failed::Retry(None))?
                .map_err(|e| Failed::Retry(code(&e)))?;
                if !connection.can_starttls() {
                    // Nothing more is said: no password in the clear.
                    close(connection).await;
                    return Err(Failed::Unsafe("starttls_missing"));
                }
                let parameters = tls::smtp_parameters(&imap.trust, &server.host)
                    .map_err(|_| Failed::Unsafe("smtp_tls_failed"))?;
                match timeout_at(within(step), connection.starttls(parameters, &hello)).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) if e.is_tls() || tls::is_tls_failure(&e) => {
                        return Err(Failed::Unsafe("smtp_tls_failed"));
                    }
                    Ok(Err(e)) => return Err(Failed::Retry(code(&e))),
                    Err(_) => return Err(Failed::Retry(None)),
                }
                if !connection.is_encrypted() {
                    close(connection).await;
                    return Err(Failed::Unsafe("smtp_tls_failed"));
                }
                connection
            }
        };
        let mut submission = Self {
            connection,
            size_offered: false,
            size_limit: None,
            eight_bit: false,
            utf8: false,
            step,
            deadline,
        };
        if let Err(failed) = submission.sign_in(username, password).await {
            close(submission.connection).await;
            return Err(failed);
        }
        if let Err(failed) = submission.extensions().await {
            close(submission.connection).await;
            return Err(failed);
        }
        Ok(submission)
    }

    /// How long the next step may take: [`step`](Self::step), never past
    /// the submission's deadline.
    fn until(&self) -> Instant {
        self.deadline.min(Instant::now() + self.step)
    }

    /// `AUTH PLAIN`, else `AUTH LOGIN`. A busy answer (4xx) is told from a
    /// refusal (5xx); a server that offers neither refuses.
    async fn sign_in(&mut self, username: &str, password: &SecretString) -> Result<(), Failed> {
        let offered = MECHANISMS
            .iter()
            .any(|&m| self.connection.server_info().supports_auth_mechanism(m));
        if !offered {
            // A submission server that takes mail from anyone is not one
            // Campbooks sends through.
            return Err(Failed::LoginRefused);
        }
        let credentials =
            Credentials::new(username.to_owned(), password.expose_secret().to_owned());
        let until = self.until();
        match timeout_at(until, self.connection.auth(&MECHANISMS, &credentials)).await {
            Ok(Ok(response)) if response.is_positive() => Ok(()),
            Ok(Ok(_)) => Err(Failed::LoginRefused),
            Ok(Err(err)) if err.is_permanent() => {
                tracing::info!(code = code(&err), "the SMTP server refused the sign-in");
                Err(Failed::LoginRefused)
            }
            Ok(Err(err)) => {
                tracing::info!(code = code(&err), "the SMTP server did not sign in");
                Err(Failed::Retry(code(&err)))
            }
            Err(_) => Err(Failed::Retry(None)),
        }
    }

    /// Ask EHLO again, signed in, for what `lettre` does not keep: `SIZE`
    /// (and its limit), with `8BITMIME` and `SMTPUTF8`.
    async fn extensions(&mut self) -> Result<(), Failed> {
        let until = self.until();
        let response = timeout_at(until, self.connection.command(Ehlo::new(hello())))
            .await
            .map_err(|_| Failed::Retry(None))?
            .map_err(|e| Failed::Retry(code(&e)))?;
        for line in response.message() {
            let mut words = line.split_ascii_whitespace();
            let Some(keyword) = words.next().map(str::to_ascii_uppercase) else {
                continue;
            };
            match keyword.as_str() {
                "SIZE" => {
                    self.size_offered = true;
                    self.size_limit = words
                        .next()
                        .and_then(|n| n.parse::<u64>().ok())
                        .filter(|n| *n > 0);
                }
                "8BITMIME" => self.eight_bit = true,
                "SMTPUTF8" => self.utf8 = true,
                _ => {}
            }
        }
        Ok(())
    }

    /// Offer a message of `size` bytes from `from` to `to`: `MAIL FROM`,
    /// each `RCPT TO`, then `DATA`, up to the server's go-ahead (354).
    /// Nothing of the message is sent yet; a refused recipient refuses it
    /// all.
    pub async fn offer(
        &mut self,
        from: &Address,
        to: &[Address],
        size: usize,
        ascii: bool,
    ) -> Result<(), Failed> {
        if self
            .size_limit
            .is_some_and(|max| u64::try_from(size).unwrap_or(u64::MAX) > max)
        {
            return Err(Failed::TooLarge);
        }
        let non_ascii_address = std::iter::once(from)
            .chain(to)
            .any(|a| !AsRef::<str>::as_ref(a).is_ascii());
        if (non_ascii_address && !self.utf8) || (!ascii && !self.eight_bit) {
            return Err(Failed::Refused(None));
        }
        let mut parameters = Vec::new();
        if self.size_offered {
            parameters.push(MailParameter::Size(size));
        }
        if !ascii {
            parameters.push(MailParameter::Body(MailBodyParameter::EightBitMime));
        }
        if non_ascii_address {
            parameters.push(MailParameter::SmtpUtfEight);
        }
        let until = self.until();
        timeout_at(
            until,
            self.connection
                .command(Mail::new(Some(from.clone()), parameters)),
        )
        .await
        .map_err(|_| Failed::Retry(None))?
        .map_err(|e| before_message(&e))?;
        for recipient in to {
            let until = self.until();
            timeout_at(
                until,
                self.connection
                    .command(Rcpt::new(recipient.clone(), Vec::new())),
            )
            .await
            .map_err(|_| Failed::Retry(None))?
            .map_err(|e| before_message(&e))?;
        }
        let until = self.until();
        timeout_at(until, self.connection.command(Data))
            .await
            .map_err(|_| Failed::Retry(None))?
            .map_err(|e| before_message(&e))?;
        Ok(())
    }

    /// Send the message, once [`offer`](Self::offer) was accepted, within
    /// what is left of the submission's time. A 4xx or 5xx answer to it is
    /// definite (it was not taken); anything but an answer is
    /// [`Failed::Unclear`].
    pub async fn message(&mut self, message: &[u8]) -> Result<(), Failed> {
        match timeout_at(self.deadline, self.connection.message(message)).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(err)) if err.is_permanent() && code(&err) == Some(552) => Err(Failed::TooLarge),
            Ok(Err(err)) if err.is_permanent() => Err(Failed::Refused(code(&err))),
            Ok(Err(err)) if err.is_transient() => Err(Failed::Retry(code(&err))),
            Ok(Err(_)) | Err(_) => Err(Failed::Unclear),
        }
    }

    /// Say goodbye, briefly; whatever the server answers.
    pub async fn quit(self) {
        close(self.connection).await;
    }

    /// Drop the connection without a word, in the middle of `DATA`: no
    /// `QUIT` (it would be taken as part of the message) and no end of data,
    /// so the server delivers nothing of it.
    pub fn abandon(self) {
        drop(self.connection);
    }
}
