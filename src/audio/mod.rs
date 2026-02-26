// src/audio/mod.rs  —  AudioOutput trait + cpal backend
use anyhow::Result;
use crate::morse::ToneSeq;

/// Platform-agnostic audio output interface
pub trait AudioOutput: Send {
    /// Play a precomputed tone sequence (blocking)
    fn play_sequence(&mut self, seq: &ToneSeq) -> Result<()>;
    /// Start a continuous tone (for sidetone monitor)
    fn tone_on(&mut self)  -> Result<()>;
    /// Stop a continuous tone
    fn tone_off(&mut self) -> Result<()>;
    fn set_frequency(&mut self, hz: f32);
    fn set_volume(&mut self, vol: f32);
}

// ── cpal backend ─────────────────────────────────────────────────────────────
#[cfg(feature = "audio-cpal")]
mod cpal_backend;
#[cfg(feature = "audio-cpal")]
pub use cpal_backend::CpalAudio;

/// Null backend (no sound — useful for testing / no-audio builds)
pub struct NullAudio;
impl AudioOutput for NullAudio {
    fn play_sequence(&mut self, seq: &ToneSeq) -> Result<()> {
        // Just sleep through the sequence so timing feels real
        for (_, dur) in seq { std::thread::sleep(*dur); }
        Ok(())
    }
    fn tone_on(&mut self)  -> Result<()> { Ok(()) }
    fn tone_off(&mut self) -> Result<()> { Ok(()) }
    fn set_frequency(&mut self, _hz: f32)  {}
    fn set_volume(&mut self,    _vol: f32) {}
}

/// Factory: returns the best available backend
pub fn create_audio(hz: f32, volume: f32) -> Box<dyn AudioOutput> {
    #[cfg(feature = "audio-cpal")]
    {
        match CpalAudio::new(hz, volume) {
            Ok(a)  => return Box::new(a),
            Err(e) => log::warn!("cpal init failed: {e}  →  using NullAudio"),
        }
    }
    Box::new(NullAudio)
}
