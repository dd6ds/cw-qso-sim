// src/keyer/vband.rs  —  VBand USB CW Adapter  (VID 0x413d / PID 0x2107)
//
// The VBand dongle is a USB HID device — it does NOT appear as /dev/ttyUSB*
// or any serial port.  On Linux it appears as /dev/hidraw*.
//
// HID protocol (8-byte report, same for VBand + ATtiny85 compatible firmware):
//   byte 0  =  paddle bitmask
//     0x01  →  DIT paddle pressed
//     0x10  →  DAH paddle pressed
//   bytes 1-7 always 0x00
//
// ── Two device backends ───────────────────────────────────────────────────────
//
//  1. HidApi  (default on all platforms)
//     The VBand adapter works out-of-the-box on Linux and macOS with the
//     system HID driver.  On Windows it works with the built-in HidUsb driver.
//
//  2. WinUSB / rusb  (feature = "keyer-vband-winusb", Windows only)
//     If someone accidentally installed a WinUSB / libwdi driver via Zadig the
//     device is removed from the Windows HID stack and hidapi can no longer see
//     it.  This backend uses rusb (libusb) to reach the device through WinUSB.
//     It is tried automatically as a fallback when HidApi open fails on Windows.
//
// ── Linux permissions ─────────────────────────────────────────────────────────
// /dev/hidraw* is root-only by default.  Create a udev rule once:
//
//   echo 'SUBSYSTEM=="hidraw", ATTRS{idVendor}=="413d", \
//         ATTRS{idProduct}=="2107", GROUP="plugdev", MODE="0660"' \
//     | sudo tee /etc/udev/rules.d/99-vband-cw.rules
//   sudo udevadm control --reload-rules && sudo udevadm trigger
//   sudo usermod -aG plugdev $USER   # re-login needed

use anyhow::{anyhow, Result};
use hidapi::HidApi;
use crate::config::PaddleMode;
use crate::morse::decoder::PaddleEvent;
use super::KeyerInput;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

pub const VBAND_VID: u16 = 0x413d;
pub const VBAND_PID: u16 = 0x2107;

/// Default bitmasks — correct for all known VBand / ATtiny85 HID firmware.
pub const DIT_MASK: u8 = 0x01;
pub const DAH_MASK: u8 = 0x10;

// ── Windows Raw-Key shim ──────────────────────────────────────────────────────
//
// When the VBand is only visible as the keyboard HID collection (\KBD path),
// kbdhid.sys owns the device and all user-space ReadFile calls return nothing.
// However, kbdhid.sys DOES translate the VBand's HID modifier byte into real
// Windows key events.  The VBand uses the standard boot-protocol keyboard
// modifier format:
//
//   HID byte 0 = 0x01  →  Left Control held  (DIT_MASK)
//   HID byte 0 = 0x10  →  Right Control held (DAH_MASK)
//
// We can therefore recover the paddle state with GetAsyncKeyState, which
// polls the low-level key state of individual virtual keys without requiring
// a message loop.  This is poll-based, so it fits naturally into our
// existing 1 ms polling loop.
//
// Virtual key codes:
//   VK_LCONTROL = 0xA2 = 162   ← DIT
//   VK_RCONTROL = 0xA3 = 163   ← DAH
//
// To avoid reporting spurious "changes" on every poll, we store the previous
// bitmask in a Cell<u8> so read_raw (&self) can update it without &mut.

#[cfg(target_os = "windows")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

#[cfg(target_os = "windows")]
const VK_LCONTROL: i32 = 0xA2;   // maps to DIT_MASK 0x01
#[cfg(target_os = "windows")]
const VK_RCONTROL: i32 = 0xA3;   // maps to DAH_MASK 0x10

// ── Device backend ────────────────────────────────────────────────────────────

/// Abstraction over the USB access backends.
enum VBandDevice {
    /// Standard hidapi path — works on Linux and macOS; on Windows only
    /// available when a non-\KBD (generic HID) interface is exposed.
    Hid(hidapi::HidDevice),
    /// WinUSB / libusb path — used on Windows when the device has a
    /// WinUSB / libwdi driver installed (e.g. via Zadig).
    #[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
    WinUsb {
        handle:   rusb::DeviceHandle<rusb::GlobalContext>,
        endpoint: u8,
    },
    /// Windows keyboard-shim — used when only the \KBD HID collection is
    /// exposed and kbdhid.sys blocks raw reads.  Polls GetAsyncKeyState for
    /// LCtrl (DIT) and RCtrl (DAH) and re-synthesises the bitmask.
    #[cfg(target_os = "windows")]
    WinKbd {
        dit_vk: i32,                    // VK_LCONTROL
        dah_vk: i32,                    // VK_RCONTROL
        prev:   std::cell::Cell<u8>,    // last reported bitmask (change detection)
    },
}

/// Internal read result returned by [`VBandDevice::read_raw`].
enum ReadResult {
    /// New HID report arrived — `mask` is the extracted paddle bitmask.
    Report(u8),
    /// Timeout — no report, previous state stands.
    NoData,
    /// Unrecoverable I/O error — caller should reset paddle state.
    Error,
}

impl VBandDevice {
    /// Read one USB report with a 1 ms timeout and extract the paddle bitmask.
    ///
    /// **Windows report-ID offset:**
    /// hidapi on Windows always prepends a Report-ID byte to the input buffer:
    /// - No-report-ID devices → 0x00 prepended  (buf[0]=0x00, data starts at buf[1])
    /// - Report-ID N devices  → N prepended      (buf[0]=N,    data starts at buf[1])
    ///
    /// The VBand's keyboard HID collection sends a standard 8-byte keyboard
    /// report where byte 0 is the modifier field (bit0=LCtrl=DIT, bit4=RCtrl=DAH).
    /// After hidapi's report-ID prepend, that modifier byte sits at buf[1].
    ///
    /// On Linux/macOS hidapi does NOT prepend a report-ID byte, so the modifier
    /// byte (or the raw bitmask for firmware using a custom report) is at buf[0].
    ///
    /// Fix: scan buf[0] and buf[1]; use whichever is non-zero (or buf[0] if both
    /// zero, i.e. "all released").  This correctly handles both platforms and
    /// both VBand firmware variants (custom bitmask report vs keyboard report).
    fn read_raw(&self, buf: &mut [u8]) -> ReadResult {
        match self {
            VBandDevice::Hid(dev) => {
                match dev.read_timeout(buf, 1) {
                    Ok(n) if n >= 1 => {
                        // Pick the first non-zero byte from buf[0..=1].
                        // On Linux/macOS: paddle mask is in buf[0].
                        // On Windows (keyboard HID + report-ID prepend): buf[0]=0x00, mask in buf[1].
                        let mask = if buf[0] != 0 { buf[0] }
                                   else if n >= 2 { buf[1] }
                                   else           { 0 };
                        log::debug!(
                            "[vband/hid] n={n} buf[0]=0x{:02X} buf[1]=0x{:02X} → mask=0x{mask:02X}",
                            buf[0], if n >= 2 { buf[1] } else { 0 }
                        );
                        ReadResult::Report(mask)
                    }
                    Ok(_) => ReadResult::NoData,
                    Err(e) => {
                        log::warn!("VBand HID read error: {e}");
                        ReadResult::Error
                    }
                }
            }

            #[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
            VBandDevice::WinUsb { handle, endpoint } => {
                match handle.read_interrupt(*endpoint, buf, Duration::from_millis(1)) {
                    Ok(n) if n >= 1 => {
                        let mask = if buf[0] != 0 { buf[0] }
                                   else if n >= 2 { buf[1] }
                                   else           { 0 };
                        ReadResult::Report(mask)
                    }
                    Ok(_)                      => ReadResult::NoData,
                    Err(rusb::Error::Timeout)  => ReadResult::NoData,
                    Err(e) => {
                        log::warn!("VBand WinUSB read error: {e}");
                        ReadResult::Error
                    }
                }
            }

            // ── Windows keyboard shim ─────────────────────────────────────
            // kbdhid.sys translates the VBand's modifier byte into LCtrl/RCtrl
            // key events.  GetAsyncKeyState polls the live key state; we
            // reconstruct the bitmask and report only on change.
            #[cfg(target_os = "windows")]
            VBandDevice::WinKbd { dit_vk, dah_vk, prev } => {
                let lctrl = unsafe { GetAsyncKeyState(*dit_vk) } as u16 & 0x8000 != 0;
                let rctrl = unsafe { GetAsyncKeyState(*dah_vk) } as u16 & 0x8000 != 0;
                // Reconstruct bitmask: DIT_MASK=0x01, DAH_MASK=0x10
                let mask = (lctrl as u8) | ((rctrl as u8) << 4);
                let old  = prev.get();
                if mask != old {
                    prev.set(mask);
                    log::debug!("[vband/winkbd] LCtrl={lctrl} RCtrl={rctrl} → mask=0x{mask:02X}");
                    ReadResult::Report(mask)
                } else {
                    ReadResult::NoData
                }
            }
        }
    }

    /// Human-readable backend label for log output.
    fn backend_name(&self) -> &'static str {
        match self {
            VBandDevice::Hid(_) => "HidApi",
            #[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
            VBandDevice::WinUsb { .. } => "WinUSB",
            #[cfg(target_os = "windows")]
            VBandDevice::WinKbd { .. } => "WinKbd (GetAsyncKeyState)",
        }
    }
}

// ── Open helpers ──────────────────────────────────────────────────────────────

/// Returns true when the ONLY HID interface for the VBand on Windows is the
/// keyboard collection (path ends `\KBD`).  kbdhid.sys owns this interface
/// exclusively — raw HID reads return nothing.  The caller should switch to
/// the keyboard-event shim (`VBandWindowsKeyer`) instead.
#[cfg(target_os = "windows")]
pub fn is_kbd_only_interface() -> bool {
    let Ok(api) = HidApi::new() else { return false; };
    let paths: Vec<_> = api.device_list()
        .filter(|d| d.vendor_id() == VBAND_VID && d.product_id() == VBAND_PID)
        .map(|d| d.path().to_string_lossy().to_uppercase())
        .collect();
    // Present + every path ends with \KBD → keyboard-only
    !paths.is_empty() && paths.iter().all(|p| p.ends_with("\\KBD"))
}

/// Try to open the VBand adapter through any available backend.
/// Returns Err (with a descriptive message) if no readable interface is found.
fn open_device() -> Result<VBandDevice> {
    // ── 1. HidApi ─────────────────────────────────────────────────────────
    //
    // On Windows the VBand exposes a \KBD top-level collection owned by
    // kbdhid.sys.  ReadFile on that interface always returns nothing.
    // If a non-\KBD (generic HID) path exists we prefer it; otherwise we
    // fall through to the WinKbd shim below.
    if let Ok(api) = HidApi::new() {
        let all_paths: Vec<_> = api.device_list()
            .filter(|d| d.vendor_id() == VBAND_VID && d.product_id() == VBAND_PID)
            .map(|d| d.path().to_owned())
            .collect();

        // Skip \KBD paths on Windows; keep all paths on Linux / macOS.
        let readable: Vec<_> = all_paths.iter().filter(|p| {
            #[cfg(target_os = "windows")]
            { !p.to_string_lossy().to_uppercase().ends_with("\\KBD") }
            #[cfg(not(target_os = "windows"))]
            { let _ = p; true }
        }).collect();

        // Prefer readable; fall back to all paths on non-Windows as last resort.
        let candidates: Vec<_> = if !readable.is_empty() {
            readable
        } else {
            #[cfg(not(target_os = "windows"))]
            { all_paths.iter().collect() }
            #[cfg(target_os = "windows")]
            { vec![] }   // KBD-only on Windows → WinKbd fallback
        };

        for path in candidates {
            match api.open_path(path) {
                Ok(dev) => {
                    log::info!("[vband] opened via HidApi  path={}", path.to_string_lossy());
                    return Ok(VBandDevice::Hid(dev));
                }
                Err(e) => log::debug!("[vband] HidApi open_path({}) failed: {e}", path.to_string_lossy()),
            }
        }
        if !all_paths.is_empty() {
            log::debug!("[vband] {} HID path(s) found but none readable", all_paths.len());
        }
    }

    // ── 2. WinKbd shim (Windows only) ─────────────────────────────────────
    // Only the \KBD interface exists; kbdhid.sys translates VBand modifier
    // byte → LCtrl (DIT) / RCtrl (DAH) key events.  Read via GetAsyncKeyState.
    #[cfg(target_os = "windows")]
    if is_kbd_only_interface() {
        log::info!(
            "[vband] KBD-only interface detected — using WinKbd (GetAsyncKeyState) shim.\
             \n  DIT = Left Ctrl  |  DAH = Right Ctrl"
        );
        return Ok(VBandDevice::WinKbd {
            dit_vk: VK_LCONTROL,
            dah_vk: VK_RCONTROL,
            prev:   std::cell::Cell::new(0),
        });
    }

    // ── 2. WinUSB fallback (Windows, feature "keyer-vband-winusb") ────────
    #[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
    {
        match try_open_winusb() {
            Ok(dev) => {
                log::info!("[vband] opened via WinUSB backend (libwdi/Zadig driver detected)");
                return Ok(dev);
            }
            Err(e) => log::debug!("[vband] WinUSB open failed: {e}"),
        }
    }

    // ── No backend worked ─────────────────────────────────────────────────
    let hint = build_open_hint();
    Err(anyhow!("Cannot open VBand {VBAND_VID:04x}:{VBAND_PID:04x}{hint}"))
}

fn build_open_hint() -> &'static str {
    if cfg!(target_os = "linux") {
        "\n  Hint: /dev/hidraw* may lack permissions.\
         \n  Quick fix:  sudo chmod a+rw /dev/hidraw*\
         \n  Permanent:  install udev rule 99-vband-cw.rules (see top of vband.rs)"
    } else if cfg!(target_os = "windows") {
        "\n  Hint: Is the VBand plugged in?\
         \n  ‣ The adapter should appear in Device Manager under Human Interface Devices.\
         \n  ‣ If another VBand application is running, close it first."
    } else if cfg!(target_os = "macos") {
        "\n  Hint: macOS 14+ requires Input Monitoring permission for HID devices.\
         \n  → System Settings → Privacy & Security → Input Monitoring\
         \n  → Add your terminal app (Terminal, iTerm2, …) and re-launch it.\
         \n  The device must also be visible in: Apple menu → About This Mac\
         \n  → System Report → USB."
    } else {
        ""
    }
}

/// WinUSB backend open — Windows + feature "keyer-vband-winusb" only.
#[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
fn try_open_winusb() -> Result<VBandDevice> {
    let handle = rusb::open_device_with_vid_pid(VBAND_VID, VBAND_PID)
        .ok_or_else(|| anyhow!("VBand not found via WinUSB ({VBAND_VID:04x}:{VBAND_PID:04x})"))?;

    let endpoint = find_interrupt_in_ep(&handle).unwrap_or_else(|e| {
        log::warn!("[vband/winusb] endpoint scan failed ({e}) — defaulting to 0x81");
        0x81
    });

    handle.claim_interface(0)
        .map_err(|e| anyhow!("WinUSB: cannot claim interface 0: {e}"))?;
    handle.set_alternate_setting(0, 0).ok(); // may fail, not fatal

    log::debug!("[vband/winusb] claimed interface 0, interrupt IN ep=0x{endpoint:02X}");
    Ok(VBandDevice::WinUsb { handle, endpoint })
}

/// Scan USB descriptors to find the first interrupt IN endpoint.
#[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
fn find_interrupt_in_ep(handle: &rusb::DeviceHandle<rusb::GlobalContext>) -> Result<u8> {
    let cfg = handle.device().active_config_descriptor()
        .map_err(|e| anyhow!("USB config descriptor: {e}"))?;
    for iface in cfg.interfaces() {
        for desc in iface.descriptors() {
            for ep in desc.endpoint_descriptors() {
                if ep.direction()     == rusb::Direction::In
                && ep.transfer_type() == rusb::TransferType::Interrupt {
                    return Ok(ep.address());
                }
            }
        }
    }
    Err(anyhow!("no interrupt IN endpoint in USB descriptor"))
}

// ── VBandKeyer ────────────────────────────────────────────────────────────────

pub struct VBandKeyer {
    device:    VBandDevice,
    mode:      PaddleMode,
    dit_mask:  u8,
    dah_mask:  u8,
    // Last known HID state — preserved between reports so the FSM
    // sees the paddle as held even when no new HID report arrives.
    last_dit:  bool,
    last_dah:  bool,
    // Iambic FSM state
    el_dur:    Duration,
    dit_mem:   bool,
    dah_mem:   bool,
    last_el:   Option<bool>,   // false = dit, true = dah
    el_end:    Instant,
    // Squeeze detection
    prev_dit:       bool,
    prev_dah:       bool,
    squeeze_active: bool,
}

impl VBandKeyer {
    pub fn new(mode: PaddleMode, dot_duration: Duration) -> Result<Self> {
        Self::new_with_masks(mode, dot_duration, DIT_MASK, DAH_MASK)
    }

    pub fn new_with_masks(
        mode:         PaddleMode,
        dot_duration: Duration,
        dit_mask:     u8,
        dah_mask:     u8,
    ) -> Result<Self> {
        let device = open_device()?;

        log::info!(
            "VBand {:04x}:{:04x} opened via {}  mode={mode:?}  dot={}ms  \
             dit_mask=0x{dit_mask:02X}  dah_mask=0x{dah_mask:02X}",
            VBAND_VID, VBAND_PID,
            device.backend_name(),
            dot_duration.as_millis()
        );

        Ok(Self {
            device,
            mode,
            dit_mask,
            dah_mask,
            last_dit: false,
            last_dah: false,
            el_dur:  dot_duration,
            dit_mem: false,
            dah_mem: false,
            last_el: None,
            el_end:  Instant::now(),
            prev_dit:       false,
            prev_dah:       false,
            squeeze_active: false,
        })
    }

    pub fn set_dot_duration(&mut self, d: Duration) { self.el_dur = d; }

    /// Read the current paddle state from USB.
    ///
    /// Reads ONE report per call (1 ms timeout).  The VBand sends a report
    /// on every state CHANGE only — when nothing arrives the last known state
    /// is preserved, giving us "held" behaviour for free.
    fn read_paddles(&mut self) -> (bool, bool) {
        let mut buf = [0u8; 64];
        match self.device.read_raw(&mut buf) {
            ReadResult::Report(mask) => {
                self.last_dit = (mask & self.dit_mask) != 0;
                self.last_dah = (mask & self.dah_mask) != 0;
                log::debug!(
                    "[vband/{}] mask=0x{mask:02X}  dit={}  dah={}",
                    self.device.backend_name(), self.last_dit, self.last_dah
                );
            }
            ReadResult::NoData => {} // nothing new — keep last state
            ReadResult::Error  => {
                self.last_dit = false;
                self.last_dah = false;
            }
        }
        (self.last_dit, self.last_dah)
    }
}

impl KeyerInput for VBandKeyer {
    fn name(&self) -> &str { "VBand USB HID" }

    fn poll(&mut self) -> PaddleEvent {
        let (dit_pressed, dah_pressed) = self.read_paddles();
        let now = Instant::now();

        match self.mode {
            PaddleMode::Straight => {
                if dit_pressed { PaddleEvent::DitDown }
                else           { PaddleEvent::DitUp   }
            }

            // ── IambicA — strict squeeze ──────────────────────────────────────
            // Opposite memory is only captured when BOTH paddles are pressed
            // simultaneously (a true squeeze).  Single-paddle continuous re-arm
            // is blocked while squeeze_active, so the alternation stops cleanly
            // once the secondary paddle is released and its memory consumed.
            //
            // Example: hold DAH, tap DIT  →  DAH DIT DAH DIT  (C)  then stops.
            //
            // IMPORTANT — squeeze latch lifetime:
            // squeeze_active is set when both paddles are pressed together and
            // cleared ONLY when the keyer returns to true idle (both paddles
            // released, no pending memories).  Clearing it on every
            // "both-released" HID report lets contact-bounce glitches or a
            // brief inter-paddle gap reset the latch mid-sequence, producing
            // one spurious extra element (e.g. C → DAH DIT DAH DIT DAH).
            PaddleMode::IambicA => {
                // ── Edge / squeeze tracking ───────────────────────────────────
                let dit_edge = dit_pressed && !self.prev_dit;
                let dah_edge = dah_pressed && !self.prev_dah;
                self.prev_dit = dit_pressed;
                self.prev_dah = dah_pressed;

                // Latch squeeze; cleared only at true idle (see return-None below)
                if dit_pressed && dah_pressed { self.squeeze_active = true; }

                // New press (rising edge) → latch immediately
                if dit_edge { self.dit_mem = true; }
                if dah_edge { self.dah_mem = true; }

                // ── During element ────────────────────────────────────────────
                if now < self.el_end {
                    // IambicA: set opposite memory ONLY on true squeeze
                    if dit_pressed && dah_pressed {
                        match self.last_el {
                            Some(true)  => { self.dit_mem = true; }
                            Some(false) => { self.dah_mem = true; }
                            None        => {}
                        }
                    }
                    return PaddleEvent::None;
                }

                // ── Element complete: decide next ─────────────────────────────
                // Single-paddle continuous only when NOT in a squeeze sequence
                if !self.squeeze_active {
                    if dit_pressed && !dah_pressed { self.dit_mem = true; }
                    if dah_pressed && !dit_pressed { self.dah_mem = true; }
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
                    // Truly idle: clear squeeze latch so next single-paddle
                    // sequence starts fresh (latch is NOT cleared mid-sequence
                    // to avoid spurious extra elements from contact bounce).
                    if !dit_pressed && !dah_pressed {
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

            // ── IambicB — lenient / extended memory ───────────────────────────
            // Sets opposite memory from a single held paddle too (classic Mode B).
            // Holding one paddle continuously sends that element repeatedly;
            // squeezing adds alternation.  One extra element is queued even after
            // releasing one paddle (the "bonus element" of Mode B).
            PaddleMode::IambicB => {
                // ── Edge / squeeze tracking ───────────────────────────────────
                let dit_edge = dit_pressed && !self.prev_dit;
                let dah_edge = dah_pressed && !self.prev_dah;
                self.prev_dit = dit_pressed;
                self.prev_dah = dah_pressed;

                if dit_pressed && dah_pressed  { self.squeeze_active = true;  }
                if !dit_pressed && !dah_pressed { self.squeeze_active = false; }

                if dit_edge { self.dit_mem = true; }
                if dah_edge { self.dah_mem = true; }

                // ── During element ────────────────────────────────────────────
                if now < self.el_end {
                    // IambicB: lenient — set opposite from single paddle
                    match self.last_el {
                        Some(true)  => { if dit_pressed { self.dit_mem = true; } }
                        Some(false) => { if dah_pressed { self.dah_mem = true; } }
                        None        => {}
                    }
                    return PaddleEvent::None;
                }

                // ── Element complete: decide next ─────────────────────────────
                // IambicB re-arms freely from held paddles
                if dit_pressed { self.dit_mem = true; }
                if dah_pressed { self.dah_mem = true; }

                let send_dit = if dit_pressed && dah_pressed {
                    let s = match self.last_el { None => true, Some(was_dah) => was_dah };
                    if s { self.dit_mem = false; } else { self.dah_mem = false; }
                    s
                } else if self.dit_mem {
                    self.dit_mem = false; true
                } else if self.dah_mem {
                    self.dah_mem = false; false
                } else {
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

// ── VBandWindowsKeyer ─────────────────────────────────────────────────────────
//
// On Windows the VBand registers as a keyboard HID device.  kbdhid.sys claims
// exclusive ownership of the USB interrupt endpoint, so raw HID reads via
// hidapi always return nothing.  Instead, Windows translates the paddle
// presses into standard keyboard events:
//
//   DIT paddle  →  Left Control  (modifier bit 0x01 = DIT_MASK)
//   DAH paddle  →  Right Control (modifier bit 0x10 = DAH_MASK)
//
// Those key events flow through the normal Windows console input queue and are
// readable by crossterm's ReadConsoleInput loop in main.rs.
//
// VBandWindowsKeyer shares an AtomicU8 with the main event loop:
//   bit 0  (0x01) = DIT currently held
//   bit 4  (0x10) = DAH currently held
//
// The main loop sets/clears the bits on LCtrl/RCtrl keydown/keyup events.
// VBandWindowsKeyer::poll() reads the atomic and runs the standard iambic FSM
// — identical logic to VBandKeyer::poll().

pub struct VBandWindowsKeyer {
    /// Shared paddle state: bit0=DIT, bit4=DAH.  Updated by main event loop.
    pub paddle_state:   Arc<AtomicU8>,
    mode:               PaddleMode,
    dit_mask:           u8,
    dah_mask:           u8,
    el_dur:             Duration,
    dit_mem:            bool,
    dah_mem:            bool,
    last_el:            Option<bool>,
    el_end:             Instant,
    prev_dit:           bool,
    prev_dah:           bool,
    squeeze_active:     bool,
}

impl VBandWindowsKeyer {
    /// Create the keyer and return a clone of the shared paddle-state arc so
    /// the caller (main loop) can update it from crossterm events.
    pub fn new(
        mode:         PaddleMode,
        dot_duration: Duration,
        dit_mask:     u8,
        dah_mask:     u8,
    ) -> (Self, Arc<AtomicU8>) {
        let paddle_state = Arc::new(AtomicU8::new(0));
        let shared       = Arc::clone(&paddle_state);
        log::info!(
            "[vband/win-kbd] Using Windows keyboard-event shim \
             (LCtrl=DIT, RCtrl=DAH)  mode={mode:?}  dot={}ms",
            dot_duration.as_millis()
        );
        (Self {
            paddle_state,
            mode,
            dit_mask,
            dah_mask,
            el_dur:         dot_duration,
            dit_mem:        false,
            dah_mem:        false,
            last_el:        None,
            el_end:         Instant::now(),
            prev_dit:       false,
            prev_dah:       false,
            squeeze_active: false,
        }, shared)
    }
}

impl KeyerInput for VBandWindowsKeyer {
    fn name(&self) -> &str { "VBand (Windows keyboard shim)" }

    fn poll(&mut self) -> PaddleEvent {
        let bits        = self.paddle_state.load(Ordering::Relaxed);
        let dit_pressed = (bits & self.dit_mask) != 0;
        let dah_pressed = (bits & self.dah_mask) != 0;
        let now         = Instant::now();

        match self.mode {
            PaddleMode::Straight => {
                if dit_pressed { PaddleEvent::DitDown } else { PaddleEvent::DitUp }
            }

            PaddleMode::IambicA => {
                let dit_edge = dit_pressed && !self.prev_dit;
                let dah_edge = dah_pressed && !self.prev_dah;
                self.prev_dit = dit_pressed;
                self.prev_dah = dah_pressed;
                if dit_pressed && dah_pressed { self.squeeze_active = true; }
                if dit_edge { self.dit_mem = true; }
                if dah_edge { self.dah_mem = true; }
                if now < self.el_end {
                    if dit_pressed && dah_pressed {
                        match self.last_el {
                            Some(true)  => { self.dit_mem = true; }
                            Some(false) => { self.dah_mem = true; }
                            None        => {}
                        }
                    }
                    return PaddleEvent::None;
                }
                if !self.squeeze_active {
                    if dit_pressed && !dah_pressed { self.dit_mem = true; }
                    if dah_pressed && !dit_pressed { self.dah_mem = true; }
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
                    if !dit_pressed && !dah_pressed { self.squeeze_active = false; }
                    self.last_el = None;
                    return PaddleEvent::None;
                };
                let dur = if send_dit { self.el_dur } else { self.el_dur * 3 };
                self.el_end  = now + dur + self.el_dur;
                self.last_el = Some(!send_dit);
                if send_dit { PaddleEvent::DitDown } else { PaddleEvent::DahDown }
            }

            PaddleMode::IambicB => {
                let dit_edge = dit_pressed && !self.prev_dit;
                let dah_edge = dah_pressed && !self.prev_dah;
                self.prev_dit = dit_pressed;
                self.prev_dah = dah_pressed;
                if dit_pressed && dah_pressed  { self.squeeze_active = true;  }
                if !dit_pressed && !dah_pressed { self.squeeze_active = false; }
                if dit_edge { self.dit_mem = true; }
                if dah_edge { self.dah_mem = true; }
                if now < self.el_end {
                    match self.last_el {
                        Some(true)  => { if dit_pressed { self.dit_mem = true; } }
                        Some(false) => { if dah_pressed { self.dah_mem = true; } }
                        None        => {}
                    }
                    return PaddleEvent::None;
                }
                if dit_pressed { self.dit_mem = true; }
                if dah_pressed { self.dah_mem = true; }
                let send_dit = if dit_pressed && dah_pressed {
                    let s = match self.last_el { None => true, Some(was_dah) => was_dah };
                    if s { self.dit_mem = false; } else { self.dah_mem = false; }
                    s
                } else if self.dit_mem {
                    self.dit_mem = false; true
                } else if self.dah_mem {
                    self.dah_mem = false; false
                } else {
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

// ── Detection helpers ─────────────────────────────────────────────────────────

/// Check if the VBand adapter is plugged in (any backend).
/// Uses sysfs on Linux (no permission needed).  Uses hidapi / rusb elsewhere.
pub fn is_vband_present() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/bus/usb/devices") {
            for entry in entries.flatten() {
                let p = entry.path();
                let vid = std::fs::read_to_string(p.join("idVendor")).unwrap_or_default();
                let pid = std::fs::read_to_string(p.join("idProduct")).unwrap_or_default();
                if vid.trim() == "413d" && pid.trim() == "2107" { return true; }
            }
        }
        false
    }

    #[cfg(not(target_os = "linux"))]
    {
        if HidApi::new()
            .map(|api| api.device_list().any(|d| d.vendor_id() == VBAND_VID && d.product_id() == VBAND_PID))
            .unwrap_or(false)
        {
            return true;
        }

        #[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
        if rusb::open_device_with_vid_pid(VBAND_VID, VBAND_PID).is_some() {
            return true;
        }

        false
    }
}

/// List connected VBand / compatible HID adapters (for --list-ports output).
pub fn list_vband_devices() -> Vec<String> {
    let mut out = Vec::new();

    // HidApi enumeration
    if let Ok(api) = HidApi::new() {
        for d in api.device_list()
            .filter(|d| d.vendor_id() == VBAND_VID && d.product_id() == VBAND_PID)
        {
            out.push(format!(
                "VBand HID {:04x}:{:04x}  [HidApi]  {}",
                d.vendor_id(), d.product_id(), d.path().to_string_lossy()
            ));
        }
    }

    // WinUSB enumeration (Windows + feature only)
    #[cfg(all(feature = "keyer-vband-winusb", target_os = "windows"))]
    if let Ok(devices) = rusb::devices() {
        for d in devices.iter() {
            if let Ok(desc) = d.device_descriptor() {
                if desc.vendor_id() == VBAND_VID && desc.product_id() == VBAND_PID {
                    let bus_addr = format!("bus={} addr={}", d.bus_number(), d.address());
                    // Only list here if NOT already found by hidapi (avoid duplicates)
                    let already_listed = out.iter().any(|s: &String| s.contains("HidApi"));
                    if !already_listed {
                        out.push(format!(
                            "VBand HID {:04x}:{:04x}  [WinUSB]  {bus_addr}",
                            VBAND_VID, VBAND_PID
                        ));
                    }
                }
            }
        }
    }

    out
}

// ── Interactive adapter check ─────────────────────────────────────────────────

/// Print a platform-specific hint after a failed check, based on how many
/// zero-data reads we accumulated (high count = device open but silent).
fn print_check_hint(zero_reads: u32) {
    // High zero_reads means the device opened and polled fine but returned
    // nothing — typical symptom of a permission gate or driver block.
    if zero_reads > 500 {
        #[cfg(target_os = "macos")]
        println!(
            "  macOS hint: the device opened but returned no data.\
             \n  This is almost always an Input Monitoring permission problem.\
             \n  → System Settings → Privacy & Security → Input Monitoring\
             \n  → Add your terminal app (Terminal.app, iTerm2, …)\
             \n  → Quit and re-launch the terminal, then run this test again."
        );
        #[cfg(target_os = "windows")]
        println!(
            "  Windows hint: the device opened but returned no data.\
             \n  Running as WinKbd shim — make sure no other software holds\
             \n  exclusive access to LCtrl / RCtrl keys (e.g. macro apps)."
        );
    }
}

/// Open the VBand, wait for each paddle in turn.
/// Returns `Ok(true)` if both paddles pass within `timeout`.
pub fn check_adapter(timeout: Duration) -> anyhow::Result<bool> {
    let device = match open_device() {
        Ok(d) => d,
        Err(e) => {
            println!("✗ VBand not found ({VBAND_VID:04x}:{VBAND_PID:04x}): {e}");
            return Ok(false);
        }
    };

    let backend = device.backend_name();
    println!("Adapter : VBand HID {:04x}:{:04x}  [{backend}]", VBAND_VID, VBAND_PID);
    #[cfg(target_os = "windows")]
    if matches!(device, VBandDevice::WinKbd { .. }) {
        println!("Protocol: Windows keyboard shim  DIT=LCtrl  DAH=RCtrl");
    } else {
        println!("Protocol: HID bitmask  DIT=0x{DIT_MASK:02X}  DAH=0x{DAH_MASK:02X}");
    }
    #[cfg(not(target_os = "windows"))]
    println!("Protocol: HID bitmask  DIT=0x{DIT_MASK:02X}  DAH=0x{DAH_MASK:02X}");
    println!();

    let mut dit_ok = false;
    let mut dah_ok = false;
    let mut buf = [0u8; 64];
    let mut zero_read_count = 0u32;

    // Step 1: DIT
    println!("[ 1/2 ]  Press DIT paddle now …");
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match device.read_raw(&mut buf) {
            ReadResult::Report(mask) => {
                zero_read_count = 0;
                let dit = (mask & DIT_MASK) != 0;
                let dah = (mask & DAH_MASK) != 0;
                log::debug!("[vband-check] mask=0x{mask:02X} dit={dit} dah={dah}");
                if dit {
                    println!("         ✓ DIT received  (mask=0x{mask:02X})");
                    dit_ok = true;
                    break;
                } else if dah {
                    println!("         ✗ Got DAH instead of DIT — paddles may be swapped, try --switch-paddle");
                }
            }
            ReadResult::NoData => { zero_read_count += 1; }
            ReadResult::Error  => {}
        }
    }
    if !dit_ok {
        println!("         ✗ DIT timeout — no DIT event received");
        print_check_hint(zero_read_count);
    }

    // Step 2: DAH
    zero_read_count = 0;
    println!("[ 2/2 ]  Press DAH paddle now …");
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match device.read_raw(&mut buf) {
            ReadResult::Report(mask) => {
                zero_read_count = 0;
                let dit = (mask & DIT_MASK) != 0;
                let dah = (mask & DAH_MASK) != 0;
                log::debug!("[vband-check] mask=0x{mask:02X} dit={dit} dah={dah}");
                if dah {
                    println!("         ✓ DAH received  (mask=0x{mask:02X})");
                    dah_ok = true;
                    break;
                } else if dit {
                    println!("         ✗ Got DIT instead of DAH — paddles may be swapped, try --switch-paddle");
                }
            }
            ReadResult::NoData => { zero_read_count += 1; }
            ReadResult::Error  => {}
        }
    }
    if !dah_ok {
        println!("         ✗ DAH timeout — no DAH event received");
        print_check_hint(zero_read_count);
    }

    println!();
    if dit_ok && dah_ok {
        println!("✓ VBand adapter OK — both paddles working");
        Ok(true)
    } else {
        println!("✗ Adapter check FAILED  (DIT: {}  DAH: {})",
            if dit_ok { "OK" } else { "FAIL" },
            if dah_ok { "OK" } else { "FAIL" });
        Ok(false)
    }
}
