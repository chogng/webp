#![forbid(unsafe_code)]
//! Stateful, scalar composition of decoded WebP animation frames.

mod compositor;

pub use compositor::AnimationCanvas;
pub use compositor::DecodedFrame;
