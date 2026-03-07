// src/keyer/pico2.rs  —  Raspberry Pi Pico 2 (RP2350) USB MIDI paddle adapter
//
// Firmware compatibility:
//   paddle_debug_pico2.ino  → Note 1 = DIT,  Note 2 = DAH  (vel 0 = release)
//
// The adapter opens the first MIDI input port whose name matches a known
// Pico 2 / TinyUSB pattern, or the port specified via --midi-port.
// Paddle state is updated in the midir callback and read lock-free via poll().
//
// ALSA permissions: the current user must be in the `audio` group, or
// PipeWire/JACK must expose the device.  Usually works out-of-the-box on
// modern Linux desktops.
//
// udev: ensure /etc/udev/rules.d/99-pico.rules grants access to VID 2e8a.

use anyhow::{anyhow, Result};
use midir::{MidiInput, MidiInputConnection};
use crate::morse::decoder::PaddleEvent;
use super::KeyerInput;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// MIDI note numbers for DIT and DAH (paddle_debug_pico2.ino firmware)
const DIT_NOTES: &[u8] = &[1];
const DAH_NOTES: &[u8] = &[2];

/// Known Raspberry Pi Pico 2 / TinyUSB MIDI port name fragments (case-insensitive)
pub const KNOWN_NAMES: &[&str] = &[
    "pico 2", "pico2", "rp2350", "tinyusb midi", "raspberry pi pico",
];

#[derive(Default)]
struct PaddleState {
    dit: bool,
    dah: bool,
}

pub struct Pico2Keyer {
    state:    Arc<Mutex<PaddleState>>,
    _conn:    MidiInputConnection<()>,
    mode:     crate::config::PaddleMode,
    el_dur:   Duration,
    pub dit_mem:        bool,
    pub dah_mem:        bool,
    pub last_el:        Option<bool>,
    pub el_end:         Instant,
    pub prev_dit:       bool,
    pub prev_dah:       bool,
    pub squeeze_active: bool,
    switch_paddle: bool,
}

impl Pico2Keyer {
    /// Open the MIDI port.  `port_hint` is either "" (auto-detect) or a
    /// substring to match against available port names.
    pub fn new(
        mode:          crate::config::PaddleMode,
        dot_dur:       Duration,
        port_hint:     &str,
        switch_paddle: bool,
    ) -> Result<Self> {
        let midi_in = MidiInput::new("cw-qso-sim")
            .map_err(|e| anyhow!("MIDI init failed: {e}"))?;

        let ports = midi_in.ports();
        if ports.is_empty() {
            return Err(anyhow!("No MIDI input ports found.\n  Is the Pico 2 plugged in?"));
        }

        let port = if port_hint.is_empty() {
            ports.iter().find(|p| {
                let name = midi_in.port_name(p).unwrap_or_default().to_lowercase();
                KNOWN_NAMES.iter().any(|n| name.contains(n))
            })
            .ok_or_else(|| {
                let avail: Vec<_> = ports.iter()
                    .map(|p| midi_in.port_name(p).unwrap_or_default())
                    .collect();
                anyhow!(
                    "Raspberry Pi Pico 2 adapter not found.\n  \
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
        log::info!("[pico2] Opening MIDI port: {port_name}");

        let state = Arc::new(Mutex::new(PaddleState::default()));
        let state_cb = Arc::clone(&state);

        let conn = midi_in.connect(
            port,
            "cw-qso-sim-paddle",
            move |_stamp, msg, _| {
                if msg.len() < 3 { return; }
                let status   = msg[0] & 0xF0;
                let note     = msg[1];
                let velocity = msg[2];

                let pressed  = status == 0x90 && velocity > 0;
                let released = (status == 0x90 && velocity == 0) || status == 0x80;

                log::debug!("[pico2] MIDI status=0x{status:02X} note={note} vel={velocity}");

                if pressed || released {
                    let mut st = state_cb.lock().unwrap();
                    if DIT_NOTES.contains(&note) {
                        st.dit = pressed;
                        log::debug!("[pico2] DIT {}", if pressed { "press" } else { "release" });
                    } else if DAH_NOTES.contains(&note) {
                        st.dah = pressed;
                        log::debug!("[pico2] DAH {}", if pressed { "press" } else { "release" });
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
            el_end: Instant::now(),
            prev_dit: false,
            prev_dah: false,
            squeeze_active: false,
            switch_paddle,
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
pub fn check_adapter(port_hint: &str, timeout: Duration) -> Result<bool> {
    use crate::config::PaddleMode;

    let mut keyer = Pico2Keyer::new(PaddleMode::IambicA, Duration::from_millis(60), port_hint, false)?;

    let port_name = {
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
        }).unwrap_or_else(|| "Raspberry Pi Pico 2 USB MIDI".into())
    };

    println!("Adapter : {port_name}");
    println!("Protocol: NoteOn/Off  DIT=notes {:?}  DAH=notes {:?}", DIT_NOTES, DAH_NOTES);
    println!();

    let mut dit_ok = false;
    let mut dah_ok = false;

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

    keyer.dit_mem        = false;
    keyer.dah_mem        = false;
    keyer.last_el        = None;
    keyer.el_end         = Instant::now();
    keyer.prev_dit       = false;
    keyer.prev_dah       = false;
    keyer.squeeze_active = false;

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
        println!("✓ Raspberry Pi Pico 2 adapter OK — both paddles working");
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

impl KeyerInput for Pico2Keyer {
    fn name(&self) -> &str { "Raspberry Pi Pico 2 USB MIDI" }

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
