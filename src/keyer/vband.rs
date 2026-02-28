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
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicBool;
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

// ── macOS IOKit IOHIDManager seize backend ────────────────────────────────────
//
// On macOS 14+ (Sonoma / Sequoia) the kernel's IOHIDDriver holds keyboard-class
// HID devices (UsagePage:0001 Usage:0006) with kIOHIDDriverExclusive.  hidapi's
// IOHIDDeviceOpen(kIOHIDOptionsTypeNone) is rejected with kIOReturnNotPrivileged
// (0xE00002C1) even when Input Monitoring TCC permission is GRANTED.
//
// Fix: use IOHIDManagerOpen with kIOHIDOptionsTypeSeizeDevice (0x01).  This
// takes the device from IOHIDDriver, allowing us to receive raw HID reports via
// an IOHIDReportCallback on a private CFRunLoop thread.
//
// The seize is released automatically when we close the manager on Drop.
// While seized, the VBand will NOT generate OS-level LCtrl/RCtrl key events —
// we read the raw HID modifier byte directly, which is exactly what we need.
//
// Requirements:
//   • Input Monitoring (TCC) permission must be GRANTED for the calling process.
//   • The VBand must be enumerable (visible in HID device list).

#[cfg(target_os = "macos")]
mod mac_iohid {
    use std::ffi::{c_void, CString};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use std::sync::mpsc;

    // ── Opaque CoreFoundation / IOKit handle types ────────────────────────
    type CFTypeRef        = *mut c_void;
    type CFStringRef      = *mut c_void;
    type CFNumberRef      = *mut c_void;
    type CFDictionaryRef  = *mut c_void;
    type CFRunLoopRef     = *mut c_void;
    type CFIndex          = isize;
    type CFTimeInterval   = f64;
    type CFStringEncoding = u32;
    type IOReturn         = i32;
    type IOOptionBits     = u32;
    type IOHIDManagerRef  = *mut c_void;

    const K_IO_RETURN_SUCCESS:         IOReturn         = 0;
    const K_IO_HID_OPTIONS_NONE:       IOOptionBits     = 0x00;
    const K_IO_HID_OPTIONS_SEIZE:      IOOptionBits     = 0x01;
    const K_CF_STRING_ENCODING_UTF8:   CFStringEncoding = 0x0800_0100;
    const K_CF_NUMBER_INT_TYPE:        i64              = 9; // kCFNumberIntType

    // Signature for IOHIDManager input-report callback
    type ReportCb = unsafe extern "C" fn(
        context:       *mut c_void,
        result:        IOReturn,
        sender:        *mut c_void,
        report_type:   u32,
        report_id:     u32,
        report:        *const u8,
        report_length: CFIndex,
    );

    #[link(name = "IOKit",          kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn IOHIDManagerCreate(
            allocator: CFTypeRef,
            options:   IOOptionBits,
        ) -> IOHIDManagerRef;

        fn IOHIDManagerSetDeviceMatching(
            manager:  IOHIDManagerRef,
            matching: CFDictionaryRef,
        );

        fn IOHIDManagerRegisterInputReportCallback(
            manager:  IOHIDManagerRef,
            callback: ReportCb,
            context:  *mut c_void,
        );

        fn IOHIDManagerScheduleWithRunLoop(
            manager:       IOHIDManagerRef,
            run_loop:      CFRunLoopRef,
            run_loop_mode: CFStringRef,
        );

        fn IOHIDManagerOpen(
            manager: IOHIDManagerRef,
            options: IOOptionBits,
        ) -> IOReturn;

        fn IOHIDManagerClose(
            manager: IOHIDManagerRef,
            options: IOOptionBits,
        ) -> IOReturn;

        fn CFRunLoopGetCurrent() -> CFRunLoopRef;

        fn CFRunLoopRunInMode(
            mode:                        CFStringRef,
            seconds:                     CFTimeInterval,
            return_after_source_handled: u8,
        ) -> i32;

        fn CFStringCreateWithCString(
            alloc:    CFTypeRef,
            c_str:    *const i8,
            encoding: CFStringEncoding,
        ) -> CFStringRef;

        fn CFNumberCreate(
            allocator: CFTypeRef,
            the_type:  i64,
            value_ptr: *const c_void,
        ) -> CFNumberRef;

        fn CFDictionaryCreate(
            allocator:       CFTypeRef,
            keys:            *const CFTypeRef,
            values:          *const CFTypeRef,
            num_values:      CFIndex,
            key_callbacks:   *const c_void,
            value_callbacks: *const c_void,
        ) -> CFDictionaryRef;

        fn CFRelease(cf: CFTypeRef);

        // CoreFoundation exported constants
        static kCFRunLoopDefaultMode:            CFStringRef;
        static kCFTypeDictionaryKeyCallBacks:    c_void;
        static kCFTypeDictionaryValueCallBacks:  c_void;
    }

    // ── Shared state ──────────────────────────────────────────────────────

    /// State shared between the IOHIDManager run-loop thread and the polling thread.
    pub struct MacCtx {
        /// Latest paddle bitmask from HID reports (DIT_MASK=0x01 | DAH_MASK=0x10).
        pub raw_mask: AtomicU8,
        /// Set to `true` by `Drop` to signal the run-loop thread to exit.
        pub stop:     AtomicBool,
    }

    // ── Report callback ───────────────────────────────────────────────────

    /// IOHIDManager input-report callback — runs on the background CFRunLoop thread.
    ///
    /// Extracts the paddle bitmask from the report and stores it in `MacCtx::raw_mask`.
    /// Byte selection follows the same logic as the HidApi backend:
    ///   buf[0] != 0  → use buf[0]  (Linux/macOS raw layout)
    ///   buf[0] == 0  → use buf[1]  (Windows report-ID prepend fallback, unlikely here)
    unsafe extern "C" fn report_cb(
        context:       *mut c_void,
        _result:       IOReturn,
        _sender:       *mut c_void,
        _report_type:  u32,
        _report_id:    u32,
        report:        *const u8,
        report_length: CFIndex,
    ) {
        if context.is_null() || report.is_null() || report_length < 1 { return; }
        let ctx  = &*(context as *const MacCtx);
        let b0   = *report.add(0);
        let b1   = if report_length >= 2 { *report.add(1) } else { 0 };
        let mask = if b0 != 0 { b0 } else { b1 };
        ctx.raw_mask.store(mask, Ordering::Relaxed);
        log::debug!(
            "[vband/mackbd] report len={report_length} \
             b0=0x{b0:02X} b1=0x{b1:02X} → mask=0x{mask:02X}"
        );
    }

    // ── Thread body ───────────────────────────────────────────────────────

    /// Core of the background thread: creates the IOHIDManager, opens it with
    /// kIOHIDOptionsTypeSeizeDevice, registers `report_cb`, and runs the CFRunLoop.
    ///
    /// SAFETY: all IOKit/CoreFoundation calls are on the same thread that owns
    /// the run loop.  The `ctx` Arc pointer passed to `report_cb` remains valid
    /// for the entire lifetime of the run loop (the Arc refcount is >= 1 because
    /// this thread holds a clone).
    unsafe fn run_loop_thread(
        vid: u16,
        pid: u16,
        ctx: Arc<MacCtx>,
        tx:  mpsc::Sender<anyhow::Result<()>>,
    ) {
        // 1. Create manager
        let mgr = IOHIDManagerCreate(std::ptr::null_mut(), K_IO_HID_OPTIONS_NONE);
        if mgr.is_null() {
            let _ = tx.send(Err(anyhow::anyhow!("IOHIDManagerCreate returned NULL")));
            return;
        }

        // 2. Build matching dict { "VendorID": vid, "ProductID": pid }
        let k_vid = CString::new("VendorID").unwrap();
        let k_pid = CString::new("ProductID").unwrap();
        let cf_kv = CFStringCreateWithCString(std::ptr::null_mut(), k_vid.as_ptr(), K_CF_STRING_ENCODING_UTF8);
        let cf_kp = CFStringCreateWithCString(std::ptr::null_mut(), k_pid.as_ptr(), K_CF_STRING_ENCODING_UTF8);
        let v_vid = vid as i32;
        let v_pid = pid as i32;
        let cf_vv = CFNumberCreate(std::ptr::null_mut(), K_CF_NUMBER_INT_TYPE, &v_vid as *const i32 as *const c_void);
        let cf_vp = CFNumberCreate(std::ptr::null_mut(), K_CF_NUMBER_INT_TYPE, &v_pid as *const i32 as *const c_void);
        let keys: [CFTypeRef; 2] = [cf_kv as _, cf_kp as _];
        let vals: [CFTypeRef; 2] = [cf_vv as _, cf_vp as _];
        let dict = CFDictionaryCreate(
            std::ptr::null_mut(),
            keys.as_ptr(),
            vals.as_ptr(),
            2,
            &kCFTypeDictionaryKeyCallBacks   as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const c_void,
        );
        IOHIDManagerSetDeviceMatching(mgr, dict);
        CFRelease(dict as CFTypeRef);
        CFRelease(cf_kv as CFTypeRef);
        CFRelease(cf_kp as CFTypeRef);
        CFRelease(cf_vv as CFTypeRef);
        CFRelease(cf_vp as CFTypeRef);

        // 3. Register callback; pass raw pointer to shared ctx
        //    Safety: Arc keeps the data alive as long as the thread runs.
        let ctx_ptr = Arc::as_ptr(&ctx) as *mut c_void;
        IOHIDManagerRegisterInputReportCallback(mgr, report_cb, ctx_ptr);

        // 4. Schedule with this thread's run loop
        let rl   = CFRunLoopGetCurrent();
        let mode = kCFRunLoopDefaultMode;
        IOHIDManagerScheduleWithRunLoop(mgr, rl, mode);

        // 5. Open with seize — takes the device from IOHIDDriver
        let ret = IOHIDManagerOpen(mgr, K_IO_HID_OPTIONS_SEIZE);
        if ret != K_IO_RETURN_SUCCESS {
            let _ = tx.send(Err(anyhow::anyhow!(
                "IOHIDManagerOpen(kIOHIDOptionsTypeSeizeDevice) → 0x{ret:08X}"
            )));
            IOHIDManagerClose(mgr, K_IO_HID_OPTIONS_NONE);
            CFRelease(mgr as CFTypeRef);
            return;
        }

        log::info!(
            "[vband/mackbd] IOHIDManager opened with kIOHIDOptionsTypeSeizeDevice \
             — VBand {:04x}:{:04x} seized from IOHIDDriver",
            vid, pid
        );
        let _ = tx.send(Ok(()));

        // 6. Drive the run loop in 10 ms slices, stopping when requested
        loop {
            if ctx.stop.load(Ordering::Relaxed) { break; }
            CFRunLoopRunInMode(mode, 0.010, 0);
        }

        IOHIDManagerClose(mgr, K_IO_HID_OPTIONS_NONE);
        CFRelease(mgr as CFTypeRef);
        log::debug!("[vband/mackbd] run-loop thread exiting");
    }

    // ── Public entry point ────────────────────────────────────────────────

    /// Spawn the IOHIDManager seize thread for the given VID:PID.
    ///
    /// Blocks until the manager is open (or returns `Err` if open failed).
    pub fn spawn(
        vid: u16,
        pid: u16,
    ) -> anyhow::Result<(Arc<MacCtx>, std::thread::JoinHandle<()>)> {
        let ctx  = Arc::new(MacCtx {
            raw_mask: AtomicU8::new(0),
            stop:     AtomicBool::new(false),
        });
        let ctx2 = Arc::clone(&ctx);

        let (tx, rx) = mpsc::channel::<anyhow::Result<()>>();

        let thread = std::thread::Builder::new()
            .name("vband-mackbd".into())
            .spawn(move || unsafe { run_loop_thread(vid, pid, ctx2, tx) })?;

        // Block until the background thread signals open success or failure
        rx.recv()
            .map_err(|_| anyhow::anyhow!("[vband/mackbd] thread died before signalling"))??;

        Ok((ctx, thread))
    }
}

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
    /// macOS IOHIDManager seize backend — used on macOS 14+ (Sonoma/Sequoia) when
    /// IOHIDDriver holds the keyboard-class device exclusively and blocks hidapi.
    /// Opens via kIOHIDOptionsTypeSeizeDevice on a private CFRunLoop thread.
    #[cfg(target_os = "macos")]
    MacKbd {
        ctx:    std::sync::Arc<mac_iohid::MacCtx>,
        prev:   std::cell::Cell<u8>,
        thread: Option<std::thread::JoinHandle<()>>,
    },
}

impl Drop for VBandDevice {
    fn drop(&mut self) {
        // Signal the macOS run-loop thread to stop, then join it so the
        // IOHIDManager is closed before the Arc<MacCtx> is released.
        #[cfg(target_os = "macos")]
        if let VBandDevice::MacKbd { ctx, thread, .. } = self {
            ctx.stop.store(true, Ordering::Relaxed);
            if let Some(t) = thread.take() {
                let _ = t.join();
            }
        }
    }
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

            // ── macOS IOHIDManager seize shim ─────────────────────────────
            // The background run-loop thread writes the latest paddle bitmask
            // into ctx.raw_mask via report_cb.  We poll it here and report
            // only on change (same pattern as WinKbd).
            #[cfg(target_os = "macos")]
            VBandDevice::MacKbd { ctx, prev, .. } => {
                let mask = ctx.raw_mask.load(Ordering::Relaxed);
                let old  = prev.get();
                if mask != old {
                    prev.set(mask);
                    log::debug!("[vband/mackbd] mask changed 0x{old:02X} → 0x{mask:02X}");
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
            #[cfg(target_os = "macos")]
            VBandDevice::MacKbd { .. } => "macOS IOKit (IOHIDManager seize)",
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
    // Track whether the VBand is enumerable at all (device plugged in).
    // Used by the macOS seize fallback to decide whether to attempt a seize.
    #[cfg(target_os = "macos")]
    let mut vband_seen = false;

    // ── 1. HidApi ─────────────────────────────────────────────────────────
    //
    // On Windows the VBand exposes a \KBD top-level collection owned by
    // kbdhid.sys.  ReadFile on that interface always returns nothing.
    // If a non-\KBD (generic HID) path exists we prefer it; otherwise we
    // fall through to the WinKbd shim below.
    //
    // On macOS 14+ this open will FAIL with kIOReturnNotPrivileged even when
    // Input Monitoring is GRANTED — the MacKbd seize fallback below fixes this.
    if let Ok(api) = HidApi::new() {
        let all_paths: Vec<_> = api.device_list()
            .filter(|d| d.vendor_id() == VBAND_VID && d.product_id() == VBAND_PID)
            .map(|d| d.path().to_owned())
            .collect();

        #[cfg(target_os = "macos")]
        if !all_paths.is_empty() { vband_seen = true; }

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
            log::debug!("[vband] {} HID path(s) found but none openable via HidApi", all_paths.len());
        }
    }

    // ── 2. macOS IOHIDManager seize fallback (macOS only) ─────────────────
    //
    // On macOS 14+ (Sonoma / Sequoia) IOHIDDriver claims keyboard-class HID
    // devices exclusively.  hidapi's IOHIDDeviceOpen(kIOHIDOptionsTypeNone)
    // is rejected at the kernel level.  We retry with IOHIDManagerOpen using
    // kIOHIDOptionsTypeSeizeDevice, which takes the device from IOHIDDriver.
    // This requires Input Monitoring TCC permission (same as hidapi).
    //
    // The seize is released in Drop → MacKbd thread stop + join.
    #[cfg(target_os = "macos")]
    if vband_seen {
        log::info!(
            "[vband] HidApi open failed on macOS — trying IOHIDManager seize \
             (kIOHIDOptionsTypeSeizeDevice) …"
        );
        match mac_iohid::spawn(VBAND_VID, VBAND_PID) {
            Ok((ctx, thread)) => {
                log::info!(
                    "[vband] VBand {:04x}:{:04x} opened via macOS IOHIDManager seize — \
                     IOHIDDriver exclusive hold bypassed.",
                    VBAND_VID, VBAND_PID
                );
                return Ok(VBandDevice::MacKbd {
                    ctx,
                    prev:   std::cell::Cell::new(0),
                    thread: Some(thread),
                });
            }
            Err(e) => log::warn!("[vband] IOHIDManager seize failed: {e}"),
        }
    }

    // ── 3. WinKbd shim (Windows only) ─────────────────────────────────────
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

    // ── 4. WinUSB fallback (Windows, feature "keyer-vband-winusb") ────────
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
        "\n  Hint: macOS requires 'Input Monitoring' permission for HID keyboard devices.\
         \n  → System Settings → Privacy & Security → Input Monitoring\
         \n  → Add your terminal app (Terminal.app, iTerm2, …) and re-launch it.\
         \n  On macOS 14+ (Sonoma/Sequoia): cw-qso-sim uses IOHIDManager with\
         \n  kIOHIDOptionsTypeSeizeDevice to bypass the kernel exclusive hold.\
         \n  If the seize also failed, the VBand may not be plugged in, or Input\
         \n  Monitoring permission has not been granted to this process.\
         \n  Check: Apple menu → About This Mac → System Report → USB."
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
    #[cfg(target_os = "macos")]
    if matches!(device, VBandDevice::MacKbd { .. }) {
        println!("Protocol: macOS IOKit seize (IOHIDManager)  DIT=LCtrl  DAH=RCtrl");
    } else {
        println!("Protocol: HID bitmask  DIT=0x{DIT_MASK:02X}  DAH=0x{DAH_MASK:02X}");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
