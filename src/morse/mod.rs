// src/morse/mod.rs  —  Encoder, Decoder, Timing
pub mod encoder;
pub mod decoder;
pub mod timing;

pub use encoder::{encode, ToneSeq};
pub use decoder::Decoder;
pub use timing::Timing;
