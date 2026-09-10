//! Identity-bound TLS for QUIC sessions.
//!
//! Every node presents a self-signed Ed25519 certificate whose key **is** its
//! protocol identity key, with the NodeID and static nonce carried in a SAN URI
//! (`bitstream://<nodeid-hex>/<static-nonce>`). Peers verify no CA: they check
//! that `Blake3(PK ‖ nonce) == NodeID` and that the static proof-of-work holds.
//! Both sides authenticate (mutual TLS), so a session's `SessionOpened` input
//! carries a verified NodeId.

use std::sync::Arc;

use anyhow::{anyhow, Context};
use bs_crypto::pow::{self, Difficulty};
use bs_crypto::Identity;
use bs_wire::{NodeId, PublicKeyBytes};
use ed25519_dalek::pkcs8::EncodePrivateKey;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};

/// ALPN protocol id.
pub const ALPN: &[u8] = b"bitstream/1";
/// The server name clients present (unused for verification).
pub const SERVER_NAME: &str = "bitstream";

/// Build the node's certificate and key from its identity.
pub fn identity_cert(
    identity: &Identity,
) -> anyhow::Result<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    let sk = ed25519_dalek::SigningKey::from_bytes(&identity.secret_bytes());
    let pkcs8 = sk.to_pkcs8_der().map_err(|e| anyhow!("pkcs8: {e}"))?;
    let key_der = PrivatePkcs8KeyDer::from(pkcs8.as_bytes().to_vec());
    let kp = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&key_der, &rcgen::PKCS_ED25519)
        .context("rcgen keypair")?;
    let uri = format!(
        "bitstream://{}/{}",
        identity.node_id(),
        identity.static_nonce()
    );
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).context("cert params")?;
    params.subject_alt_names.push(rcgen::SanType::URI(
        rcgen::Ia5String::try_from(uri).context("san")?,
    ));
    params.distinguished_name = rcgen::DistinguishedName::new();
    let cert = params.self_signed(&kp).context("self sign")?;
    Ok((cert.der().clone(), PrivateKeyDer::Pkcs8(key_der)))
}

/// What a peer's certificate proves.
#[derive(Debug, Clone, Copy)]
pub struct PeerIdentity {
    /// NodeID.
    pub node_id: NodeId,
    /// Ed25519 key.
    pub public_key: PublicKeyBytes,
    /// Static nonce.
    pub static_nonce: u64,
}

/// Parse and verify a peer certificate: Ed25519 SPKI, SAN URI, PoW binding.
pub fn verify_peer_cert(der: &[u8], c1: u32) -> Result<PeerIdentity, rustls::Error> {
    let bad = |m: &str| rustls::Error::General(m.to_string());
    let (_, cert) = x509_parser::parse_x509_certificate(der).map_err(|_| bad("cert parse"))?;
    let spki = &cert.tbs_certificate.subject_pki;
    let key = spki.subject_public_key.data.as_ref();
    if key.len() != 32 {
        return Err(bad("expected an Ed25519 key"));
    }
    let public_key = PublicKeyBytes(key.try_into().unwrap());
    let san = cert
        .subject_alternative_name()
        .map_err(|_| bad("san"))?
        .ok_or_else(|| bad("no san"))?;
    let mut found = None;
    for name in &san.value.general_names {
        if let x509_parser::extensions::GeneralName::URI(u) = name {
            if let Some(rest) = u.strip_prefix("bitstream://") {
                let mut it = rest.split('/');
                let id_hex = it.next().unwrap_or("");
                let nonce = it
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
                    .ok_or_else(|| bad("san nonce"))?;
                let id = hex::decode(id_hex).map_err(|_| bad("san id"))?;
                let id: [u8; 32] = id.try_into().map_err(|_| bad("san id len"))?;
                found = Some((NodeId(id), nonce));
            }
        }
    }
    let (node_id, static_nonce) = found.ok_or_else(|| bad("no bitstream san"))?;
    pow::verify_static(&public_key, static_nonce, &node_id, c1).map_err(|_| bad("identity pow"))?;
    Ok(PeerIdentity {
        node_id,
        public_key,
        static_nonce,
    })
}

/// Verifier used on both sides: identity binding instead of a CA.
#[derive(Debug)]
pub struct PeerVerifier {
    c1: u32,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl PeerVerifier {
    /// New verifier for static difficulty `c1`.
    pub fn new(c1: u32, provider: Arc<rustls::crypto::CryptoProvider>) -> Self {
        Self { c1, provider }
    }
}

impl ServerCertVerifier for PeerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        verify_peer_cert(end_entity.as_ref(), self.c1)?;
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            m,
            c,
            d,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            m,
            c,
            d,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

impl ClientCertVerifier for PeerVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        verify_peer_cert(end_entity.as_ref(), self.c1)?;
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            m,
            c,
            d,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            m,
            c,
            d,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// quinn client and server configs for this identity.
pub fn quic_configs(
    identity: &Identity,
    difficulty: Difficulty,
) -> anyhow::Result<(quinn::ClientConfig, quinn::ServerConfig)> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let (cert, key) = identity_cert(identity)?;
    let verifier = Arc::new(PeerVerifier::new(difficulty.c1, provider.clone()));

    let mut client = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .context("tls13")?
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_client_auth_cert(vec![cert.clone()], key.clone_key())
        .context("client cert")?;
    client.alpn_protocols = vec![ALPN.to_vec()];
    let mut server = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .context("tls13")?
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert], key)
        .context("server cert")?;
    server.alpn_protocols = vec![ALPN.to_vec()];

    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(4)).unwrap(),
    ));
    transport.keep_alive_interval(Some(std::time::Duration::from_millis(1500)));
    transport.datagram_receive_buffer_size(Some(8 << 20));
    transport.datagram_send_buffer_size(8 << 20);
    transport.max_concurrent_bidi_streams(64u32.into());
    let transport = Arc::new(transport);

    let qc =
        quinn::crypto::rustls::QuicClientConfig::try_from(client).context("quic client config")?;
    let mut client_cfg = quinn::ClientConfig::new(Arc::new(qc));
    client_cfg.transport_config(transport.clone());
    let qs =
        quinn::crypto::rustls::QuicServerConfig::try_from(server).context("quic server config")?;
    let mut server_cfg = quinn::ServerConfig::with_crypto(Arc::new(qs));
    server_cfg.transport_config(transport);
    Ok((client_cfg, server_cfg))
}

/// Extract the verified peer identity from an established connection.
pub fn connection_peer(conn: &quinn::Connection, c1: u32) -> anyhow::Result<PeerIdentity> {
    let id = conn
        .peer_identity()
        .ok_or_else(|| anyhow!("no peer identity"))?;
    let certs = id
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| anyhow!("unexpected identity type"))?;
    let first = certs.first().ok_or_else(|| anyhow!("empty cert chain"))?;
    verify_peer_cert(first.as_ref(), c1).map_err(|e| anyhow!("peer cert: {e}"))
}
