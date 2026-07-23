#[derive(Debug)]
pub(crate) struct Fixture {
    pub(crate) name: String,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn generate() -> Vec<Fixture> {
    let mut fixtures = malformed_fixtures()
        .into_iter()
        .map(|(name, bytes)| Fixture {
            name: format!("{name}.webp"),
            bytes,
        })
        .collect::<Vec<_>>();
    fixtures.extend(metadata_fixtures());
    fixtures.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    fixtures
}

fn metadata_fixtures() -> Vec<Fixture> {
    const LENGTHS: [usize; 11] = [0, 1, 2, 3, 4, 7, 8, 15, 16, 255, 256];
    const MINIMAL_VP8_KEY_FRAME: [u8; 10] =
        [0x10, 0x00, 0x00, 0x9d, 0x01, 0x2a, 0x01, 0x00, 0x01, 0x00];

    let mut fixtures = Vec::new();
    for mask in 0_u8..8 {
        for length in LENGTHS {
            if mask == 0 && length != 0 {
                continue;
            }
            let placements = if mask & 0b110 == 0 {
                &["before"][..]
            } else {
                &["before", "after"][..]
            };
            for placement in placements {
                let payload = (0..length)
                    .map(|index| (index as u8).wrapping_add(mask))
                    .collect::<Vec<_>>();
                let flags = (if mask & 1 != 0 { 1 << 5 } else { 0 })
                    | (if mask & 2 != 0 { 1 << 3 } else { 0 })
                    | (if mask & 4 != 0 { 1 << 2 } else { 0 });
                let mut chunks = vec![chunk(*b"VP8X", &[flags, 0, 0, 0, 0, 0, 0, 0, 0, 0], None)];
                if mask & 1 != 0 {
                    chunks.push(chunk(*b"ICCP", &payload, None));
                }
                let mut positionable = Vec::new();
                if mask & 2 != 0 {
                    positionable.push(chunk(*b"EXIF", &payload, None));
                }
                if mask & 4 != 0 {
                    positionable.push(chunk(*b"XMP ", &payload, None));
                }
                let image = chunk(*b"VP8 ", &MINIMAL_VP8_KEY_FRAME, None);
                if *placement == "before" {
                    chunks.extend(positionable);
                    chunks.push(image);
                } else {
                    chunks.push(image);
                    chunks.extend(positionable);
                }
                fixtures.push(Fixture {
                    name: format!("metadata-{mask:01x}-{length:03}-{placement}.webp"),
                    bytes: riff_body(chunks.concat()),
                });
            }
        }
    }
    fixtures
}

fn malformed_fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let valid_vp8 = riff_body(chunk(*b"VP8 ", &[0x00, 0x00], None));
    let mut trailing = valid_vp8;
    trailing.push(0xff);

    let truncated_chunk = riff_body({
        let mut body = b"VP8 ".to_vec();
        body.extend_from_slice(&1_u32.to_le_bytes());
        body
    });
    let vp8x = chunk(*b"VP8X", &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);
    let duplicate_vp8x = riff_body([vp8x.clone(), vp8x].concat());
    let animation_vp8x = chunk(*b"VP8X", &[0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);
    let truncated_anmf = riff_body({
        let mut body = animation_vp8x.clone();
        body.extend_from_slice(b"ANMF");
        body.extend_from_slice(&16_u32.to_le_bytes());
        body.extend_from_slice(&[0; 10]);
        body
    });
    let exif_vp8x = chunk(*b"VP8X", &[1 << 3, 0, 0, 0, 0, 0, 0, 0, 0, 0], None);

    vec![
        ("riff-declared-size-too-large", {
            let mut bytes = b"RIFF".to_vec();
            bytes.extend_from_slice(&u32::MAX.to_le_bytes());
            bytes.extend_from_slice(b"WEBP");
            bytes
        }),
        ("chunk-payload-truncated", truncated_chunk),
        ("riff-trailing-byte", trailing),
        (
            "non-zero-padding",
            riff_body(chunk(*b"VP8 ", &[0x00], Some(0xff))),
        ),
        (
            "vp8x-reserved-bit",
            riff_body(chunk(*b"VP8X", &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0], None)),
        ),
        ("duplicate-vp8x", duplicate_vp8x),
        ("animation-anmf-payload-truncated", truncated_anmf),
        (
            "animation-anmf-non-zero-padding",
            riff_body([animation_vp8x, chunk(*b"ANMF", &[0x00], Some(0xff))].concat()),
        ),
        (
            "duplicate-exif",
            riff_body(
                [
                    exif_vp8x.clone(),
                    chunk(*b"EXIF", &[0x01], None),
                    chunk(*b"EXIF", &[0x02], None),
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                ]
                .concat(),
            ),
        ),
        (
            "metadata-without-vp8x",
            riff_body(
                [
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                    chunk(*b"EXIF", &[0x01], None),
                ]
                .concat(),
            ),
        ),
        (
            "vp8x-exif-flag-missing",
            riff_body(
                [
                    chunk(*b"VP8X", &[0; 10], None),
                    chunk(*b"EXIF", &[0x01], None),
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                ]
                .concat(),
            ),
        ),
        (
            "vp8x-exif-flag-without-chunk",
            riff_body([exif_vp8x.clone(), chunk(*b"VP8 ", &[0x00, 0x00], None)].concat()),
        ),
        (
            "vp8x-not-first",
            riff_body([chunk(*b"VP8 ", &[0x00, 0x00], None), exif_vp8x].concat()),
        ),
        (
            "mixed-vp8-vp8l",
            riff_body(
                [
                    chunk(*b"VP8 ", &[0x00, 0x00], None),
                    chunk(*b"VP8L", &[0x2f, 0x00, 0x00, 0x00, 0x00], None),
                ]
                .concat(),
            ),
        ),
    ]
}

fn riff_body(body: Vec<u8>) -> Vec<u8> {
    let mut body_with_form_type = b"WEBP".to_vec();
    body_with_form_type.extend_from_slice(&body);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(
        &u32::try_from(body_with_form_type.len())
            .expect("generated RIFF body length fits u32")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&body_with_form_type);
    bytes
}

fn chunk(fourcc: [u8; 4], payload: &[u8], padding: Option<u8>) -> Vec<u8> {
    let mut bytes = fourcc.to_vec();
    bytes.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("generated chunk payload length fits u32")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        bytes.push(padding.unwrap_or(0));
    }
    bytes
}

#[cfg(test)]
#[path = "fixture_set_tests.rs"]
mod tests;
