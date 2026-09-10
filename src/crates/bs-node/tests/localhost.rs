//! Real-network integration test: one publisher and two viewers over QUIC on
//! localhost stream a file byte-exact (M2 exit criterion, single host).

use std::net::SocketAddr;
use std::path::PathBuf;

use bs_core::{Params, Role};
use bs_media::Ladder;
use bs_node::{start, Media, RunConfig};
use bs_wire::NodeClass;

fn temp_file(name: &str, bytes: usize, seed: u64) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bs-node-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    // Deterministic pseudo-random content.
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut v = Vec::with_capacity(bytes);
    while v.len() < bytes {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let b = x.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes();
        let take = (bytes - v.len()).min(8);
        v.extend_from_slice(&b[..take]);
    }
    std::fs::write(&p, &v).unwrap();
    p
}

/// Ch4 end to end over quinn: every viewer's output hashes equal to the source.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn publisher_and_two_viewers_stream_a_file_byte_exact() {
    let _ = tracing_subscriber::fmt().with_env_filter("warn").try_init();
    // 1.5 MB at 6 Mbps = 2 s of stream (8 chunks), plus the 3 s playout buffer.
    let file = temp_file("source.bin", 1_500_000, 7);
    let expect = bs_node::file_hash(&file, None).unwrap();
    let params = Params::for_simulation();

    let publisher = start(RunConfig {
        role: Role::Publisher {
            ladder: Ladder::reference(),
            forest_size: 3,
        },
        node_class: NodeClass::Relay,
        listen: "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        advertise_ip: None,
        bootstrap: None,
        stream_id: None,
        upload_kbps: 100_000,
        ladder: Ladder::reference(),
        media: Media::File {
            path: file.clone(),
            repeat: false,
            max_bytes: None,
        },
        params: params.clone(),
        seed: 1,
        duration_s: Some(30.0),
        start_delay_s: 2.0,
        result_path: None,
    })
    .await
    .expect("publisher starts");
    let bootstrap = publisher.local_addr;
    let stream_id = publisher.stream_id;

    let mut viewers = Vec::new();
    for (i, class) in [(1u64, NodeClass::Relay), (2, NodeClass::Leaf)] {
        let out = file.with_file_name(format!("viewer{i}.bin"));
        let h = start(RunConfig {
            role: Role::Viewer { top_layer: 2 },
            node_class: class,
            listen: "127.0.0.1:0".parse().unwrap(),
            advertise_ip: None,
            bootstrap: Some(bootstrap),
            stream_id: Some(stream_id),
            upload_kbps: 20_000,
            ladder: Ladder::reference(),
            media: Media::Output {
                path: Some(out.clone()),
                expect_hash: Some(expect.clone()),
            },
            params: params.clone(),
            seed: 10 + i,
            duration_s: Some(25.0),
            start_delay_s: 0.0,
            result_path: None,
        })
        .await
        .expect("viewer starts");
        viewers.push((h, out));
    }

    let source = std::fs::read(&file).unwrap();
    for (h, out) in viewers {
        let report = h.done.await.unwrap().unwrap();
        assert_eq!(
            report.state, "Terminated",
            "viewer did not see STREAM_END: {report:?}"
        );
        assert_eq!(report.hash_match, Some(true), "hash mismatch: {report:?}");
        assert_eq!(report.steady_starved, 0, "starvation: {report:?}");
        assert_eq!(
            report.first_segment,
            Some(1),
            "missed the start: {report:?}"
        );
        assert_eq!(std::fs::read(&out).unwrap(), source, "output bytes differ");
    }
    let pub_report = publisher.done.await.unwrap().unwrap();
    assert_eq!(pub_report.chunks_published, 8);
    assert_eq!(pub_report.guardian_peers, Some(3));
}
