// src/keyer/mod.rs  —  KeyerInput trait + adapter registry
pub mod keyboard;
#[cfg(feature = "keyer-vband")]
pub mod vband;
#[cfg(feature = "keyer-attiny85")]
pub mod attiny85;

use crate::morse::decoder::PaddleEvent;
use anyhow::Result;
#[cfg(feature = "keyer-vband")]
use hidapi;

/// Adapter interface — returns paddle events non-blocking
pub trait KeyerInput: Send {
    /// Poll for the next event (non-blocking; returns PaddleEvent::None if nothing)
    fn poll(&mut self) -> PaddleEvent;
    /// Human-readable adapter name
    fn name(&self) -> &str;
}

/// List connected HID keyer devices (used by --list-ports)
pub fn list_ports() -> Vec<String> {
    let mut out = vec![];
    #[cfg(feature = "keyer-vband")]
    {
        let mut v = vband_list();
        out.append(&mut v);
    }
    #[cfg(feature = "keyer-attiny85")]
    {
        let mut m = attiny85::list_midi_ports();
        out.append(&mut m);
    }
    if out.is_empty() {
        out.push("No keyer adapters found.".into());
    }
    out
}

#[cfg(feature = "keyer-vband")]
fn vband_list() -> Vec<String> {
    let mut out = vband::list_vband_devices();
    if out.is_empty() {
        if vband::is_vband_present() {
            out.push(format!(
                "VBand detected in sysfs but /dev/hidraw* is not accessible.\
                 \n  Run: sudo chmod a+rw /dev/hidraw*"
            ));
        } else {
            out.push("No VBand HID adapter found (VID 413d:PID 2107). Is it plugged in?".into());
        }
    }
    out
}

/// Probe all compiled-in adapters and return the first one found.
/// Order: VBand HID → ATtiny85 MIDI → Keyboard fallback.
///
/// When neither hardware feature is compiled in (e.g. Windows release build)
/// this returns Keyboard immediately without any HID/MIDI scan.
pub fn autodetect_adapter() -> crate::config::AdapterType {
    use crate::config::AdapterType;

    // Compile-time shortcut: no hardware features → skip scanning entirely.
    #[cfg(not(any(feature = "keyer-vband", feature = "keyer-attiny85")))]
    {
        log::info!("[autodetect] No hardware keyer features compiled in — using keyboard text-input mode");
        return AdapterType::Keyboard;
    }

    #[cfg(feature = "keyer-vband")]
    {
        // Try to actually open the device — listing it is not enough because
        // hidapi can enumerate VBand even when /dev/hidraw* lacks permissions.
        if let Ok(api) = hidapi::HidApi::new() {
            if api.open(vband::VBAND_VID, vband::VBAND_PID).is_ok() {
                log::info!("[autodetect] VBand HID found and accessible");
                return AdapterType::Vband;
            } else if !vband::list_vband_devices().is_empty() {
                log::warn!("[autodetect] VBand detected but cannot open — check /dev/hidraw* permissions");
            }
        }
    }

    #[cfg(feature = "keyer-attiny85")]
    {
        use midir::MidiInput;
        if let Ok(mi) = MidiInput::new("cw-qso-sim-detect") {
            let found = mi.ports().iter().any(|p| {
                let name = mi.port_name(p).unwrap_or_default().to_lowercase();
                attiny85::KNOWN_NAMES.iter().any(|n| name.contains(n))
            });
            if found {
                log::info!("[autodetect] ATtiny85 MIDI found");
                return AdapterType::Attiny85;
            }
        }
    }

    log::info!("[autodetect] No hardware adapter found — using keyboard text-input mode");
    AdapterType::Keyboard
}

/// Factory — `dot_dur` comes from `Timing::from_wpm(cfg.wpm).dot`
///
/// Returns `(keyer, is_keyboard)`.
/// When `is_keyboard` is true the main loop must read crossterm events
/// and forward paddle keys directly to the `tx_key` channel itself —
/// the keyer thread will only produce `PaddleEvent::None` for the stub.
/// Hardware adapters poll their own device so `is_keyboard` is false.
pub fn create_keyer(
    adapter:       crate::config::AdapterType,
    port:          &str,
    mode:          crate::config::PaddleMode,
    dot_dur:       std::time::Duration,
    switch_paddle: bool,
) -> Result<(Box<dyn KeyerInput>, bool)> {
    use crate::config::AdapterType;

    // Resolve Auto before matching
    let adapter = if adapter == AdapterType::Auto {
        let detected = autodetect_adapter();
        log::info!("[autodetect] selected adapter: {:?}", detected);
        detected
    } else {
        adapter
    };

    match adapter {
        AdapterType::Auto => unreachable!(),
        AdapterType::Keyboard | AdapterType::Text | AdapterType::None => {
            Ok((Box::new(keyboard::KeyboardKeyer::new()), true))
        }
        AdapterType::Vband => {
            #[cfg(feature = "keyer-vband")]
            {
                let (dit_mask, dah_mask) = if switch_paddle {
                    (vband::DAH_MASK, vband::DIT_MASK)
                } else {
                    (vband::DIT_MASK, vband::DAH_MASK)
                };
                if switch_paddle { log::info!("Paddle switched: DIT←→DAH"); }
                Ok((Box::new(vband::VBandKeyer::new_with_masks(mode, dot_dur, dit_mask, dah_mask)?), false))
            }
            #[cfg(not(feature = "keyer-vband"))]
            {
                log::warn!("adapter = \"vband\" in config but this build has no VBand support — falling back to keyboard text-input");
                Ok((Box::new(keyboard::KeyboardKeyer::new()), true))
            }
        }
        AdapterType::Attiny85 => {
            #[cfg(feature = "keyer-attiny85")]
            {
                if switch_paddle { log::info!("Paddle switched: DIT←→DAH"); }
                Ok((Box::new(attiny85::Attiny85Keyer::new(mode, dot_dur, port, switch_paddle)?), false))
            }
            #[cfg(not(feature = "keyer-attiny85"))]
            {
                log::warn!("adapter = \"attiny85\" in config but this build has no ATtiny85 support — falling back to keyboard text-input");
                Ok((Box::new(keyboard::KeyboardKeyer::new()), true))
            }
        }
    }
}
