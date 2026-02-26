// src/morse/timing.rs  —  WPM → element durations (PARIS standard)
use std::time::Duration;

/// All timing derived from a single dot length
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub dot:        Duration,  // 1 unit
    pub dash:       Duration,  // 3 units
    pub elem_gap:   Duration,  // 1 unit  (between dits/dahs in same char)
    pub char_gap:   Duration,  // 3 units (between characters)
    pub word_gap:   Duration,  // 7 units (between words)
}

impl Timing {
    /// PARIS standard: dot = 1200 ms / wpm
    pub fn from_wpm(wpm: u8) -> Self {
        let wpm = wpm.max(1) as u64;
        let dot_ms = 1200 / wpm;
        Self {
            dot:      Duration::from_millis(dot_ms),
            dash:     Duration::from_millis(dot_ms * 3),
            elem_gap: Duration::from_millis(dot_ms),
            char_gap: Duration::from_millis(dot_ms * 3),
            word_gap: Duration::from_millis(dot_ms * 7),
        }
    }

    /// Farnsworth: characters at char_wpm, spacing at effective wpm
    pub fn farnsworth(char_wpm: u8, eff_wpm: u8) -> Self {
        let base = Self::from_wpm(char_wpm);
        let eff_dot_ms = 1200 / (eff_wpm.max(1) as u64);
        // Farnsworth adjustment to inter-char and word gaps
        let t = base.dot.as_millis() as u64;
        let extra_char = if eff_dot_ms * 3 > t * 3 { eff_dot_ms * 3 } else { t * 3 };
        let extra_word = if eff_dot_ms * 7 > t * 7 { eff_dot_ms * 7 } else { t * 7 };
        Self {
            char_gap: Duration::from_millis(extra_char),
            word_gap: Duration::from_millis(extra_word),
            ..base
        }
    }
}
