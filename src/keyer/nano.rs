// src/keyer/nano.rs  —  Arduino Nano serial-MIDI paddle adapter
//
// The Nano runs paddle_debug_Arduino_Nano.ino which sends standard MIDI bytes
// over UART at 31250 baud.  No USB-MIDI bridge (ttymidi etc.) is needed —
// we open the serial port directly and parse the 3-byte MIDI messages here.
//
// MIDI protocol (same note numbers as the ATtiny85 firmware):
//   Note On  (0x90) note=60 vel>0  → DIT press
//   Note On  (0x90) note=60 vel=0  → DIT release
//   Note Off (0x80) note=60        → DIT release
//   Note On  (0x90) note=62 vel>0  → DAH press
//   Note On  (0x90) note=62 vel=0  → DAH release
//   Note Off (0x80) note=62        → DAH release
//
// Linux:  port is typically /dev/ttyUSB0 or /dev/ttyACM0
//         Permissions: add yourself to the `dialout` group, or:
//           sudo chmod a+rw /dev/ttyUSB0
// Windows: port is COM3, COM4, …  (check Device Manager)
// macOS:  /dev/cu.usbserial-*  or /dev/cu.usbmodem*

use anyhow::{anyhow, Result};
use serialport::SerialPort;
use crate::morse::decoder::PaddleEvent;
use super::KeyerInput;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;

/// Standard MIDI baud rate — Arduino Nano and Uno
pub const BAUD_MIDI:  u32 = 31_250;
/// Standard serial baud rate — ESP32 (31250 is unreliable on Linux with CP2102/CH340)
pub const BAUD_ESP32: u32 = 115_200;

const NOTE_DIT: u8 = 60;   // Middle C
const NOTE_DAH: u8 = 62;   // D

/// USB VID/PID pairs for common Arduino Nano USB chips.
/// Used for autodetect when --port is not given.
///
///  CH340 / CH341  — the overwhelming majority of cheap Nano clones
///  CH9102         — newer CH340-family variant
///  FT232RL        — some genuine or high-quality clones
///  ATmega16U2     — genuine Arduino Nano (old bootloader / new bootloader)
pub const NANO_USB_IDS: &[(u16, u16)] = &[
    (0x1A86, 0x7523),   // CH340
    (0x1A86, 0x55D4),   // CH9102
    (0x0403, 0x6001),   // FT232RL
    (0x2341, 0x0043),   // Arduino Nano (ATmega16U2, new bootloader)
    (0x2341, 0x0001),   // Arduino Nano (ATmega16U2, old bootloader)
];

/// USB VID/PID pairs for Arduino Uno USB chips.
///
///  ATmega16U2 (genuine Uno R3, new + old bootloader)
///  Uno R4 Minima / WiFi (RA4M1 USB)
///  CH340 clones — distinguished from Nano clones by product string where possible
pub const UNO_USB_IDS: &[(u16, u16)] = &[
    (0x2341, 0x0043),   // Uno R3 ATmega16U2 (new bootloader)
    (0x2341, 0x0001),   // Uno ATmega16U2    (old bootloader)
    (0x2341, 0x0049),   // Uno WiFi Rev2
    (0x2341, 0x0069),   // Uno R4 Minima
    (0x2341, 0x0070),   // Uno R4 WiFi
    (0x1A86, 0x7523),   // CH340 clone  (shared with Nano clones)
    (0x0403, 0x6001),   // FT232RL      (shared with Nano clones)
];

/// Scan for an Arduino Uno by USB VID/PID.
/// Where VID/PID is shared with the Nano (CH340/FT232) the product string
/// is checked for "uno" first; if unreadable the port is still returned as
/// a candidate so the user can confirm with --list-ports.
pub fn autodetect_uno_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    // First pass: prefer ports whose product string contains "uno"
    for p in &ports {
        if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
            if UNO_USB_IDS.iter().any(|&(v, d)| info.vid == v && info.pid == d) {
                let name = info.product.as_deref().unwrap_or("").to_lowercase();
                if name.contains("uno") {
                    log::info!(
                        "[uno] autodetect (name match): {} (VID:{:04x} PID:{:04x} \"{}\")",
                        p.port_name, info.vid, info.pid,
                        info.product.as_deref().unwrap_or("?")
                    );
                    return Some(p.port_name.clone());
                }
            }
        }
    }
    // Second pass: accept any VID/PID match (Uno-specific PIDs only, not shared ones)
    const UNO_ONLY: &[(u16, u16)] = &[
        (0x2341, 0x0043), (0x2341, 0x0001),
        (0x2341, 0x0049), (0x2341, 0x0069), (0x2341, 0x0070),
    ];
    for p in &ports {
        if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
            if UNO_ONLY.iter().any(|&(v, d)| info.vid == v && info.pid == d) {
                log::info!(
                    "[uno] autodetect (PID match): {} (VID:{:04x} PID:{:04x})",
                    p.port_name, info.vid, info.pid
                );
                return Some(p.port_name.clone());
            }
        }
    }
    None
}

/// Scan available serial ports and return the path of the first one whose
/// USB VID/PID matches a known Nano chip.  Returns None if nothing found.
pub fn autodetect_nano_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    for p in &ports {
        if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
            if NANO_USB_IDS.iter().any(|&(v, d)| info.vid == v && info.pid == d) {
                log::info!(
                    "[nano] autodetect: found {} (VID:{:04x} PID:{:04x})",
                    p.port_name, info.vid, info.pid
                );
                return Some(p.port_name.clone());
            }
        }
    }
    None
}

#[derive(Default)]
struct PaddleState { dit: bool, dah: bool }

pub struct NanoKeyer {
    state:          Arc<Mutex<PaddleState>>,
    _reader:        thread::JoinHandle<()>,  // background serial reader
    mode:           crate::config::PaddleMode,
    el_dur:         Duration,
    dit_mem:        bool,
    dah_mem:        bool,
    last_el:        Option<bool>,
    el_end:         Instant,
    prev_dit:       bool,
    prev_dah:       bool,
    squeeze_active: bool,
    switch_paddle:  bool,
}

impl NanoKeyer {
    /// Open `port_path` (e.g. "/dev/ttyUSB0" or "COM3") at `baud_rate`.
    /// Use `BAUD_MIDI` (31250) for Arduino Nano/Uno, `BAUD_ESP32` (115200) for ESP32.
    pub fn new(
        mode:          crate::config::PaddleMode,
        dot_dur:       Duration,
        port_path:     &str,
        switch_paddle: bool,
        baud_rate:     u32,
    ) -> Result<Self> {
        // If no port given, try to find one by USB VID/PID
        let resolved = if port_path.is_empty() {
            autodetect_nano_port().ok_or_else(|| anyhow!(
                "Arduino Nano not found automatically.\n  \
                 Plug in the Nano, then either:\n  \
                   --port /dev/ttyUSB0    (Linux)\n  \
                   --port COM3            (Windows)\n  \
                 Run `cw-qso-sim --list-ports` to see all serial ports."
            ))?
        } else {
            port_path.to_string()
        };

        let port: Box<dyn SerialPort> = serialport::new(&resolved, baud_rate)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|e| anyhow!(
                "Cannot open serial port '{}': {e}\n  \
                 Check that the device is plugged in and you have read/write permission.\n  \
                 Linux: sudo usermod -aG dialout $USER  (then re-login)",
                resolved
            ))?;

        log::info!("[nano] Opened {} at {} baud", resolved, baud_rate);

        let state     = Arc::new(Mutex::new(PaddleState::default()));
        let state_cb  = Arc::clone(&state);

        // Background thread: read raw MIDI bytes, parse, update state
        let handle = thread::spawn(move || {
            serial_reader(port, state_cb);
        });

        Ok(Self {
            state,
            _reader: handle,
            mode,
            el_dur: dot_dur,
            dit_mem: false,
            dah_mem: false,
            last_el: None,
            el_end: Instant::now(),
            prev_dit: false,
            prev_dah: false,
            squeeze_active: false,
            switch_paddle,
        })
    }
}

// ── Serial MIDI reader (runs in background thread) ────────────────────────────

fn serial_reader(mut port: Box<dyn SerialPort>, state: Arc<Mutex<PaddleState>>) {
    // Simple MIDI byte-stream parser.
    // MIDI is self-synchronising: status bytes have bit7 set, data bytes don't.
    let mut buf  = [0u8; 64];
    let mut msg  = Vec::<u8>::with_capacity(3);
    let mut expected_len = 0usize;

    loop {
        match port.read(&mut buf) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(1));
                continue;
            }
            Ok(n) => {
                for &byte in &buf[..n] {
                    if byte & 0x80 != 0 {
                        // Status byte — start of new message
                        msg.clear();
                        msg.push(byte);
                        let status = byte & 0xF0;
                        expected_len = match status {
                            0x80 | 0x90 => 3,   // NoteOff / NoteOn: 3 bytes
                            _           => 1,   // ignore other messages
                        };
                    } else {
                        // Data byte
                        msg.push(byte);
                    }

                    if msg.len() == expected_len && expected_len == 3 {
                        process_midi(&msg, &state);
                        msg.clear();
                        expected_len = 0;
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                log::error!("[nano] Serial read error: {e}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn process_midi(msg: &[u8], state: &Arc<Mutex<PaddleState>>) {
    let status   = msg[0] & 0xF0;
    let note     = msg[1];
    let velocity = msg[2];

    let pressed  = status == 0x90 && velocity > 0;
    let released = (status == 0x90 && velocity == 0) || status == 0x80;

    log::debug!("[nano] MIDI status=0x{:02X} note={note} vel={velocity}", msg[0]);

    if pressed || released {
        let mut st = state.lock().unwrap();
        if note == NOTE_DIT {
            st.dit = pressed;
            log::debug!("[nano] DIT {}", if pressed { "press" } else { "release" });
        } else if note == NOTE_DAH {
            st.dah = pressed;
            log::debug!("[nano] DAH {}", if pressed { "press" } else { "release" });
        }
    }
}

// ── List serial ports (for --list-ports) ─────────────────────────────────────

pub fn list_nano_ports() -> Vec<String> {
    match serialport::available_ports() {
        Ok(ports) => ports.iter().map(|p| {
            let detail = match &p.port_type {
                serialport::SerialPortType::UsbPort(info) => format!(
                    "USB VID:{:04x} PID:{:04x}{}",
                    info.vid, info.pid,
                    info.product.as_deref()
                        .map(|s| format!(" \"{}\"", s))
                        .unwrap_or_default()
                ),
                serialport::SerialPortType::BluetoothPort => "Bluetooth".into(),
                _ => "Serial".into(),
            };
            format!("Serial [Nano?] {}  ({})", p.port_name, detail)
        }).collect(),
        Err(e) => vec![format!("Serial port enumeration failed: {e}")],
    }
}

// ── Interactive adapter check (--check-adapter) ───────────────────────────────

/// Open `port_path`, wait for DIT then DAH within `timeout`.
/// Works for Arduino Nano, Arduino Uno, and ESP32 — all speak the same protocol.
/// Returns Ok(true) if both paddles produce the expected events.
pub fn check_adapter(port_path: &str, label: &str, baud_rate: u32, timeout: Duration) -> Result<bool> {
    use crate::config::PaddleMode;
    use crate::morse::decoder::PaddleEvent;

    let mut keyer = NanoKeyer::new(
        PaddleMode::IambicA,
        Duration::from_millis(60),
        port_path,
        false,
        baud_rate,
    )?;

    println!("Adapter : {label}");
    println!("Port    : {port_path}");
    println!("Protocol: MIDI NoteOn/Off  DIT=note {NOTE_DIT}  DAH=note {NOTE_DAH}  @ {baud_rate} baud");
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

    // Reset FSM between steps
    keyer.dit_mem       = false;
    keyer.dah_mem       = false;
    keyer.last_el       = None;
    keyer.el_end        = Instant::now();
    keyer.prev_dit      = false;
    keyer.prev_dah      = false;
    keyer.squeeze_active = false;

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

// ── KeyerInput impl (iambic/straight logic, same as ATtiny85) ────────────────

impl KeyerInput for NanoKeyer {
    fn name(&self) -> &str { "Arduino Nano (serial MIDI)" }

    fn poll(&mut self) -> PaddleEvent {
        let (raw_dit, raw_dah) = {
            let st = self.state.lock().unwrap();
            (st.dit, st.dah)
        };
        let (dit_pressed, dah_pressed) = if self.switch_paddle {
            (raw_dah, raw_dit)
        } else {
            (raw_dit, raw_dah)
        };

        let now = Instant::now();

        use crate::config::PaddleMode;
        match self.mode {
            PaddleMode::Straight => {
                if dit_pressed { PaddleEvent::DitDown } else { PaddleEvent::DitUp }
            }

            PaddleMode::IambicA | PaddleMode::IambicB => {
                let dit_edge = dit_pressed && !self.prev_dit;
                let dah_edge = dah_pressed && !self.prev_dah;
                self.prev_dit = dit_pressed;
                self.prev_dah = dah_pressed;

                if dit_pressed && dah_pressed { self.squeeze_active = true; }
                if self.mode == PaddleMode::IambicB && !dit_pressed && !dah_pressed {
                    self.squeeze_active = false;
                }

                if dit_edge { self.dit_mem = true; }
                if dah_edge { self.dah_mem = true; }

                // During element
                if now < self.el_end {
                    match self.mode {
                        PaddleMode::IambicA => {
                            if dit_pressed && dah_pressed {
                                match self.last_el {
                                    Some(true)  => { self.dit_mem = true; }
                                    Some(false) => { self.dah_mem = true; }
                                    None        => {}
                                }
                            }
                        }
                        _ => {
                            match self.last_el {
                                Some(true)  => { if dit_pressed { self.dit_mem = true; } }
                                Some(false) => { if dah_pressed { self.dah_mem = true; } }
                                None        => {}
                            }
                        }
                    }
                    return PaddleEvent::None;
                }

                // Element complete: decide next
                match self.mode {
                    PaddleMode::IambicA => {
                        if !self.squeeze_active {
                            if dit_pressed && !dah_pressed { self.dit_mem = true; }
                            if dah_pressed && !dit_pressed { self.dah_mem = true; }
                        }
                    }
                    _ => {
                        if dit_pressed { self.dit_mem = true; }
                        if dah_pressed { self.dah_mem = true; }
                    }
                }

                let send_dit = if dit_pressed && dah_pressed {
                    let s = match self.last_el { None => true, Some(was_dah) => was_dah };
                    if s { self.dit_mem = false; } else { self.dah_mem = false; }
                    s
                } else if self.dit_mem {
                    self.dit_mem = false; true
                } else if self.dah_mem {
                    self.dah_mem = false; false
                } else {
                    if self.mode == PaddleMode::IambicA && !dit_pressed && !dah_pressed {
                        self.squeeze_active = false;
                    }
                    self.last_el = None;
                    return PaddleEvent::None;
                };

                let dur = if send_dit { self.el_dur } else { self.el_dur * 3 };
                self.el_end  = now + dur + self.el_dur;
                self.last_el = Some(!send_dit);
                if send_dit { PaddleEvent::DitDown } else { PaddleEvent::DahDown }
            }
        }
    }
}
