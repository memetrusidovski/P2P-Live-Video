//! The ffmpeg ingest cuts fragments so that every keyframe starts a fragment. A
//! player switches renditions at a fragment start, and a decoder starts at a
//! keyframe only.

use std::path::PathBuf;
use std::time::Duration;

use bs_ingest::mp4::starts_with_keyframe;
use bs_ingest::{container, FfmpegSource, RecordType, RenditionSpec};

/// With a 1 s GOP and 250 ms fragments, fragments 0, 4 and 8 start with a keyframe
/// and no other fragment does. One ffmpeg process feeds both renditions, so the
/// keyframes of the two renditions fall in the same chunks.
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
    let specs = RenditionSpec::parse_list("240p:400,360p:800").unwrap();
    let mut src = FfmpegSource::spawn(sample, specs, Some(30)).await.unwrap();
    let mut keys = Vec::new();
    for _ in 0..9 {
        let chunk = tokio::time::timeout(Duration::from_secs(20), src.next_chunk())
            .await
            .expect("ffmpeg produced a chunk in time")
            .expect("the clip is longer than nine chunks");
        assert_eq!(chunk.layers.len(), 2, "one layer per rendition");
        let key_of = |layer: usize| {
            let media = container::decode(&chunk.layers[layer])
                .into_iter()
                .find(|r| r.kind == RecordType::Media)
                .expect("a media fragment per rendition per chunk");
            // Track 1 is the video track of ffmpeg's mp4 muxer.
            starts_with_keyframe(&media.data, 1)
        };
        assert_eq!(key_of(0), key_of(1), "renditions are keyframe aligned");
        keys.push(key_of(0));
    }
    let expected: Vec<Option<bool>> = (0..9).map(|i| Some(i % 4 == 0)).collect();
    assert_eq!(keys, expected);
}

/// A process that cannot open its input is an error at spawn, not a silent end.
#[tokio::test]
async fn a_missing_input_fails_at_spawn() {
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("skipped: needs ffmpeg");
        return;
    }
    let specs = RenditionSpec::parse_list("240p:400").unwrap();
    let r = FfmpegSource::spawn("/nonexistent/clip.mp4".into(), specs, Some(30)).await;
    assert!(r.is_err(), "spawn reports an input ffmpeg cannot open");
}

/// The ladder carries each rendition's bitrate on the wire, not its video bitrate.
#[test]
fn ladder_counts_audio_and_container() {
    let specs = RenditionSpec::parse_list("240p:400,720p:2500").unwrap();
    let ladder = RenditionSpec::ladder(&specs);
    let rates: Vec<u32> = ladder.layers.iter().map(|l| l.bitrate_kbps).collect();
    // (400 + 96) and (2500 + 96), each plus 3 % rounded up.
    assert_eq!(rates, vec![511, 2674]);
    assert_eq!(specs[0].kbps, 400, "the video bitrate is unchanged");
}
