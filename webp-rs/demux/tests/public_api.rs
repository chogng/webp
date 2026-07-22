use webp_demux::CompatibilityProfile;
use webp_demux::Container;
use webp_demux::ContainerErrorKind;
use webp_demux::ContainerLimits;
use webp_demux::DemuxOptions;
use webp_demux::Demuxer;
use webp_demux::ImageBitstream;
use webp_demux::VP8L;

fn riff(chunks: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    for (fourcc, payload) in chunks {
        body.extend_from_slice(fourcc);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut output = b"RIFF".to_vec();
    output.extend_from_slice(&(body.len() as u32).to_le_bytes());
    output.extend_from_slice(&body);
    output
}

#[test]
fn demuxer_exposes_stable_static_queries() {
    let bytes = riff(&[(*b"zZZ!", &[1, 2, 3]), (VP8L, &[9, 8])]);
    let demuxer = Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();
    let _: &Container<'_> = &demuxer;

    assert_eq!(demuxer.chunk_count(), 2);
    assert_eq!(demuxer.chunk(0).unwrap().fourcc, *b"zZZ!");
    assert_eq!(demuxer.chunks_with(VP8L).count(), 1);
    assert_eq!(demuxer.unknown_chunks().count(), 1);
    assert_eq!(
        demuxer.image().unwrap().bitstream(),
        ImageBitstream::Vp8l(&[9, 8])
    );
    assert_eq!(demuxer.image().unwrap().alpha(), None);
}

#[test]
fn free_parse_remains_available() {
    let bytes = riff(&[(VP8L, &[1, 2])]);
    let parsed = webp_demux::parse(
        &bytes,
        CompatibilityProfile::SpecStrict,
        &DemuxOptions::default().limits,
    )
    .unwrap();

    assert_eq!(parsed.chunk_count(), 1);
}

#[test]
fn options_bound_retained_chunks() {
    let bytes = riff(&[(VP8L, &[1, 2])]);
    let options = DemuxOptions {
        limits: ContainerLimits {
            max_chunks: 0,
            ..ContainerLimits::default()
        },
        ..DemuxOptions::default()
    };

    let error = Demuxer::parse(&bytes, &options).unwrap_err();

    assert_eq!(error.kind(), ContainerErrorKind::LimitExceeded);
}
