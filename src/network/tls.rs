use std::sync::Arc;

use futures_rustls::{
    TlsConnector,
    rustls::{
        self, ClientConfig,
        pki_types::{CertificateDer, ServerName, pem::PemObject},
        version::TLS13,
    },
};
use rustls_platform_verifier::Verifier;

#[derive(Debug, thiserror::Error)]
pub enum TlsSetupError {
    #[error("{0} is not host:port")]
    Address(String),
    #[error("{0} is not a valid server name")]
    ServerName(String),
    #[error("cannot read the extra CA {0}: {1}")]
    ExtraCa(String, rustls::pki_types::pem::Error),
    #[error("the extra CA {0} contains no certificates")]
    EmptyExtraCa(String),
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
}

/// The host part of `host:port`, which is what the certificate must name.
pub fn server_name(address: &str) -> Result<ServerName<'static>, TlsSetupError> {
    let (host, _port) = address
        .rsplit_once(':')
        .ok_or_else(|| TlsSetupError::Address(address.to_string()))?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    ServerName::try_from(host.to_string()).map_err(|_| TlsSetupError::ServerName(host.to_string()))
}

/// Trusts what the operating system trusts, as the HTTPS login does, plus `extra_ca`.
pub fn connector(extra_ca: Option<&str>) -> Result<TlsConnector, TlsSetupError> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let verifier = match extra_ca {
        Some(path) => {
            let roots = CertificateDer::pem_file_iter(path)
                .and_then(|certs| certs.collect::<Result<Vec<_>, _>>())
                .map_err(|e| TlsSetupError::ExtraCa(path.to_string(), e))?;
            if roots.is_empty() {
                return Err(TlsSetupError::EmptyExtraCa(path.to_string()));
            }
            Verifier::new_with_extra_roots(roots, Arc::clone(&provider))?
        }
        None => Verifier::new(Arc::clone(&provider))?,
    };

    let config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_is_the_server_name() {
        assert_eq!(
            server_name("rustibia.online:5555").unwrap(),
            ServerName::try_from("rustibia.online").unwrap()
        );
        assert_eq!(
            server_name("127.0.0.1:5555").unwrap(),
            ServerName::try_from("127.0.0.1").unwrap()
        );
    }

    #[test]
    fn an_address_without_a_port_is_refused() {
        assert!(matches!(
            server_name("rustibia.online"),
            Err(TlsSetupError::Address(_))
        ));
    }

    #[test]
    fn an_unreadable_extra_ca_is_an_error_not_a_fallback() {
        assert!(matches!(
            connector(Some("/nonexistent/ca.crt")),
            Err(TlsSetupError::ExtraCa(..))
        ));
    }

    #[test]
    fn an_extra_ca_without_certificates_is_refused() {
        let path = std::env::temp_dir().join("rustibia-empty-extra-ca.crt");
        std::fs::write(&path, "").unwrap();

        let result = connector(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);

        assert!(matches!(result, Err(TlsSetupError::EmptyExtraCa(_))));
    }
}
