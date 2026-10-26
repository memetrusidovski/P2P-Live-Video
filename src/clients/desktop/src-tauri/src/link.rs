//! The `bs://` share link: the one string a viewer needs to join a stream.
//!
//! Format: `bs://<host>:<port>/<stream-id-hex>`. `host:port` is a bootstrap
//! contact, normally the publisher's public UDP address. The stream id is the
//! 64-character hex form of `bs_wire::StreamId`. The protocol defines no link
//! format; the format belongs to the desktop client.

use std::net::SocketAddr;

use bs_wire::StreamId;

/// Scheme prefix of a share link.
pub const SCHEME: &str = "bs://";

/// A parsed share link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareLink {
    /// Bootstrap contact as written, `host:port`. The host is not resolved.
    pub bootstrap: String,
    /// Stream to join.
    pub stream_id: StreamId,
}

impl ShareLink {
    /// Parse a share link.
    ///
    /// Returns an error message that names the first problem found.
    pub fn parse(text: &str) -> Result<Self, String> {
        let rest = text
            .trim()
            .strip_prefix(SCHEME)
            .ok_or_else(|| format!("a share link starts with {SCHEME}"))?;
        let (host, hex_id) = rest
            .split_once('/')
            .ok_or("a share link needs /<stream-id> after host:port")?;
        let hex_id = hex_id.trim_end_matches('/');
        let port_ok = host
            .rsplit_once(':')
            .is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok());
        if !port_ok {
            return Err(format!("the bootstrap must be host:port, got {host:?}"));
        }
        Ok(Self {
            bootstrap: host.to_string(),
            stream_id: hex_id.parse().map_err(|e| format!("stream id: {e}"))?,
        })
    }

    /// Build the link for a stream reachable through `bootstrap`.
    pub fn format(bootstrap: SocketAddr, stream_id: StreamId) -> String {
        format!("{SCHEME}{bootstrap}/{stream_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> StreamId {
        StreamId([0xab; 32])
    }

    #[test]
    fn round_trip() {
        let addr: SocketAddr = "203.0.113.9:4000".parse().unwrap();
        let text = ShareLink::format(addr, id());
        assert_eq!(text, format!("bs://203.0.113.9:4000/{}", "ab".repeat(32)));
        let parsed = ShareLink::parse(&text).unwrap();
        assert_eq!(parsed.bootstrap, "203.0.113.9:4000");
        assert_eq!(parsed.stream_id, id());
    }

    #[test]
    fn accepts_hostnames_whitespace_and_trailing_slash() {
        let text = format!("  bs://example.net:4000/{}/ \n", "ab".repeat(32));
        let parsed = ShareLink::parse(&text).unwrap();
        assert_eq!(parsed.bootstrap, "example.net:4000");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(ShareLink::parse("http://x:1/aa").is_err());
        assert!(ShareLink::parse("bs://x:1").is_err());
        assert!(ShareLink::parse("bs://x/aa").is_err());
        assert!(ShareLink::parse("bs://x:notaport/aa").is_err());
        assert!(ShareLink::parse(&format!("bs://x:1/{}", "ab".repeat(31))).is_err());
        assert!(ShareLink::parse(&format!("bs://x:1/{}", "zz".repeat(32))).is_err());
    }
}
