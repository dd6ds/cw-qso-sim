// src/keyer/keyboard.rs  —  Keyboard fallback (no hardware needed)
//
// The keyboard keyer is a no-op stub.  All key events are read in the
// main loop (single crossterm reader) and injected directly into the
// tx_key channel, bypassing this keyer thread entirely.
// The keyer thread still runs but always gets PaddleEvent::None from here —
// that's fine; hardware adapters (VBand, ATtiny85) use the same thread and
// need it running.

use crate::morse::decoder::PaddleEvent;
use super::KeyerInput;

pub struct KeyboardKeyer;

impl KeyboardKeyer {
    pub fn new() -> Self { Self }
}

impl KeyerInput for KeyboardKeyer {
    fn name(&self) -> &str { "Keyboard" }
    fn poll(&mut self) -> PaddleEvent { PaddleEvent::None }
}
