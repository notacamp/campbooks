//! TLS to mail servers: rustls (aws-lc-rs), certificates checked against
//! the server's host name by the platform's verifier (or, in tests, against
//! the test servers' own root).

use std::sync::Arc;

use lettre::transport::smtp::client::{Certificate, CertificateStore, TlsParameters};
use rustls_platform_verifier::BuilderVerifierExt as _;
use tokio_rustls::rustls::{
    self, ClientConfig, RootCertStore, crypto::aws_lc_rs, pki_types::ServerName,
};

use super::Trust;

/// A client configuration that checks certificates as `trust` says.
pub fn client_config(trust: &Trust) -> Result<Arc<ClientConfig>, rustls::Error> {
    let builder = ClientConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()?;
    let config = match trust {
        Trust::Platform => builder.with_platform_verifier()?.with_no_client_auth(),
        Trust::Roots(roots) => {
            let mut store = RootCertStore::empty();
            for root in roots {
                store.add(root.clone())?;
            }
            builder.with_root_certificates(store).with_no_client_auth()
        }
    };
    Ok(Arc::new(config))
}

/// The name the server's certificate must be valid for.
pub fn server_name(host: &str) -> Option<ServerName<'static>> {
    ServerName::try_from(host.to_owned()).ok()
}

/// The SMTP client's TLS settings for `host`, checking certificates as
/// `trust` says.
pub fn smtp_parameters(
    trust: &Trust,
    host: &str,
) -> Result<TlsParameters, lettre::transport::smtp::Error> {
    let mut builder = TlsParameters::builder(host.to_owned());
    if let Trust::Roots(roots) = trust {
        builder = builder.certificate_store(CertificateStore::None);
        for root in roots {
            builder = builder.add_root_certificate(Certificate::from_der(root.to_vec())?);
        }
    }
    builder.build_rustls()
}

/// Whether `err`, or an error it was caused by, is a TLS failure (a
/// certificate not valid for the name, a handshake the server broke off)
/// rather than a network one.
pub fn is_tls_failure(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut next = Some(err);
    while let Some(err) = next {
        if err.is::<rustls::Error>() {
            return true;
        }
        if let Some(io) = err.downcast_ref::<std::io::Error>()
            && io
                .get_ref()
                .is_some_and(|inner| inner.is::<rustls::Error>())
        {
            return true;
        }
        next = err.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_failures_are_told_from_network_ones() {
        let tls = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName),
        );
        assert!(is_tls_failure(&tls));
        let network = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        assert!(!is_tls_failure(&network));
    }

    #[test]
    fn both_trusts_build() {
        assert!(client_config(&Trust::Roots(Vec::new())).is_ok());
        assert!(server_name("imap.example.com").is_some());
        assert!(server_name("192.0.2.1").is_some());
        assert!(server_name("not a name").is_none());
    }
}
