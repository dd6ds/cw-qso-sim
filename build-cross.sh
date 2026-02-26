#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-cross.sh  —  Cross-compile cw-qso-sim for all supported targets
#
# Requirements:
#   cargo install cross --git https://github.com/cross-rs/cross
#   docker  (or podman — set CROSS_CONTAINER_ENGINE=podman)
#
# Features per target:
#   Linux x86_64    : full  (audio-cpal + keyer-vband + keyer-attiny85 + tui)
#   Windows GNU     : full  (audio-cpal + keyer-vband + keyer-attiny85 + tui)
#
# Why --target-dir per cross target?
#   cross runs build-scripts (serde, parking_lot_core …) inside its Docker
#   container.  If those scripts were already compiled natively on the host
#   they live in target/release/build/ and may link against a newer glibc
#   than the container provides → "GLIBC_2.3x not found" build failure.
#   A per-target directory prevents any sharing between native and cross builds.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

BINARY="cw-qso-sim"
OUT_DIR="dist"
mkdir -p "$OUT_DIR"

# ── Dependency checks ─────────────────────────────────────────────────────────
check_dep() {
    if ! command -v "$1" &>/dev/null; then
        echo "Error: '$1' not found in PATH." >&2
        echo "  → cargo install cross --git https://github.com/cross-rs/cross" >&2
        exit 1
    fi
}
check_dep cargo
check_dep cross

# ── Detect host triple via rustc ──────────────────────────────────────────────
HOST_TRIPLE=$(rustc -vV 2>/dev/null | sed -n 's|^host: ||p')
echo "Host triple: ${HOST_TRIPLE:-unknown}"

# ── Helper ────────────────────────────────────────────────────────────────────
build() {
    local target="$1"
    local ext="${2:-}"        # ".exe" for Windows, empty otherwise
    local features="${3:-audio-cpal,keyer-vband,tui}"

    # Each cross target gets its own target directory so build-script
    # binaries compiled inside the container are never mixed with host binaries.
    local tgt_dir
    if [[ "$target" == "$HOST_TRIPLE" ]]; then
        tgt_dir="target"        # native build uses the default dir
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

# ── Targets ───────────────────────────────────────────────────────────────────
echo "Starting cross-compile for cw-qso-sim …"

# Linux x86_64 — full features (native build)
build "x86_64-unknown-linux-gnu"

# Linux ARM — uncomment as needed
#build "aarch64-unknown-linux-gnu"
#build "armv7-unknown-linux-gnueabihf"

# Windows x86_64 — full features.
# keyer-vband-winusb adds a WinUSB/rusb fallback for devices where a libwdi
# driver (e.g. installed via Zadig) has replaced the native HidUsb driver.
# Requires cmake in the cross container for the vendored libusb build.
build "x86_64-pc-windows-gnu" ".exe" "audio-cpal,keyer-vband,keyer-vband-winusb,keyer-attiny85,tui"

echo ""
echo "══════════════════════════════════════════════"
echo "  All done!  Artifacts in ./$OUT_DIR/"
ls -lh "$OUT_DIR/"
echo "══════════════════════════════════════════════"
