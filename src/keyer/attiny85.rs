// src/keyer/attiny85.rs  —  ATtiny85 Digispark MIDI paddle adapter
//
// Firmware compatibility:
//   paddle_debug.ino        → Note  1 = DIT,  Note  2 = DAH  (vel 0 = release)
//   paddle_decoder_vusb.ino → Note 60 = DIT,  Note 62 = DAH  (NoteOff = release)
//
// The adapter opens the first MIDI input port whose name matches a known
// Digispark/ATtiny85 pattern, or the port specified via --midi-port.
// Paddle state is updated in the midir callback and read lock-free via poll().
//
// ALSA permissions: the current user must be in the `audio` group, or
// PipeWire/JACK must expose the device.  Usually works out-of-the-box on
// modern Linux desktops.

use anyhow::{anyhow, Result};
use midir::{MidiInput, MidiInputConnection};
use crate::morse::decoder::PaddleEvent;
use super::KeyerInput;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// MIDI note numbers recognised as DIT or DAH (both firmware variants)
const DIT_NOTES: &[u8] = &[1, 60];
const DAH_NOTES: &[u8] = &[2, 62];

/// Known Digispark / ATtiny85 MIDI port name fragments (case-insensitive)
pub const KNOWN_NAMES: &[&str] = &[
    "digispark", "attiny", "tiny", "digikey",
    "midistomp", "usb midi", "midi keyer", "cw",
];

#[derive(Default)]
struct PaddleState {
    dit: bool,
    dah: bool,
}

pub struct Attiny85Keyer {
    state:    Arc<Mutex<PaddleState>>,
    _conn:    MidiInputConnection<()>,
    mode:     crate::config::PaddleMode,
    el_dur:   Duration,
    pub dit_mem:  bool,
    pub dah_mem:  bool,
    pub last_el:  Option<bool>,
    pub el_end:   Instant,
}

impl Attiny85Keyer {
    /// Open the MIDI port.  `port_hint` is either "" (auto-detect) or a
    /// substring to match against available port names.
    pub fn new(
        mode:      crate::config::PaddleMode,
        dot_dur:   Duration,
        port_hint: &str,
    ) -> Result<Self> {
        let midi_in = MidiInput::new("cw-qso-sim")
            .map_err(|e| anyhow!("MIDI init failed: {e}"))?;

        let ports = midi_in.ports();
        if ports.is_empty() {
            return Err(anyhow!("No MIDI input ports found.\n  Is the ATtiny85 plugged in?"));
        }

        // Find the best matching port
        let port = if port_hint.is_empty() {
            // Auto-detect: match known names only — no silent fallback to wrong port
            ports.iter().find(|p| {
                let name = midi_in.port_name(p).unwrap_or_default().to_lowercase();
                KNOWN_NAMES.iter().any(|n| name.contains(n))
            })
            .ok_or_else(|| {
                let avail: Vec<_> = ports.iter()
                    .map(|p| midi_in.port_name(p).unwrap_or_default())
                    .collect();
                anyhow!(
                    "ATtiny85 adapter not found.\n  \
                     Available MIDI ports: {avail:?}\n  \
                     → Plug in the device, or use --midi-port \"<name>\" to select manually."
                )
            })?
        } else {
            let hint_lc = port_hint.to_lowercase();
            ports.iter().find(|p| {
                let name = midi_in.port_name(p).unwrap_or_default().to_lowercase();
                name.contains(&hint_lc)
            })
            .ok_or_else(|| {
                let avail: Vec<_> = ports.iter()
                    .map(|p| midi_in.port_name(p).unwrap_or_default())
                    .collect();
                anyhow!("MIDI port matching '{port_hint}' not found.\n  Available: {avail:?}")
            })?
        };

        let port_name = midi_in.port_name(port).unwrap_or_else(|_| "?".into());
        log::info!("[attiny85] Opening MIDI port: {port_name}");

        let state = Arc::new(Mutex::new(PaddleState::default()));
        let state_cb = Arc::clone(&state);

        let conn = midi_in.connect(
            port,
            "cw-qso-sim-paddle",
            move |_stamp, msg, _| {
                // MIDI message format: [status, note, velocity]
                if msg.len() < 3 { return; }
                let status   = msg[0] & 0xF0;  // strip channel
                let note     = msg[1];
                let velocity = msg[2];

                // NoteOn with vel>0 = press, NoteOn vel=0 or NoteOff = release
                let pressed = status == 0x90 && velocity > 0;
                let released = (status == 0x90 && velocity == 0) || status == 0x80;

                log::debug!(
                    "[attiny85] MIDI status=0x{status:02X} note={note} vel={velocity}"
                );

                if pressed || released {
                    let mut st = state_cb.lock().unwrap();
                    if DIT_NOTES.contains(&note) {
                        st.dit = pressed;
                        log::debug!("[attiny85] DIT {}", if pressed { "press" } else { "release" });
                    } else if DAH_NOTES.contains(&note) {
                        st.dah = pressed;
                        log::debug!("[attiny85] DAH {}", if pressed { "press" } else { "release" });
                    }
                }
            },
            (),
        )
        .map_err(|e| anyhow!("MIDI connect failed: {e}"))?;

        Ok(Self {
            state,
            _conn: conn,
            mode,
            el_dur: dot_dur,
            dit_mem: false,
            dah_mem: false,
            last_el: None,
            el_end: std::time::Instant::now(),
        })
    }
}

/// List available MIDI input ports (for --list-ports output)
pub fn list_midi_ports() -> Vec<String> {
    let Ok(midi_in) = MidiInput::new("cw-qso-sim-list") else { return vec![]; };
    midi_in.ports().iter().enumerate().map(|(i, p)| {
        let name = midi_in.port_name(p).unwrap_or_else(|_| format!("port-{i}"));
        format!("MIDI [{i}] {name}")
    }).collect()
}

/// Interactive adapter check: open the port, wait for each paddle in turn.
/// Reuses Attiny85Keyer + poll() — the exact same code path as game mode.
/// Returns Ok(true) if both paddles pass within `timeout`.
pub fn check_adapter(port_hint: &str, timeout: Duration) -> Result<bool> {
    use crate::config::{PaddleMode};

    // Use IambicA with a dummy dot duration — we only care about press/release
    let mut keyer = Attiny85Keyer::new(PaddleMode::IambicA, Duration::from_millis(60), port_hint)?;

    let port_name = {
        // Just for display — re-query the port name
        let mi = MidiInput::new("cw-qso-sim-check-info").ok();
        mi.and_then(|m| {
            m.ports().iter().find(|p| {
                let name = m.port_name(p).unwrap_or_default().to_lowercase();
                if port_hint.is_empty() {
                    KNOWN_NAMES.iter().any(|n| name.contains(n))
                } else {
                    name.contains(&port_hint.to_lowercase())
                }
            }).and_then(|p| m.port_name(p).ok())
        }).unwrap_or_else(|| "ATtiny85 MIDI".into())
    };

    println!("Adapter : {port_name}");
    println!("Protocol: NoteOn/Off  DIT=notes {:?}  DAH=notes {:?}", DIT_NOTES, DAH_NOTES);
    println!();

    let mut dit_ok = false;
    let mut dah_ok = false;

    // ── Step 1: DIT ───────────────────────────────────────────────────────────
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

    // Reset FSM state between tests
    keyer.dit_mem  = false;
    keyer.dah_mem  = false;
    keyer.last_el  = None;
    keyer.el_end   = Instant::now();

    // ── Step 2: DAH ───────────────────────────────────────────────────────────
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
        println!("✓ ATtiny85 adapter OK — both paddles working");
        Ok(true)
    } else {
        println!("✗ Adapter check FAILED  (DIT: {}  DAH: {})",
            if dit_ok { "OK" } else { "FAIL" },
            if dah_ok { "OK" } else { "FAIL" },
        );
        if dit_ok != dah_ok {
            println!("  Hint: try --switch-paddle if paddles appear swapped");
        }
        Ok(false)
    }
}

impl KeyerInput for Attiny85Keyer {
    fn name(&self) -> &str { "ATtiny85 MIDI" }

    fn poll(&mut self) -> PaddleEvent {
        let (dit_pressed, dah_pressed) = {
            let st = self.state.lock().unwrap();
            (st.dit, st.dah)
        };

        let now = std::time::Instant::now();

        use crate::config::PaddleMode;
        match self.mode {
            PaddleMode::Straight => {
                if dit_pressed { PaddleEvent::DitDown } else { PaddleEvent::DitUp }
            }

            PaddleMode::IambicA | PaddleMode::IambicB => {
                // During active element: only latch the OPPOSITE paddle (squeeze memory)
                if now < self.el_end {
                    match self.last_el {
                        Some(true)  => { if dit_pressed { self.dit_mem = true; } }
                        Some(false) => { if dah_pressed { self.dah_mem = true; } }
                        None        => {}
                    }
                    return PaddleEvent::None;
                }

                // Inter-element gap: accept both paddles
                if dit_pressed { self.dit_mem = true; }
                if dah_pressed { self.dah_mem = true; }

                let send_dit = if dit_pressed && dah_pressed {
                    match self.last_el {
                        None          => true,
                        Some(was_dah) => was_dah,
                    }
                } else if self.dit_mem || dit_pressed {
                    self.dit_mem = false;
                    true
                } else if self.dah_mem || dah_pressed {
                    self.dah_mem = false;
                    false
                } else {
                    self.dit_mem = false;
                    self.dah_mem = false;
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
