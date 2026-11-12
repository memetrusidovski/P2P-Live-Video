//! The public address of a UDP port, asked from a STUN server (RFC 8489 binding
//! request).
//!
//! A publisher behind a port forward knows its LAN address only. Peers verify its
//! frames against the address they observe, so the publisher must advertise the
//! public one. The lookup is opt-in: it sends one request to a server the user names.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use anyhow::Context;

const MAGIC_COOKIE: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const MAPPED_ADDRESS: u16 = 0x0001;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
/// Requests sent before the lookup fails.
const ATTEMPTS: u32 = 3;

/// A binding request with the given transaction id.
pub fn binding_request(txid: [u8; 12]) -> [u8; 20] {
    let mut m = [0u8; 20];
    m[..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    // Bytes 2..4 are the attribute length: zero.
    m[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    m[8..].copy_from_slice(&txid);
    m
}

/// The mapped address in a binding success response to the request `txid`.
///
/// Returns `None` if `msg` is not that response or carries no address.
pub fn mapped_address(msg: &[u8], txid: [u8; 12]) -> Option<SocketAddr> {
    if msg.len() < 20
        || u16::from_be_bytes([msg[0], msg[1]]) != BINDING_SUCCESS
        || msg[4..8] != MAGIC_COOKIE.to_be_bytes()
        || msg[8..20] != txid
    {
        return None;
    }
    let len = u16::from_be_bytes([msg[2], msg[3]]) as usize;
    let attrs = msg.get(20..20 + len)?;
    let mut plain = None;
    let mut o = 0;
    while o + 4 <= attrs.len() {
        let kind = u16::from_be_bytes([attrs[o], attrs[o + 1]]);
        let alen = u16::from_be_bytes([attrs[o + 2], attrs[o + 3]]) as usize;
        let value = attrs.get(o + 4..o + 4 + alen)?;
        match kind {
            XOR_MAPPED_ADDRESS => return address(value, Some(txid)),
            MAPPED_ADDRESS => plain = address(value, None),
            _ => {}
        }
        // Attributes are padded to four bytes.
        o += 4 + alen.div_ceil(4) * 4;
    }
    plain
}

/// Decode an address attribute. `xor` holds the transaction id of an XOR-mapped one.
fn address(value: &[u8], xor: Option<[u8; 12]>) -> Option<SocketAddr> {
    let family = *value.get(1)?;
    let mut port = u16::from_be_bytes([*value.get(2)?, *value.get(3)?]);
    let mut key = [0u8; 16];
    if let Some(txid) = xor {
        port ^= (MAGIC_COOKIE >> 16) as u16;
        key[..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        key[4..].copy_from_slice(&txid);
    }
    let ip = match family {
        0x01 => {
            let b = value.get(4..8)?;
            IpAddr::V4(Ipv4Addr::new(
                b[0] ^ key[0],
                b[1] ^ key[1],
                b[2] ^ key[2],
                b[3] ^ key[3],
            ))
        }
        0x02 => {
            let b = value.get(4..20)?;
            let mut a = [0u8; 16];
            for i in 0..16 {
                a[i] = b[i] ^ key[i];
            }
            IpAddr::V6(Ipv6Addr::from(a))
        }
        _ => return None,
    };
    Some(SocketAddr::new(ip, port))
}

/// Ask `server` which address it observes for the UDP port `local`.
///
/// Args:
///     local: Address to send from. Use the node's listen address, before the node
///         binds it, so the answer names the mapping of the node's own port.
///     server: STUN server as `host:port`.
///     timeout: Wait per attempt.
///
/// Returns:
///     The address and port the server observed.
///
/// Raises:
///     An error if `local` cannot be bound, the server does not resolve, or no
///     attempt gets an answer.
pub async fn public_addr(
    local: SocketAddr,
    server: &str,
    timeout: Duration,
) -> anyhow::Result<SocketAddr> {
    let sock = tokio::net::UdpSocket::bind(local)
        .await
        .with_context(|| format!("binding {local} for the STUN request"))?;
    let want_v4 = local.is_ipv4();
    let server_addr = tokio::net::lookup_host(server)
        .await
        .with_context(|| format!("resolving STUN server {server}"))?
        .find(|a| a.is_ipv4() == want_v4)
        .with_context(|| format!("STUN server {server} has no address of the listen family"))?;
    let txid: [u8; 12] = rand::random();
    let req = binding_request(txid);
    let mut buf = [0u8; 576];
    for _ in 0..ATTEMPTS {
        sock.send_to(&req, server_addr).await?;
        let deadline = tokio::time::Instant::now() + timeout;
        while let Ok(Ok((n, from))) =
            tokio::time::timeout_at(deadline, sock.recv_from(&mut buf)).await
        {
            if from == server_addr {
                if let Some(addr) = mapped_address(&buf[..n], txid) {
                    return Ok(addr);
                }
            }
        }
    }
    anyhow::bail!("STUN server {server} did not answer {ATTEMPTS} requests")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A binding success with an XOR-MAPPED-ADDRESS for `addr`, as a server builds it.
    fn success(txid: [u8; 12], addr: SocketAddr, software_first: bool) -> Vec<u8> {
        let mut attr = vec![0x00, 0x20, 0x00, 0x00, 0x00];
        let port = addr.port() ^ (MAGIC_COOKIE >> 16) as u16;
        let mut key = MAGIC_COOKIE.to_be_bytes().to_vec();
        key.extend_from_slice(&txid);
        match addr.ip() {
            IpAddr::V4(ip) => {
                attr.push(0x01);
                attr.extend_from_slice(&port.to_be_bytes());
                attr.extend(ip.octets().iter().zip(&key).map(|(a, k)| a ^ k));
            }
            IpAddr::V6(ip) => {
                attr.push(0x02);
                attr.extend_from_slice(&port.to_be_bytes());
                attr.extend(ip.octets().iter().zip(&key).map(|(a, k)| a ^ k));
            }
        }
        let alen = (attr.len() - 4) as u16;
        attr[2..4].copy_from_slice(&alen.to_be_bytes());
        let mut attrs = Vec::new();
        if software_first {
            // SOFTWARE (0x8022) with a five-byte value and three bytes of padding.
            attrs.extend_from_slice(&[
                0x80, 0x22, 0x00, 0x05, b't', b'e', b's', b't', b'!', 0, 0, 0,
            ]);
        }
        attrs.extend_from_slice(&attr);
        let mut m = Vec::new();
        m.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
        m.extend_from_slice(&(attrs.len() as u16).to_be_bytes());
        m.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        m.extend_from_slice(&txid);
        m.extend_from_slice(&attrs);
        m
    }

    #[test]
    fn request_layout() {
        let r = binding_request([7; 12]);
        assert_eq!(&r[..8], &[0x00, 0x01, 0x00, 0x00, 0x21, 0x12, 0xA4, 0x42]);
        assert_eq!(&r[8..], &[7; 12]);
    }

    #[test]
    fn xor_mapped_address_is_decoded() {
        let txid = [3; 12];
        // RFC 5769 §2.2 uses this address: the XOR-ed wire form is a1 47 e1 12 a6 43.
        let v4: SocketAddr = "192.0.2.1:32853".parse().unwrap();
        let msg = success(txid, v4, true);
        assert!(msg
            .windows(6)
            .any(|w| w == [0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43]));
        assert_eq!(mapped_address(&msg, txid), Some(v4));
        let v6: SocketAddr = "[2001:db8::7]:4000".parse().unwrap();
        assert_eq!(mapped_address(&success(txid, v6, false), txid), Some(v6));
        // Another transaction, a request, and a truncated message are not answers.
        assert_eq!(mapped_address(&msg, [4; 12]), None);
        assert_eq!(mapped_address(&binding_request(txid), txid), None);
        assert_eq!(mapped_address(&msg[..msg.len() - 3], txid), None);
    }

    #[tokio::test]
    async fn lookup_asks_the_server_and_returns_its_answer() {
        let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            // Drop the first request: the lookup must ask again.
            let _ = server.recv_from(&mut buf).await.unwrap();
            let (n, from) = server.recv_from(&mut buf).await.unwrap();
            assert_eq!(n, 20);
            let mut txid = [0u8; 12];
            txid.copy_from_slice(&buf[8..20]);
            server
                .send_to(&success(txid, from, false), from)
                .await
                .unwrap();
        });
        let got = public_addr(
            "127.0.0.1:0".parse().unwrap(),
            &server_addr.to_string(),
            Duration::from_millis(200),
        )
        .await
        .unwrap();
        assert!(got.ip().is_loopback() && got.port() != 0, "{got}");
    }

    #[tokio::test]
    async fn lookup_fails_when_nobody_answers() {
        let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let r = public_addr(
            "127.0.0.1:0".parse().unwrap(),
            &silent.local_addr().unwrap().to_string(),
            Duration::from_millis(50),
        )
        .await;
        assert!(r.is_err());
    }
}
