// src/morse/encoder.rs  —  Text → sequence of (tone_on, Duration)
use crate::morse::Timing;
use std::time::Duration;

pub type ToneSeq = Vec<(bool, Duration)>; // (key_down, duration)

/// ITU Morse code table
pub fn char_to_morse(c: char) -> Option<&'static str> {
    match c.to_ascii_uppercase() {
        'A' => Some(".-"),    'B' => Some("-..."),  'C' => Some("-.-."),
        'D' => Some("-.."),   'E' => Some("."),      'F' => Some("..-."),
        'G' => Some("--."),   'H' => Some("...."),   'I' => Some(".."),
        'J' => Some(".---"),  'K' => Some("-.-"),    'L' => Some(".-.."),
        'M' => Some("--"),    'N' => Some("-."),     'O' => Some("---"),
        'P' => Some(".--."),  'Q' => Some("--.-"),   'R' => Some(".-."),
        'S' => Some("..."),   'T' => Some("-"),      'U' => Some("..-"),
        'V' => Some("...-"),  'W' => Some(".--"),    'X' => Some("-..-"),
        'Y' => Some("-.--"),  'Z' => Some("--.."),
        '0' => Some("-----"), '1' => Some(".----"),  '2' => Some("..---"),
        '3' => Some("...--"), '4' => Some("....-"),  '5' => Some("....."),
        '6' => Some("-...."), '7' => Some("--..."),  '8' => Some("---.."),
        '9' => Some("----."),
        '.' => Some(".-.-.-"),',' => Some("--..--"), '?' => Some("..--.."),
        '/' => Some("-..-."), '+' => Some(".-.-."),  '=' => Some("-...-"),
        '-' => Some("-....-"),'@' => Some(".--.-."), '(' => Some("-.--."),
        ')' => Some("-.--.-"),'\'' => Some(".----."),
        // Prosigns stored as pseudo-chars
        // AR = end of transmission, SK = end of QSO, BK = break, KN = go only
        _   => None,
    }
}

/// Prosign: text like "<AR>" → dit/dah string without char gaps
pub fn prosign_to_morse(s: &str) -> Option<&'static str> {
    match s {
        "<AR>" | "+"   => Some(".-.-."),
        "<SK>"         => Some("...-.-"),
        "<KN>"         => Some("-.--."),
        "<BK>"         => Some("-...-.-"),
        "<SOS>"        => Some("...---..."),
        "<HH>"         => Some("........"), // error
        _              => None,
    }
}

/// Encode full text into a tone sequence
pub fn encode(text: &str, timing: &Timing) -> ToneSeq {
    let mut seq = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    for (wi, word) in words.iter().enumerate() {
        // Check prosign first
        if word.starts_with('<') && word.ends_with('>') {
            if let Some(code) = prosign_to_morse(word) {
                // Prosigns need inter-element gaps (1 dot between each dit/dah)
                // just like regular characters.  What makes them a *prosign* is the
                // absence of the 3-dot inter-character gap between their constituent
                // letters — that gap is simply never appended here.
                push_code(&mut seq, code, timing, true);
            }
        } else {
            let chars: Vec<char> = word.chars().collect();
            for (ci, &ch) in chars.iter().enumerate() {
                if let Some(code) = char_to_morse(ch) {
                    push_code(&mut seq, code, timing, true);
                    // inter-char gap (not after last char in word)
                    if ci + 1 < chars.len() {
                        seq.push((false, timing.char_gap));
                    }
                }
            }
        }
        // word gap (not after last word)
        if wi + 1 < words.len() {
            seq.push((false, timing.word_gap));
        }
    }
    seq
}

fn push_code(seq: &mut ToneSeq, code: &str, t: &Timing, inter_elem: bool) {
    let elems: Vec<char> = code.chars().collect();
    for (i, &el) in elems.iter().enumerate() {
        let dur = if el == '-' { t.dash } else { t.dot };
        seq.push((true, dur));
        if i + 1 < elems.len() {
            if inter_elem { seq.push((false, t.elem_gap)); }
        }
    }
}
