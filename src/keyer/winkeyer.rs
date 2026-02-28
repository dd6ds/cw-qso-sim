// src/keyer/winkeyer.rs  —  K1EL WinKeyer USB/Serial adapter (WK2 / WK3)
//
// OVERVIEW
// ────────
// WinKeyer is opened in *host mode* with paddle echoback enabled.
// The device decodes whatever the operator keys on the physical paddle and
// echoes each decoded character back to the host as plain ASCII.
//
// We receive those characters in a background thread, convert each one to its
// Morse dit/dah pattern, and schedule the resulting synthetic PaddleEvents on
// a time-stamped queue.  poll() drains the queue and feeds the existing
// Decoder pipeline in main.rs — no changes to main.rs needed.
//
// SERIAL SETTINGS  (confirmed from K1EL datasheet & PyWinKeyerSerial)
//   1200 baud · 8 data bits · No parity · 2 stop bits · DTR asserted
//
// HOST MODE PROTOCOL
//   Admin Close  →  [0x00, 0x03]        reset any previous session
//   Admin Open   →  [0x00, 0x02]        enable host mode
//   Response     ←  1 byte (firmware version)
//   Set Mode     →  [0x0E, mode_byte]
//
// MODE BYTE (MSB = bit 7)
//   bit 7: disable_paddle_watchdog  = 0
//   bit 6: paddle_echoback          = 1  ← required
//   bit 5: keyer_mode MSB  ┐  00=IambicB  01=IambicA
//   bit 4: keyer_mode LSB  ┘
//   bit 3: paddle_swap              = switch_paddle flag
//   bit 2: serial_echoback          = 0  (we only want paddle echo)
//   bit 1: auto_space               = 0
//   bit 0: ct_spacing               = 0
//
// RESPONSE BYTE CLASSIFICATION
//   (byte & 0xC0) == 0xC0  →  WK status byte     — ignored
//   (byte & 0xC0) == 0x80  →  Speed-pot byte     — ignored
//   else                   →  Decoded paddle echo (ASCII character)

use anyhow::{anyhow, Result};
use serialport::SerialPort;
use crate::morse::decoder::PaddleEvent;
use super::KeyerInput;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use std::thread;

const BAUD_RATE: u32 = 1_200;

// ── Morse code table ─────────────────────────────────────────────────────────
// Returns a slice of booleans: false = dit, true = dah.
// Unknown characters return an empty slice (skipped silently).
fn morse_pattern(ch: char) -> &'static [bool] {
    match ch.to_ascii_uppercase() {
        'A' => &[false, true],
        'B' => &[true,  false, false, false],
        'C' => &[true,  false, true,  false],
        'D' => &[true,  false, false],
        'E' => &[false],
        'F' => &[false, false, true,  false],
        'G' => &[true,  true,  false],
        'H' => &[false, false, false, false],
        'I' => &[false, false],
        'J' => &[false, true,  true,  true],
        'K' => &[true,  false, true],
        'L' => &[false, true,  false, false],
        'M' => &[true,  true],
        'N' => &[true,  false],
        'O' => &[true,  true,  true],
        'P' => &[false, true,  true,  false],
        'Q' => &[true,  true,  false, true],
        'R' => &[false, true,  false],
        'S' => &[false, false, false],
        'T' => &[true],
        'U' => &[false, false, true],
        'V' => &[false, false, false, true],
        'W' => &[false, true,  true],
        'X' => &[true,  false, false, true],
        'Y' => &[true,  false, true,  true],
        'Z' => &[true,  true,  false, false],
        '0' => &[true,  true,  true,  true,  true],
        '1' => &[false, true,  true,  true,  true],
        '2' => &[false, false, true,  true,  true],
        '3' => &[false, false, false, true,  true],
        '4' => &[false, false, false, false, true],
        '5' => &[false, false, false, false, false],
        '6' => &[true,  false, false, false, false],
        '7' => &[true,  true,  false, false, false],
        '8' => &[true,  true,  true,  false, false],
        '9' => &[true,  true,  true,  true,  false],
        '?' => &[false, false, true,  true,  false, false],
        '/' => &[true,  false, false, true,  false],
        // ── Prosign characters echoed by WinKeyer firmware ──────────────────
        // WK2/WK3 paddle echoback sends these ASCII bytes when the operator
        // keys the corresponding prosign on the physical paddle.
        //
        //   AR  (.-.-.)  → echoed as '+' (WK standard)
        //   BT  (-...-.) → echoed as '='  (paragraph/separator)
        //   SK  (...-.-)  → echoed as '%'  (end of QSO, some firmware)
        //   KN  (-.--.)   → echoed as '('  (go ahead, specific station)
        //
        // The patterns here are the combined prosign element sequences
        // (A+R, B+T, S+K, K+N sent without inter-character gaps).
        // enqueue_char() schedules them as individual synthetic DIT/DAH events
        // with inter-element gaps — exactly right for a prosign.
        '+' => &[false, true,  false, true,  false],           // AR .-.-.
        '=' => &[true,  false, false, false, true],            // BT -...-
        '%' => &[false, false, false, true,  false, true],     // SK ...-.-
        '(' => &[true,  false, true,  true,  false],           // KN -.--.
        _   => &[],
    }
}

// ── Synthesis queue ───────────────────────────────────────────────────────────
struct SynthEvent {
    is_dah:  bool,
    emit_at: Instant,
}

// ── WinKeyerKeyer ────────────────────────────────────────────────────────────

pub struct WinKeyerKeyer {
    /// Channel on which the background reader delivers decoded chars
    rx_chars:  Receiver<char>,
    /// Timed DIT/DAH event queue
    queue:     VecDeque<SynthEvent>,
    /// Dot duration derived from user_wpm (drives the synthesis timing)
    dot_dur:   Duration,
    /// Timeline cursor: the instant at which the queue's last event *ends*
    /// (including the trailing inter-character gap).  New characters are
    /// appended after this point.
    next_slot: Instant,
}

impl WinKeyerKeyer {
    pub fn new(
        port_path:     &str,
        dot_dur:       Duration,
        paddle_mode:   crate::config::PaddleMode,
        switch_paddle: bool,
    ) -> Result<Self> {
        if port_path.is_empty() {
            return Err(anyhow!(
                "WinKeyer requires an explicit serial port.\n  \
                 Pass it with  --port /dev/ttyUSB0  (Linux)\n  \
                               --port COM3          (Windows)\n  \
                 Run `cw-qso-sim --list-ports` to list all available ports."
            ));
        }

        // ── Build mode byte ───────────────────────────────────────────────────
        // bit 6 = paddle echoback (mandatory for us)
        // bits 5-4 = keyer mode: 01 = Iambic A, 00 = Iambic B
        // bit 3 = paddle swap
        let keyer_mode_bits: u8 = match paddle_mode {
            crate::config::PaddleMode::IambicA => 0b01,
            _                                  => 0b00,
        };
        let swap_bit: u8 = if switch_paddle { 0x08 } else { 0x00 };
        let mode_byte: u8 = 0x40                 // paddle echoback
            | (keyer_mode_bits << 4)             // keyer mode bits
            | swap_bit;                          // paddle swap

        // ── Open serial port ─────────────────────────────────────────────────
        let mut port: Box<dyn SerialPort> = serialport::new(port_path, BAUD_RATE)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::Two)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|e| anyhow!(
                "Cannot open WinKeyer port '{}': {e}\n  \
                 Check the device is connected and you have permission.\n  \
                 Linux: sudo usermod -aG dialout $USER  (then re-login)",
                port_path
            ))?;

        // Assert DTR — WinKeyer USB models need it to power the logic level
        if let Err(e) = port.write_data_terminal_ready(true) {
            log::warn!("[winkeyer] Could not assert DTR: {e}");
        }

        log::info!("[winkeyer] Opened {} at {} baud (8N2)", port_path, BAUD_RATE);

        // ── Initialise host mode ─────────────────────────────────────────────
        // 1. Close first — resets any leftover session from a previous run.
        port.write_all(&[0x00, 0x03])?;
        thread::sleep(Duration::from_millis(100));

        // 2. Drain stale bytes from the input buffer.
        let mut drain = [0u8; 64];
        let _ = port.read(&mut drain);

        // 3. Open host mode.
        port.write_all(&[0x00, 0x02])?;
        thread::sleep(Duration::from_millis(500));

        // 4. Read firmware version (1 byte expected).
        let mut ver_buf = [0u8; 8];
        match port.read(&mut ver_buf) {
            Ok(n) if n > 0 => log::info!("[winkeyer] Firmware version: {}", ver_buf[0]),
            _              => log::warn!("[winkeyer] No version byte received — verify port"),
        }

        // 5. Set mode: enable paddle echoback + keyer mode.
        port.write_all(&[0x0E, mode_byte])?;
        log::info!("[winkeyer] Mode byte: 0x{:02X}  (paddle echo ON, mode bits {:02b}, swap {})",
            mode_byte, keyer_mode_bits, switch_paddle);

        // ── Spawn background reader ───────────────────────────────────────────
        let (tx, rx) = mpsc::channel::<char>();
        thread::spawn(move || serial_reader(port, tx));

        Ok(Self {
            rx_chars:  rx,
            queue:     VecDeque::new(),
            dot_dur,
            next_slot: Instant::now(),
        })
    }

    /// Append synthesised DIT/DAH events for one decoded character.
    ///
    /// Timing follows standard Morse spacing (all in units of dot_dur):
    ///   dit element  = 1 unit
    ///   dah element  = 3 units
    ///   inter-element gap = 1 unit
    ///   inter-character gap = 3 units (appended after the last element)
    ///   word space (ASCII ' ') = 7 units total
    ///                           (4 extra since the previous char added 3)
    fn enqueue_char(&mut self, ch: char) {
        // Word space: add 4 extra dots on top of the 3-dot char gap that the
        // previous character already appended → total 7 dot word gap.
        if ch == ' ' {
            self.next_slot += self.dot_dur * 4;
            return;
        }

        let pattern = morse_pattern(ch);
        if pattern.is_empty() {
            return; // Unknown or unmappable — skip silently
        }

        // Start no earlier than 'now' so stale timestamps don't pile up.
        let dot = self.dot_dur;
        let mut t = self.next_slot.max(Instant::now());

        for (i, &is_dah) in pattern.iter().enumerate() {
            self.queue.push_back(SynthEvent { is_dah, emit_at: t });

            let el_dur = if is_dah { dot * 3 } else { dot };

            if i + 1 < pattern.len() {
                // Between elements within the same character: 1-dot gap
                t += el_dur + dot;
            } else {
                // After the last element: 3-dot inter-character gap
                t += el_dur + dot * 3;
            }
        }

        self.next_slot = t;
    }
}

// ── Background serial reader ─────────────────────────────────────────────────
//
// Reads bytes from WinKeyer and forwards decoded ASCII characters to the
// main thread.  Status and speed-pot bytes are filtered out:
//
//   (byte & 0xC0) == 0xC0  →  WK status byte   (top 2 bits = 11)
//   (byte & 0xC0) == 0x80  →  speed-pot byte   (top 2 bits = 10)
//   everything else        →  paddle echo char  (ASCII: high bit always 0)

fn serial_reader(mut port: Box<dyn SerialPort>, tx: Sender<char>) {
    let mut buf = [0u8; 64];
    loop {
        match port.read(&mut buf) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(2));
            }
            Ok(n) => {
                for &byte in &buf[..n] {
                    if (byte & 0xC0) == 0xC0 {
                        log::debug!("[winkeyer] status byte: 0x{:02X}", byte);
                    } else if (byte & 0xC0) == 0x80 {
                        log::debug!("[winkeyer] speed-pot: {} WPM", byte & 0x3F);
                    } else {
                        // Decoded paddle echo
                        let ch = byte as char;
                        log::debug!("[winkeyer] echo char: {:?}", ch);
                        if tx.send(ch).is_err() {
                            return; // Main thread gone — exit
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                log::error!("[winkeyer] serial read error: {e}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

// ── Interactive adapter check (--check-adapter) ───────────────────────────────

/// Open the WinKeyer on `port_path`, wait for DIT then DAH within `timeout`.
/// The WK decodes paddle input and echoes ASCII characters (E = dit, T = dah);
/// poll() converts those back to PaddleEvents — exactly the game-mode path.
/// Returns Ok(true) if both paddles respond.
pub fn check_adapter(port_path: &str, timeout: Duration) -> Result<bool> {
    use crate::config::PaddleMode;
    use crate::morse::decoder::PaddleEvent;

    let mut keyer = WinKeyerKeyer::new(
        port_path,
        Duration::from_millis(60),
        PaddleMode::IambicA,
        false,
    )?;

    println!("Adapter : K1EL WinKeyer (host-mode, paddle echoback)");
    println!("Port    : {port_path}");
    println!("Protocol: {BAUD_RATE} baud 8N2, Admin Open + echoback enabled");
    println!("Tip     : press a single DIT (E) then a single DAH (T)");
    println!();

    let mut dit_ok = false;
    let mut dah_ok = false;

    // ── Step 1: DIT ──────────────────────────────────────────────────────────
    println!("[ 1/2 ]  Press DIT paddle now …");
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match keyer.poll() {
            PaddleEvent::DitDown => {
                println!("         ✓ DIT received");
                dit_ok = true;
                break;
            }
            PaddleEvent::DahDown => {
                println!("         ✗ Got DAH instead of DIT — try --switch-paddle");
            }
            _ => {}
        }
        thread::sleep(Duration::from_millis(2));
    }
    if !dit_ok { println!("         ✗ DIT timeout — no DIT event received"); }

    // Drain leftover synthetic events before the next test
    thread::sleep(Duration::from_millis(300));
    loop {
        match keyer.poll() {
            PaddleEvent::None => break,
            _ => {}
        }
    }

    // ── Step 2: DAH ──────────────────────────────────────────────────────────
    println!("[ 2/2 ]  Press DAH paddle now …");
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match keyer.poll() {
            PaddleEvent::DahDown => {
                println!("         ✓ DAH received");
                dah_ok = true;
                break;
            }
            PaddleEvent::DitDown => {
                println!("         ✗ Got DIT instead of DAH — try --switch-paddle");
            }
            _ => {}
        }
        thread::sleep(Duration::from_millis(2));
    }
    if !dah_ok { println!("         ✗ DAH timeout — no DAH event received"); }

    println!();
    if dit_ok && dah_ok {
        println!("✓  Both paddles OK — adapter is working correctly.");
        Ok(true)
    } else {
        println!("✗  Adapter check failed.");
        Ok(false)
    }
}

// ── KeyerInput impl ───────────────────────────────────────────────────────────

impl KeyerInput for WinKeyerKeyer {
    fn name(&self) -> &str { "WinKeyer K1EL (paddle echoback)" }

    fn poll(&mut self) -> PaddleEvent {
        // Pull any newly decoded characters from the background reader and
        // convert them to scheduled DIT/DAH events.
        while let Ok(ch) = self.rx_chars.try_recv() {
            self.enqueue_char(ch);
        }

        // Emit the next event once its scheduled time has arrived.
        let now = Instant::now();
        if let Some(ev) = self.queue.front() {
            if now >= ev.emit_at {
                let ev = self.queue.pop_front().unwrap();
                return if ev.is_dah { PaddleEvent::DahDown } else { PaddleEvent::DitDown };
            }
        }

        PaddleEvent::None
    }
}
