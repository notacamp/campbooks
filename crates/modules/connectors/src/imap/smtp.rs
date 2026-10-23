//! Checking a mailbox's SMTP submission server: connect to the address the
//! guard checked, TLS from the first byte or STARTTLS (refused when it is
//! not offered), then authenticate (`PLAIN` or `LOGIN`) and quit. No
//! message is sent, and no `MAIL FROM` either.

use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use lettre::transport::smtp::{
    authentication::{Credentials, Mechanism},
    client::{AsyncSmtpConnection, TlsParameters},
    extension::ClientId,
};
use secrecy::{ExposeSecret as _, SecretString};
use tokio::time::timeout;

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
