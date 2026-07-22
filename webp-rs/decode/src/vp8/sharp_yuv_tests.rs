use crate::vp8::rgba_to_yuv420;

#[test]
fn sharp_sampler_preserves_neutral_chroma_and_macroblock_edges() {
    let rgba = [96, 96, 96, 0, 96, 96, 96, 255, 96, 96, 96, 12];
    let yuv = rgba_to_yuv420(3, 1, &rgba).expect("convert neutral RGB through sharp sampler");
    assert_eq!((yuv.y_stride, yuv.uv_stride), (16, 8));
    assert!(yuv.u.iter().all(|&sample| sample == 128));
    assert!(yuv.v.iter().all(|&sample| sample == 128));
    assert_eq!(yuv.y[2], yuv.y[3], "right edge is replicated");
    assert_eq!(yuv.y[0], yuv.y[16], "bottom edge is replicated");
}

#[test]
fn sharp_sampler_matches_pinned_libsharpyuv_on_a_chroma_edge() {
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255,
        0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 0, 255, 255, 255,
    ];
    let sharp = rgba_to_yuv420(3, 3, &rgba).expect("convert sharp chroma edge");
    assert_eq!(&sharp.y[..3], &[97, 189, 94]);
    assert_eq!(&sharp.y[16..19], &[190, 55, 229]);
    assert_eq!(&sharp.y[32..35], &[75, 235, 169]);
    assert_eq!(&sharp.u[..2], &[126, 0]);
    assert_eq!(&sharp.u[8..10], &[121, 229]);
    assert_eq!(&sharp.v[..2], &[80, 249]);
    assert_eq!(&sharp.v[8..10], &[172, 0]);
}
