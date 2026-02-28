#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-cross.sh  —  Cross-compile cw-qso-sim for all supported targets
#
# Requirements (all targets):
#   cargo install cross --git https://github.com/cross-rs/cross
#   docker  (or podman — set CROSS_CONTAINER_ENGINE=podman)
#
# Additional requirements (macOS targets when building from Linux):
#   cargo install cargo-zigbuild
#   rustup target add x86_64-apple-darwin aarch64-apple-darwin
#
#   The macOS SDK is downloaded automatically on first run from:
#     https://github.com/alexey-lysiuk/macos-sdk/releases/download/14.5/MacOSX14.5.tar.xz
#   and cached in ./macos-sdk/  for subsequent builds.
#   Override with: MACOS_SDK_PATH=/path/to/MacOSX.sdk
#
#   When running ON a macOS host the SDK is not needed; the system Xcode
#   toolchain is used directly via plain `cargo`.
#
# Features per target:
#   Linux x86_64    : full  (audio-cpal + keyer-vband + keyer-attiny85 + keyer-nano + keyer-winkeyer + tui)
#   Linux aarch64   : full  (audio-cpal + keyer-vband + keyer-attiny85 + keyer-nano + keyer-winkeyer + tui)
#   Linux armv7     : full  (audio-cpal + keyer-vband + keyer-attiny85 + keyer-nano + keyer-winkeyer + tui)
#   macOS x86_64    : full  (audio-cpal + keyer-vband + keyer-attiny85 + keyer-nano + keyer-winkeyer + tui)
#   macOS aarch64   : full  (audio-cpal + keyer-vband + keyer-attiny85 + keyer-nano + keyer-winkeyer + tui)
#   Windows GNU     : full  (audio-cpal + keyer-vband + keyer-attiny85 + keyer-nano + keyer-winkeyer + tui)
#
# keyer-nano     — Arduino Nano / Uno / ESP32 / ESP8266 serial-MIDI paddles (serialport crate)
# keyer-winkeyer — K1EL WinKeyer USB/Serial WK2/WK3 (1200 baud, serialport crate)
#
# Why --target-dir per cross target?
#   cross runs build-scripts (serde, parking_lot_core …) inside its Docker
#   container.  If those scripts were already compiled natively on the host
#   they live in target/release/build/ and may link against a newer glibc
#   than the container provides → "GLIBC_2.3x not found" build failure.
#   A per-target directory prevents any sharing between native and cross builds.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# Ensure ~/.local/bin (cargo-zigbuild's zig wrapper) is on PATH
export PATH="$HOME/.local/bin:$PATH"

BINARY="cw-qso-sim"
OUT_DIR="dist"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT_DIR"

# ── Dependency checks ─────────────────────────────────────────────────────────
check_dep() {
    if ! command -v "$1" &>/dev/null; then
        echo "Error: '$1' not found in PATH." >&2
        echo "  → $2" >&2
        exit 1
    fi
}
check_dep cargo "rustup — see https://rustup.rs"
check_dep cross "cargo install cross --git https://github.com/cross-rs/cross"

# ── Detect host triple via rustc ──────────────────────────────────────────────
HOST_TRIPLE=$(rustc -vV 2>/dev/null | sed -n 's|^host: ||p')
echo "Host triple: ${HOST_TRIPLE:-unknown}"

# ── macOS SDK — auto-download, extract, locate ───────────────────────────────
# Sets global SDKROOT; returns 0 on success, 1 on failure (target is skipped).
MACOS_SDK_URL="https://github.com/alexey-lysiuk/macos-sdk/releases/download/14.5/MacOSX14.5.tar.xz"
MACOS_SDK_ARCHIVE="${SCRIPT_DIR}/macos-sdk/MacOSX14.5.tar.xz"
MACOS_SDK_DIR="${SCRIPT_DIR}/macos-sdk"

find_macos_sdk() {
    # 1. Explicit override via environment
    if [[ -n "${MACOS_SDK_PATH:-}" ]]; then
        if [[ -d "$MACOS_SDK_PATH" ]]; then
            SDKROOT="$MACOS_SDK_PATH"
            return 0
        fi
        echo "  ⚠  MACOS_SDK_PATH='$MACOS_SDK_PATH' does not exist." >&2
        return 1
    fi

    # 2. Already extracted — use it
    local sdk
    sdk=$(find "$MACOS_SDK_DIR" -maxdepth 1 -name '*.sdk' -type d 2>/dev/null \
          | sort -V | tail -1) || sdk=""
    if [[ -n "$sdk" ]]; then
        SDKROOT="$sdk"
        return 0
    fi

    # 3. Archive present but not yet extracted
    if [[ -f "$MACOS_SDK_ARCHIVE" ]]; then
        echo "  Extracting $(basename "$MACOS_SDK_ARCHIVE") …"
        tar -xJf "$MACOS_SDK_ARCHIVE" -C "$MACOS_SDK_DIR"
        sdk=$(find "$MACOS_SDK_DIR" -maxdepth 1 -name '*.sdk' -type d 2>/dev/null \
              | sort -V | tail -1) || sdk=""
        if [[ -n "$sdk" ]]; then
            SDKROOT="$sdk"
            return 0
        fi
        echo "  ⚠  Extraction produced no *.sdk directory." >&2
        return 1
    fi

    # 4. Download + extract
    if ! command -v curl &>/dev/null && ! command -v wget &>/dev/null; then
        echo "  ⚠  Neither curl nor wget found — cannot download macOS SDK." >&2
        return 1
    fi

    echo "  Downloading macOS SDK from:"
    echo "    $MACOS_SDK_URL"
    mkdir -p "$MACOS_SDK_DIR"

    if command -v curl &>/dev/null; then
        curl -L --fail --progress-bar -o "$MACOS_SDK_ARCHIVE" "$MACOS_SDK_URL"
    else
        wget -q --show-progress -O "$MACOS_SDK_ARCHIVE" "$MACOS_SDK_URL"
    fi

    echo "  Extracting $(basename "$MACOS_SDK_ARCHIVE") …"
    tar -xJf "$MACOS_SDK_ARCHIVE" -C "$MACOS_SDK_DIR"

    sdk=$(find "$MACOS_SDK_DIR" -maxdepth 1 -name '*.sdk' -type d 2>/dev/null \
          | sort -V | tail -1) || sdk=""
    if [[ -n "$sdk" ]]; then
        SDKROOT="$sdk"
        return 0
    fi

    echo "  ⚠  SDK extraction failed or produced no *.sdk directory." >&2
    return 1
}

# ── Helper — Linux / Windows targets (via cross) ─────────────────────────────
build() {
    local target="$1"
    local ext="${2:-}"        # ".exe" for Windows, empty otherwise
    local features="${3:-audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui}"

    local tgt_dir
    if [[ "$target" == "$HOST_TRIPLE" ]]; then
        tgt_dir="target"
    else
        tgt_dir="target-${target}"
    fi

    echo ""
    echo "══════════════════════════════════════════════"
    echo "  Building  →  $target"
    echo "  Features  →  $features"
    echo "  TargetDir →  $tgt_dir"
    echo "══════════════════════════════════════════════"

    local cmd="cross"
    [[ "$target" == "$HOST_TRIPLE" ]] && cmd="cargo"

    $cmd build --release \
        --target      "$target" \
        --target-dir  "$tgt_dir" \
        --no-default-features \
        --features    "$features"

    local src="${tgt_dir}/${target}/release/${BINARY}${ext}"
    local dst="${OUT_DIR}/${BINARY}-${target}${ext}"
    cp "$src" "$dst"
    echo "  ✓  $dst  ($(du -sh "$dst" | cut -f1))"
}

# ── Helper — macOS targets ────────────────────────────────────────────────────
# On a macOS host: plain `cargo` with the Xcode toolchain.
# On Linux:        `cargo zigbuild` + macOS SDK sysroot (auto-downloaded).
#
# SDK env vars forwarded to C-binding crates:
#   SDKROOT                  — sysroot for cc-rs and clang
#   COREAUDIO_SDK_PATH       — coreaudio-sys/build.rs reads this directly
#   BINDGEN_EXTRA_CLANG_ARGS — passes -isysroot into bindgen's libclang
build_mac() {
    local target="$1"
    local features="${2:-audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui}"
    local tgt_dir="target-${target}"

    echo ""
    echo "══════════════════════════════════════════════"
    echo "  Building  →  $target"
    echo "  Features  →  $features"
    echo "  TargetDir →  $tgt_dir"
    echo "══════════════════════════════════════════════"

    if [[ "$HOST_TRIPLE" == *"-apple-"* ]]; then
        # ── Native macOS build ─────────────────────────────────────────────
        cargo build --release \
            --target      "$target" \
            --target-dir  "$tgt_dir" \
            --no-default-features \
            --features    "$features"
    else
        # ── Cross-compile from Linux via cargo-zigbuild ────────────────────
        if ! command -v cargo-zigbuild &>/dev/null; then
            echo "  ⚠  Skipping $target (cargo-zigbuild not found)" >&2
            echo "     → cargo install cargo-zigbuild" >&2
            echo "     → rustup target add $target" >&2
            return 0
        fi

        SDKROOT=""
        if ! find_macos_sdk; then
            echo "  ⚠  Skipping $target (macOS SDK unavailable)"
            return 0
        fi
        echo "  SDK       →  $SDKROOT"

        # cargo-zigbuild's generated zigcc wrapper does not inject -isysroot
        # or -F for the linker phase.  We wrap it: intercept the link step and
        # add the framework + lib paths from our SDK so zig can resolve
        # CoreAudio, CoreMIDI, IOKit, etc.
        local zig_cache_dir
        zig_cache_dir=$(dirname "$(ls "$HOME"/.cache/cargo-zigbuild/*/zigcc-${target}-*.sh 2>/dev/null | sort -V | tail -1)")
        local wrapper_dir="${SCRIPT_DIR}/target-${target}-linker"
        mkdir -p "$wrapper_dir"
        cat > "${wrapper_dir}/zig-link-wrapper.sh" << WRAPPER
#!/bin/sh
# Inject macOS SDK framework + lib search paths into every zig cc link call.
exec "${zig_cache_dir}/zigcc-${target}-"*.sh \\
    -F"${SDKROOT}/System/Library/Frameworks" \\
    -L"${SDKROOT}/usr/lib" \\
    "\$@"
WRAPPER
        chmod +x "${wrapper_dir}/zig-link-wrapper.sh"

        local linker_var="CARGO_TARGET_$(echo "$target" | tr '[:lower:]-' '[:upper:]_')_LINKER"

        # cc-rs reads CFLAGS_<target> (lowercase, dashes → underscores) and
        # injects those flags into every C compilation it drives.  Without
        # -isysroot the zig C front-end cannot find IOKit/hid/IOHIDManager.h,
        # CoreAudio/AudioHardware.h, etc. that live inside the macOS SDK.
        local cflags_var="CFLAGS_$(echo "$target" | tr '-' '_')"
        local sdk_cflags="-isysroot $SDKROOT -I$SDKROOT/usr/include -F$SDKROOT/System/Library/Frameworks"

        env \
            SDKROOT="$SDKROOT" \
            COREAUDIO_SDK_PATH="$SDKROOT" \
            BINDGEN_EXTRA_CLANG_ARGS="-isysroot $SDKROOT -F$SDKROOT/System/Library/Frameworks" \
            "${cflags_var}=${sdk_cflags}" \
            "${linker_var}=${wrapper_dir}/zig-link-wrapper.sh" \
        cargo zigbuild --release \
            --target      "$target" \
            --target-dir  "$tgt_dir" \
            --no-default-features \
            --features    "$features"
    fi

    local src="${tgt_dir}/${target}/release/${BINARY}"
    local dst="${OUT_DIR}/${BINARY}-${target}"
    cp "$src" "$dst"
    echo "  ✓  $dst  ($(du -sh "$dst" | cut -f1))"
}

# ── macOS cross-compile prerequisites — install tools + download SDK ──────────
# Called once before any builds.  On a native macOS host this is a no-op.
#
# Fixes addressed here:
#   • mkdir is created BEFORE the first `find` so the dir always exists —
#     previously `find` on a missing dir returned exit-code 1, which with
#     `set -euo pipefail` silently killed the script before any build ran.
#   • Every `sdk=$(find …)` carries `|| sdk=""` so a non-zero exit from find
#     (e.g. empty dir, permission issue) never triggers `set -e`.
#   • cargo-zigbuild and the two Darwin rustup targets are installed
#     automatically when missing, so macOS cross-builds actually work.
prepare_macos_sdk() {
    # No-op on a native macOS host — Xcode toolchain is used directly.
    if [[ "$HOST_TRIPLE" == *"-apple-"* ]]; then
        return 0
    fi

    echo ""
    echo "── macOS cross-compile setup ──────────────────────"

    # ── cargo-zigbuild ────────────────────────────────────────────────────────
    if ! command -v cargo-zigbuild &>/dev/null; then
        echo "  cargo-zigbuild not found — installing via cargo …"
        cargo install cargo-zigbuild
    else
        echo "  ✓  cargo-zigbuild $(cargo-zigbuild --version 2>/dev/null | head -1)"
    fi

    # ── rustup Darwin targets ─────────────────────────────────────────────────
    for darwin_target in x86_64-apple-darwin aarch64-apple-darwin; do
        if rustup target list --installed 2>/dev/null | grep -q "^${darwin_target}$"; then
            echo "  ✓  rustup target: ${darwin_target}"
        else
            echo "  Adding rustup target: ${darwin_target} …"
            rustup target add "${darwin_target}"
        fi
    done

    # ── macOS SDK ─────────────────────────────────────────────────────────────
    # Create the cache directory first — find on a missing path returns exit-code
    # 1, which would trigger set -e and kill the script before any download runs.
    mkdir -p "$MACOS_SDK_DIR"

    # 1. Already extracted — nothing to do
    local sdk
    sdk=$(find "$MACOS_SDK_DIR" -maxdepth 1 -name '*.sdk' -type d 2>/dev/null \
          | sort -V | tail -1) || sdk=""
    if [[ -n "$sdk" ]]; then
        echo "  ✓  SDK already extracted: $sdk"
        return 0
    fi

    # 2. Archive already present — just extract it
    if [[ -f "$MACOS_SDK_ARCHIVE" ]]; then
        echo "  Archive found — extracting $(basename "$MACOS_SDK_ARCHIVE") …"
        tar -xJf "$MACOS_SDK_ARCHIVE" -C "$MACOS_SDK_DIR"
        sdk=$(find "$MACOS_SDK_DIR" -maxdepth 1 -name '*.sdk' -type d 2>/dev/null \
              | sort -V | tail -1) || sdk=""
        if [[ -n "$sdk" ]]; then
            echo "  ✓  Extracted: $sdk"
        else
            echo "  ⚠  Extraction produced no *.sdk directory — macOS builds may fail." >&2
        fi
        return 0
    fi

    # 3. Download then extract
    if ! command -v curl &>/dev/null && ! command -v wget &>/dev/null; then
        echo "  ⚠  Neither curl nor wget found — cannot download macOS SDK." >&2
        echo "     Install curl or wget and re-run." >&2
        return 0
    fi

    echo "  Downloading MacOSX14.5.tar.xz …"
    echo "    URL : $MACOS_SDK_URL"
    echo "    Dest: $MACOS_SDK_ARCHIVE"

    if command -v curl &>/dev/null; then
        curl -L --fail --progress-bar -o "$MACOS_SDK_ARCHIVE" "$MACOS_SDK_URL"
    else
        wget -q --show-progress -O "$MACOS_SDK_ARCHIVE" "$MACOS_SDK_URL"
    fi

    echo "  Extracting $(basename "$MACOS_SDK_ARCHIVE") to $MACOS_SDK_DIR …"
    tar -xJf "$MACOS_SDK_ARCHIVE" -C "$MACOS_SDK_DIR"

    sdk=$(find "$MACOS_SDK_DIR" -maxdepth 1 -name '*.sdk' -type d 2>/dev/null \
          | sort -V | tail -1) || sdk=""
    if [[ -n "$sdk" ]]; then
        echo "  ✓  SDK ready: $sdk"
    else
        echo "  ⚠  Extraction produced no *.sdk directory — macOS builds may fail." >&2
    fi
}

prepare_macos_sdk

# ── Targets ───────────────────────────────────────────────────────────────────
echo "Starting cross-compile for cw-qso-sim …"

# Linux x86_64 — full features (native build when host is x86_64 Linux)
build "x86_64-unknown-linux-gnu" "" \
    "audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui"

# Linux ARM64
build "aarch64-unknown-linux-gnu" "" \
    "audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui"

# Linux ARMv7 — uncomment if needed
#build "armv7-unknown-linux-gnueabihf" "" \
#    "audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui"

# macOS x86_64 (Intel)
build_mac "x86_64-apple-darwin" \
    "audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui"

# macOS aarch64 (Apple Silicon)
build_mac "aarch64-apple-darwin" \
    "audio-cpal,keyer-vband,keyer-attiny85,keyer-nano,keyer-winkeyer,tui"

# Windows x86_64 — full features.
# keyer-vband-winusb adds a WinUSB/rusb fallback for devices where a libwdi
# driver (e.g. installed via Zadig) has replaced the native HidUsb driver.
# Requires cmake in the cross container for the vendored libusb build.
build "x86_64-pc-windows-gnu" ".exe" \
    "audio-cpal,keyer-vband,keyer-vband-winusb,keyer-attiny85,keyer-nano,keyer-winkeyer,tui"

echo ""
echo "══════════════════════════════════════════════"
echo "  All done!  Artifacts in ./$OUT_DIR/"
ls -lh "$OUT_DIR/"
echo "══════════════════════════════════════════════"
