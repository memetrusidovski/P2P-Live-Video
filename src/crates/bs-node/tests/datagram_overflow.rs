//! A connection whose datagram send buffer overflows drops the oldest datagrams and
//! stays up.
//!
//! quinn-proto 0.11.17 subtracted a dropped datagram's bytes twice and then panicked
//! on its empty queue. The panic poisoned the connection's lock and aborted the node:
//! a publisher behind a slow or lossy uplink died a few seconds into the stream.

use std::net::SocketAddr;
use std::sync::Arc;

use bs_crypto::{Difficulty, Identity};
use bs_node::tls;
use bytes::Bytes;

fn endpoint(seed: u8) -> quinn::Endpoint {
    let difficulty = Difficulty::TEST;
    let identity = Identity::from_seed([seed; 32], difficulty);
    let (client, server) = tls::quic_configs(&identity, difficulty).unwrap();
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut ep = quinn::Endpoint::new(
        quinn::EndpointConfig::default(),
        Some(server),
        sock,
        Arc::new(quinn::TokioRuntime),
    )
    .unwrap();
    ep.set_default_client_config(client);
    ep
}

#[tokio::test]
async fn an_overflowing_datagram_buffer_drops_the_oldest_and_keeps_the_connection() {
    let server = endpoint(1);
    let client = endpoint(2);
    let server_addr: SocketAddr = server.local_addr().unwrap();
    let accept = tokio::spawn(async move { server.accept().await.unwrap().await.unwrap() });
    let conn = client
        .connect(server_addr, tls::SERVER_NAME)
        .unwrap()
        .await
        .unwrap();
    let peer = accept.await.unwrap();

    // Queue six times the buffer in one go: nothing can leave between two sends.
    let payload = Bytes::from(vec![7u8; 1000]);
    let count = 6 * tls::DATAGRAM_SEND_BUFFER / payload.len();
    for _ in 0..count {
        conn.send_datagram(payload.clone())
            .expect("a full buffer drops the oldest datagram, it does not fail");
    }
    // Let the queue drain. The broken version counted each dropped datagram twice,
    // so its byte counter fell below zero here.
    let quiet = std::time::Duration::from_millis(300);
    let mut drained = 0;
    while let Ok(Ok(_)) = tokio::time::timeout(quiet, peer.read_datagram()).await {
        drained += 1;
    }
    assert!(drained > 0, "datagrams arrived after the overflow");
    // A second overflow on the drained queue is where the broken version panicked.
    for _ in 0..count {
        conn.send_datagram(payload.clone())
            .expect("the buffer still takes datagrams after a drain");
    }
    let got = tokio::time::timeout(std::time::Duration::from_secs(10), peer.read_datagram())
        .await
        .expect("a datagram arrives after the second overflow")
        .expect("the connection is open");
    assert_eq!(got.len(), payload.len());
    assert!(
        conn.close_reason().is_none(),
        "the sender's connection is open"
    );
    assert!(
        peer.close_reason().is_none(),
        "the receiver's connection is open"
    );
}
