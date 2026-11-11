//! The ffmpeg ingest cuts fragments so that every keyframe starts a fragment. A
//! player switches renditions at a fragment start, and a decoder starts at a
//! keyframe only.

use std::path::PathBuf;
use std::time::Duration;

use bs_ingest::mp4::starts_with_keyframe;
use bs_ingest::{container, FfmpegSource, RecordType, RenditionSpec};

/// With a 1 s GOP and 250 ms fragments, fragments 0, 4 and 8 start with a keyframe
/// and no other fragment does.
#[tokio::test]
async fn every_fourth_fragment_starts_with_a_keyframe() {
    let sample =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/generated/sample.mp4");
    let ffmpeg = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output();
    if ffmpeg.is_err() || !sample.exists() {
        eprintln!("skipped: needs ffmpeg and scripts/make_testdata.sh");
        return;
    }
    let specs = RenditionSpec::parse_list("360p:800").unwrap();
    let mut src = FfmpegSource::spawn(sample, specs, Some(30)).await.unwrap();
    let mut keys = Vec::new();
    for _ in 0..9 {
        let chunk = tokio::time::timeout(Duration::from_secs(20), src.next_chunk())
            .await
            .expect("ffmpeg produced a chunk in time")
            .expect("the clip is longer than nine chunks");
        let media = container::decode(&chunk.layers[0])
            .into_iter()
            .find(|r| r.kind == RecordType::Media)
            .expect("a media fragment per chunk");
        // Track 1 is the video track of ffmpeg's mp4 muxer.
        keys.push(starts_with_keyframe(&media.data, 1));
    }
    let expected: Vec<Option<bool>> = (0..9).map(|i| Some(i % 4 == 0)).collect();
    assert_eq!(keys, expected);
}
