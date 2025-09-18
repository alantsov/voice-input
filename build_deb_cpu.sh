#!/usr/bin/env bash
set -euo pipefail

# Build a CPU-only Debian package for voice-input (no CUDA, no GTK tray)
# Result: ../voice-input_<version>_amd64_cpu.deb

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required. Install with: sudo apt-get install dpkg-dev debhelper fakeroot" >&2
  exit 1
fi

# Extract version from Cargo.toml
VERSION=$(sed -n 's/^version\s*=\s*"\(.*\)"/\1/p' Cargo.toml | head -n1)
if [[ -z "${VERSION}" ]]; then
  echo "Unable to parse version from Cargo.toml" >&2
  exit 1
fi

# Build CPU-only binary with tray icon feature
export RUSTFLAGS="${RUSTFLAGS:-}"
echo "Building voice_input (CPU-only, with tray icon)..."
cargo build --release --features tray-icon

BIN_PATH="target/release/voice_input"
if [[ ! -x "$BIN_PATH" ]]; then
  echo "Build failed: $BIN_PATH not found" >&2
  exit 1
fi

# Staging directories
STAGE_ROOT="build/cpu-deb"
PKG_DIR="$STAGE_ROOT/voice-input"
DEBIAN_DIR="$PKG_DIR/DEBIAN"
BIN_DIR="$PKG_DIR/usr/bin"
APPS_DIR="$PKG_DIR/usr/share/applications"
ICONS_DIR="$PKG_DIR/usr/share/icons/hicolor"
AUTOSTART_DIR="$PKG_DIR/etc/xdg/autostart"

rm -rf "$STAGE_ROOT"
mkdir -p "$DEBIAN_DIR" "$BIN_DIR" "$APPS_DIR" "$ICONS_DIR" "$AUTOSTART_DIR"

# Install binary
install -m 0755 "$BIN_PATH" "$BIN_DIR/voice-input"

# Install desktop entry (if present)
if [[ -f "voice-input.desktop" ]]; then
  install -m 0644 "voice-input.desktop" "$APPS_DIR/voice-input.desktop"
  # Also provide autostart entry
  install -m 0644 "voice-input.desktop" "$AUTOSTART_DIR/voice-input.desktop"
fi

# Install icons from assets (if present)
if [[ -d assets/icons/hicolor ]]; then
  # Copy the whole hicolor tree
  mkdir -p "$ICONS_DIR"
  cp -a assets/icons/hicolor/. "$ICONS_DIR/"
fi

# Compute Installed-Size in KiB
INSTALLED_SIZE=$(du -sk "$PKG_DIR" | awk '{print $1}')

# Create control file (CPU-only with GTK tray UI, no CUDA)
cat >"$DEBIAN_DIR/control" <<CONTROL
Package: voice-input
Version: ${VERSION}
Architecture: amd64
Maintainer: Voice Input Maintainers <noreply@example.com>
Section: utils
Priority: optional
Homepage: https://github.com/example/voice-input
Installed-Size: ${INSTALLED_SIZE}
Depends: libasound2 (>= 1.0.29), libc6 (>= 2.34), libcairo-gobject2 (>= 1.10.0), libcairo2 (>= 1.2.4), libgcc-s1 (>= 4.2), libgdk-pixbuf-2.0-0 (>= 2.22.0), libglib2.0-0 (>= 2.35.8), libgtk-3-0 (>= 3.0.0), libpango-1.0-0 (>= 1.14.0), libssl3 (>= 3.0.0~~alpha1), libstdc++6 (>= 11), libx11-6, libxtst6, libxdo3, libatk1.0-0, libayatana-appindicator3-1, libxcb-shape0, libxcb-xfixes0
Description: Voice Input Application (CPU-only build with tray icon)
 A simple application for recording voice input using the microphone.
 .
 Features:
  * Press Ctrl+CAPSLOCK to start/stop recording
  * Saves WAV files with timestamps
  * Transcribes audio and inserts text at cursor position
  * CPU-only build with GTK-based tray icon (no CUDA)
CONTROL

# Set permissions for DEBIAN metadata
chmod 0755 "$DEBIAN_DIR"
find "$DEBIAN_DIR" -type f -exec chmod 0644 {} +

# Build the .deb
OUT_PATH="../voice-input_${VERSION}_amd64_cpu.deb"
echo "Building package: $OUT_PATH"
fakeroot dpkg-deb --build "$PKG_DIR" "$OUT_PATH"

echo
echo "Package created: $OUT_PATH"
echo "Install with: sudo dpkg -i '$OUT_PATH' && sudo apt-get -f install"
echo "Note: This package is CPU-only (no CUDA) and includes the GTK-based tray icon; GTK/AppIndicator libraries are required at runtime."
