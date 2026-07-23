use webp_demux::CompatibilityProfile;
use webp_demux::Container;
use webp_demux::ContainerErrorKind;
use webp_demux::ContainerLimits;
use webp_demux::DemuxOptions;
use webp_demux::Demuxer;
use webp_demux::ImageBitstream;
use webp_demux::VP8L;

fn vp8l(width: u32, height: u32) -> Vec<u8> {
    let fields = (width - 1) | ((height - 1) << 14);
    let mut payload = vec![0x2f];
    payload.extend_from_slice(&fields.to_le_bytes());
    payload
}

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
    let payload = vp8l(2, 3);
    let bytes = riff(&[(*b"zZZ!", &[1, 2, 3]), (VP8L, &payload)]);
    let demuxer = Demuxer::parse(&bytes, &DemuxOptions::default()).unwrap();
    let _: &Container<'_> = &demuxer;

    assert_eq!(demuxer.chunk_count(), 2);
    assert_eq!(demuxer.chunk(0).unwrap().fourcc, *b"zZZ!");
    assert_eq!(demuxer.chunks_with(VP8L).count(), 1);
    assert_eq!(demuxer.unknown_chunks().count(), 1);
    assert_eq!(
        demuxer.image().unwrap().bitstream(),
        ImageBitstream::Vp8l(&payload)
    );
    assert_eq!(demuxer.image().unwrap().alpha(), None);
    assert_eq!(demuxer.canvas_dimensions(), Some((2, 3)));
    assert_eq!(demuxer.frame_count(), 1);
    assert!(!demuxer.is_animated());
    assert_eq!(demuxer.image().unwrap().width(), 2);
    assert_eq!(demuxer.image().unwrap().height(), 3);
}

#[test]
fn free_parse_remains_available() {
    let payload = vp8l(1, 1);
    let bytes = riff(&[(VP8L, &payload)]);
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
    let payload = vp8l(1, 1);
    let bytes = riff(&[(VP8L, &payload)]);
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
