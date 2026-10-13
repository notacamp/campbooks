//! Mail the instance itself sends (security notices first; invitations
//! later): one [`Mailer`] port in [`AppState`](crate::AppState), backed by an
//! SMTP server the operator configures, or by nothing.
//!
//! - `CAMPBOOKS_MAIL__SMTP_URL` (or `…_FILE`): the transport, e.g.
//!   `smtps://user:password@smtp.example.com` or
//!   `smtp://user:password@smtp.example.com:587?tls=required` (STARTTLS).
//!   Secret: it is never logged, and errors never repeat it.
//! - `CAMPBOOKS_MAIL__FROM`: the sender, e.g.
//!   `Campbooks <no-reply@example.com>`.
//!
//! With neither set, [`NoMailer`] is installed: nothing is sent and callers
//! fall back to what they show in the app. Mail is sent from jobs (retried,
//! idempotent per message), never inline in a request. A [`Mail`] carries a
//! plain-text and an HTML part; callers keep HTML free of remote content.

use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
};

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart},
    transport::smtp::AsyncSmtpTransportBuilder,
};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

/// `CAMPBOOKS_MAIL__*`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MailConfig {
    /// The SMTP transport URL (`CAMPBOOKS_MAIL__SMTP_URL`). Secret.
    pub smtp_url: Option<SecretString>,
    /// A file holding it (`CAMPBOOKS_MAIL__SMTP_URL_FILE`), read at startup.
    pub smtp_url_file: Option<PathBuf>,
    /// The sender (`CAMPBOOKS_MAIL__FROM`).
    pub from: Option<String>,
}

impl MailConfig {
    /// The transport URL, from the setting or its file; `None` when unset
    /// or blank.
    fn url(&self) -> Result<Option<SecretString>, String> {
        if let Some(path) = self
            .smtp_url_file
            .as_ref()
            .filter(|p| !p.as_os_str().is_empty())
        {
            let text = std::fs::read_to_string(path).map_err(|e| {
                format!(
                    "CAMPBOOKS_MAIL__SMTP_URL_FILE={}: cannot read it: {e}",
                    path.display()
                )
            })?;
            let text = text.trim().to_owned();
            return Ok((!text.is_empty()).then(|| SecretString::from(text)));
        }
        Ok(self
            .smtp_url
            .as_ref()
            .filter(|url| !url.expose_secret().trim().is_empty())
            .cloned())
    }

    /// The transport and sender these settings describe: `None` when
    /// neither is set. One without the other, an unreadable file, a
    /// malformed URL or sender is an error naming the setting (never
    /// repeating the URL).
    fn parts(&self) -> Result<Option<(AsyncSmtpTransportBuilder, Mailbox)>, String> {
        let url = self.url()?;
        let from = self
            .from
            .as_deref()
            .map(str::trim)
            .filter(|from| !from.is_empty());
        match (url, from) {
            (None, None) => Ok(None),
            (Some(_), None) => Err(
                "CAMPBOOKS_MAIL__SMTP_URL is set but CAMPBOOKS_MAIL__FROM is not: say which \
                 address the instance sends from"
                    .to_owned(),
            ),
            (None, Some(_)) => Err(
                "CAMPBOOKS_MAIL__FROM is set but CAMPBOOKS_MAIL__SMTP_URL is not: say which \
                 server sends the mail"
                    .to_owned(),
            ),
            (Some(url), Some(from)) => {
                let sender: Mailbox = from.parse().map_err(|_| {
                    format!(
                        "CAMPBOOKS_MAIL__FROM={from} is not an address such as \
                         `Campbooks <no-reply@example.com>`"
                    )
                })?;
                let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url.expose_secret())
                    .map_err(|_| {
                        "CAMPBOOKS_MAIL__SMTP_URL is not an smtp:// or smtps:// URL".to_owned()
                    })?;
                Ok(Some((transport, sender)))
            }
        }
    }

    /// Startup check: the settings are usable (see [`mailer`](Self::mailer)).
    pub fn check(&self) -> Result<(), String> {
        self.parts().map(|_| ())
    }

    /// The mailer these settings describe: SMTP when both the transport and
    /// the sender are set, [`NoMailer`] when neither is.
    pub fn mailer(&self) -> Result<MailerPort, String> {
        Ok(match self.parts()? {
            None => Arc::new(NoMailer),
            Some((transport, from)) => Arc::new(Smtp {
                transport: transport.build(),
                from,
            }),
        })
    }
}

/// One message to one recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    /// The recipient's address.
    pub to: String,
    /// The subject.
    pub subject: String,
    /// The plain-text part.
    pub text: String,
    /// The HTML part: no remote content, no tracking.
    pub html: String,
}

/// Why a message was not sent. Never carries an address, the transport URL
/// or the server's words (which can repeat an address).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MailError {
    /// No transport is configured.
    #[error("no mail transport is configured")]
    NotConfigured,
    /// The recipient is not an address.
    #[error("the recipient is not an email address")]
    BadRecipient,
    /// The server refused for good (an unknown recipient, a policy).
    #[error("the mail server refused the message")]
    Refused,
    /// The server could not be reached or failed for now: try again.
    #[error("the mail server is unavailable")]
    Unavailable,
}

/// A future returned by a [`Mailer`].
pub type MailFuture<'a> = Pin<Box<dyn Future<Output = Result<(), MailError>> + Send + 'a>>;

/// The port that sends the instance's mail.
pub trait Mailer: std::fmt::Debug + Send + Sync + 'static {
    /// Whether mail is sent at all.
    fn is_configured(&self) -> bool;

    /// Send `mail`.
    fn send<'a>(&'a self, mail: &'a Mail) -> MailFuture<'a>;
}

/// Shared handle to the registered [`Mailer`].
pub type MailerPort = Arc<dyn Mailer>;

/// No transport: nothing is sent.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMailer;

impl Mailer for NoMailer {
    fn is_configured(&self) -> bool {
        false
    }

    fn send<'a>(&'a self, _mail: &'a Mail) -> MailFuture<'a> {
        Box::pin(async { Err(MailError::NotConfigured) })
    }
}

/// An SMTP server.
struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl std::fmt::Debug for Smtp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Smtp").finish_non_exhaustive()
    }
}

impl Mailer for Smtp {
    fn is_configured(&self) -> bool {
        true
    }

    fn send<'a>(&'a self, mail: &'a Mail) -> MailFuture<'a> {
        Box::pin(async move {
            let to: Mailbox = mail.to.parse().map_err(|_| MailError::BadRecipient)?;
            let message = Message::builder()
                .from(self.from.clone())
                .to(to)
                .subject(mail.subject.clone())
                .multipart(MultiPart::alternative_plain_html(
                    mail.text.clone(),
                    mail.html.clone(),
                ))
                .map_err(|_| MailError::BadRecipient)?;
            self.transport
                .send(message)
                .await
                .map(|_| ())
                .map_err(|err| {
                    if err.is_permanent() {
                        MailError::Refused
                    } else {
                        MailError::Unavailable
                    }
                })
        })
    }
}

/// A mailer that keeps what it is asked to send, for tests (in place of a
/// server; nothing leaves the process).
#[derive(Debug, Default)]
pub struct Outbox {
    sent: Mutex<Vec<Mail>>,
    failing: Mutex<bool>,
}

impl Outbox {
    /// Everything sent so far.
    pub fn sent(&self) -> Vec<Mail> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Make sends fail as unavailable (`true`) or succeed again.
    pub fn fail(&self, failing: bool) {
        *self.failing.lock().unwrap_or_else(PoisonError::into_inner) = failing;
    }
}

impl Mailer for Outbox {
    fn is_configured(&self) -> bool {
        true
    }

    fn send<'a>(&'a self, mail: &'a Mail) -> MailFuture<'a> {
        let failing = *self.failing.lock().unwrap_or_else(PoisonError::into_inner);
        if !failing {
            self.sent
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(mail.clone());
        }
        Box::pin(async move {
            if failing {
                Err(MailError::Unavailable)
            } else {
                Ok(())
            }
        })
    }
}

/// `text` safe inside HTML (`&`, `<`, `>`, `"` and `'` escaped).
pub fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::{
        io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader},
        net::TcpListener,
    };

    use super::*;

    /// A minimal SMTP server on loopback that accepts one message and hands
    /// back what it was sent (the envelope and the data).
    async fn fake_smtp() -> (u16, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let port = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("{e}"))
            .port();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap_or_else(|e| panic!("{e}"));
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            let mut transcript = String::new();
            write
                .write_all(b"220 fake ESMTP\r\n")
                .await
                .unwrap_or_default();
            let mut in_data = false;
            while let Ok(Some(line)) = lines.next_line().await {
                transcript.push_str(&line);
                transcript.push('\n');
                if in_data {
                    if line == "." {
                        in_data = false;
                        write.write_all(b"250 queued\r\n").await.unwrap_or_default();
                    }
                    continue;
                }
                let verb = line
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                let reply: &[u8] = match verb.as_str() {
                    "EHLO" | "HELO" => b"250 fake\r\n",
                    "DATA" => {
                        in_data = true;
                        b"354 go on\r\n"
                    }
                    "QUIT" => {
                        write.write_all(b"221 bye\r\n").await.unwrap_or_default();
                        break;
                    }
                    _ => b"250 ok\r\n",
                };
                write.write_all(reply).await.unwrap_or_default();
            }
            transcript
        });
        (port, server)
    }

    fn config(url: Option<&str>, from: Option<&str>) -> MailConfig {
        MailConfig {
            smtp_url: url.map(|u| SecretString::from(u.to_owned())),
            smtp_url_file: None,
            from: from.map(ToOwned::to_owned),
        }
    }

    #[tokio::test]
    async fn smtp_sends_both_parts_to_the_recipient() {
        let (port, server) = fake_smtp().await;
        let mailer = config(
            Some(&format!("smtp://127.0.0.1:{port}")),
            Some("Campbooks <no-reply@example.com>"),
        )
        .mailer()
        .unwrap_or_else(|e| panic!("{e}"));
        assert!(mailer.is_configured());
        mailer
            .send(&Mail {
                to: "ana@example.com".to_owned(),
                subject: "A new way in".to_owned(),
                text: "Plain words.".to_owned(),
                html: "<p>Plain words.</p>".to_owned(),
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let transcript = server.await.unwrap_or_else(|e| panic!("{e}"));
        assert!(
            transcript.contains("MAIL FROM:<no-reply@example.com>"),
            "{transcript}"
        );
        assert!(
            transcript.contains("RCPT TO:<ana@example.com>"),
            "{transcript}"
        );
        assert!(transcript.contains("text/plain"), "{transcript}");
        assert!(transcript.contains("text/html"), "{transcript}");
        assert!(transcript.contains("Subject: A new way in"), "{transcript}");
    }

    #[test]
    fn settings_come_in_pairs_and_never_repeat_the_url() {
        assert!(
            !config(None, None)
                .mailer()
                .unwrap_or_else(|e| panic!("{e}"))
                .is_configured()
        );
        let secret = "smtp://user:hunter2@smtp.example.com";
        let half = config(Some(secret), None).mailer().unwrap_err();
        assert!(half.contains("CAMPBOOKS_MAIL__FROM"), "{half}");
        assert!(!half.contains("hunter2"));
        let other = config(None, Some("no-reply@example.com"))
            .mailer()
            .unwrap_err();
        assert!(other.contains("CAMPBOOKS_MAIL__SMTP_URL"), "{other}");
        let bad = config(Some("ftp://user:hunter2@x"), Some("no-reply@example.com"))
            .mailer()
            .unwrap_err();
        assert!(
            bad.contains("CAMPBOOKS_MAIL__SMTP_URL") && !bad.contains("hunter2"),
            "{bad}"
        );
        let sender = config(Some(secret), Some("not an address"))
            .mailer()
            .unwrap_err();
        assert!(sender.contains("CAMPBOOKS_MAIL__FROM"), "{sender}");
        assert!(!format!("{:?}", config(Some(secret), None)).contains("hunter2"));
    }

    #[tokio::test]
    async fn the_outbox_keeps_or_fails() {
        let outbox = Arc::new(Outbox::default());
        let mail = Mail {
            to: "ana@example.com".to_owned(),
            subject: "s".to_owned(),
            text: "t".to_owned(),
            html: "h".to_owned(),
        };
        outbox.fail(true);
        assert_eq!(outbox.send(&mail).await, Err(MailError::Unavailable));
        outbox.fail(false);
        outbox.send(&mail).await.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(outbox.sent(), [mail]);
        assert_eq!(
            NoMailer.send(&outbox.sent()[0]).await,
            Err(MailError::NotConfigured)
        );
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(
            escape_html("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }
}
