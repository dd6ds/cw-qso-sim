// src/morse/decoder.rs  —  Paddle timings → characters (Farnsworth aware)
use crate::morse::Timing;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaddleEvent {
    DitDown, DitUp,
    DahDown, DahUp,
    None,
}

/// State machine: accumulated element string → character lookup
pub struct Decoder {
    current_code:    String,
    decoded_text:    String,
    last_event:      Instant,
    last_char_flush: Option<Instant>,  // when the last char was flushed
    timing:          Timing,
}

impl Decoder {
    pub fn new(timing: Timing) -> Self {
        Self {
            current_code:    String::new(),
            decoded_text:    String::new(),
            last_event:      Instant::now(),
            last_char_flush: None,
            timing,
        }
    }

    pub fn update_timing(&mut self, t: Timing) { self.timing = t; }

    /// Call this when a paddle element fires.
    /// `el_dur` is the element duration (dot or dash length, WITHOUT inter-element gap).
    /// Setting last_event to NOW + el_dur ensures the char_gap is measured from
    /// the END of the element, not the start — otherwise a dah triggers a premature
    /// char flush because char_gap == dah_duration.
    pub fn push_element(&mut self, is_dash: bool, el_dur: Duration) {
        if is_dash { self.current_code.push('-'); }
        else       { self.current_code.push('.'); }
        // Advance last_event to the projected end of this element so that
        // char_gap / word_gap are measured from when the element finishes.
        self.last_event = Instant::now() + el_dur;
    }

    /// Call every ~10ms loop tick; returns newly completed chars
    pub fn tick(&mut self) -> Option<String> {
        // Check for word gap even when current_code is empty —
        // the last char was already flushed at char_gap, but we still
        // need to emit the space once word_gap elapses.
        if self.current_code.is_empty() {
            if let Some(flushed_at) = self.last_char_flush {
                if flushed_at.elapsed() >= self.timing.word_gap {
                    self.last_char_flush = None;
                    self.decoded_text.push(' ');
                    log::debug!("[decoder] word_gap → space");
                    return Some(" ".to_string());
                }
            }
            return None;
        }

        let elapsed = self.last_event.elapsed();

        if elapsed >= self.timing.word_gap {
            log::debug!("[decoder] word_gap elapsed={:?} code='{}' → flush+space", elapsed, self.current_code);
            let c = self.flush_char();
            self.last_char_flush = None;
            self.decoded_text.push(' ');
            return c.map(|ch| format!("{ch} "));
        }
        if elapsed >= self.timing.char_gap {
            log::debug!("[decoder] char_gap elapsed={:?} code='{}' → flush", elapsed, self.current_code);
            let c = self.flush_char();
            self.last_char_flush = Some(Instant::now());
            return c.map(|ch| ch.to_string());
        }
        None
    }

    fn flush_char(&mut self) -> Option<char> {
        let code = std::mem::take(&mut self.current_code);
        decode_code(&code)
    }

    pub fn decoded_text(&self) -> &str { &self.decoded_text }
    pub fn current_code(&self) -> &str { &self.current_code }
}

fn decode_code(code: &str) -> Option<char> {
    // Reverse lookup from encoder table
    let table = [
        (".-",    'A'), ("-...",  'B'), ("-.-.",  'C'), ("-..",   'D'),
        (".",     'E'), ("..-.",  'F'), ("--.",   'G'), ("....",  'H'),
        ("..",    'I'), (".---",  'J'), ("-.-",   'K'), (".-..",  'L'),
        ("--",    'M'), ("-.",    'N'), ("---",   'O'), (".--.",  'P'),
        ("--.-",  'Q'), (".-.",   'R'), ("...",   'S'), ("-",     'T'),
        ("..-",   'U'), ("...-",  'V'), (".--",   'W'), ("-..-",  'X'),
        ("-.--",  'Y'), ("--..",  'Z'),
        ("-----", '0'), (".----", '1'), ("..---", '2'), ("...--", '3'),
        ("....-", '4'), (".....", '5'), ("-....", '6'), ("--...", '7'),
        ("---..", '8'), ("----.", '9'),
        (".-.-.-",'.'), ("--..--",','), ("..--..",'?'), ("-..-.",'/'),
        (".----.",'\''),(  "-.--.", ')'), ("-.--.",'('),
        ("...-.-", ' '), // SK → word-space placeholder
    ];
    table.iter().find(|(c, _)| *c == code).map(|(_, ch)| *ch)
}
