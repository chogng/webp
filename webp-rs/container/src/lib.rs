#![forbid(unsafe_code)]
//! Public surface for safe, zero-copy WebP RIFF container parsing.

mod container;

pub use container::{
    ALPH, ANIM, ANMF, Animation, AnimationFrame, Chunk, Container, EXIF, FourCc, FrameBitstream,
    ICCP, Metadata, VP8, VP8L, VP8X, Vp8x, Vp8xFlags, XMP, parse,
};
