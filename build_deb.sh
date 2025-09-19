#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

ARTIFACTS_DIR="dist"
BUILD_SRC_DIR="$ARTIFACTS_DIR/build-src"

mkdir -p "$ARTIFACTS_DIR"
rm -rf "$BUILD_SRC_DIR"

# Prepare a build copy inside ./dist so dpkg-buildpackage writes artifacts into ./dist (its parent)
if ! command -v rsync >/dev/null 2>&1; then
  echo "rsync is required. Install with: sudo apt-get install rsync" >&2
  exit 1
fi

# Copy the source tree excluding heavy/irrelevant directories
rsync -a \
  --exclude '.git' \
  --exclude 'target' \
  --exclude 'dist' \
  --exclude 'build' \
  --exclude 'ggml/src/ggml-cuda/CMakeFiles' \
  ./ "$BUILD_SRC_DIR"/

echo "Building Debian package for voice-input..."
# Change to build src; dpkg-buildpackage will place artifacts in the parent dir (./dist)
(
  cd "$BUILD_SRC_DIR"
  # Use -d flag to override build dependencies check (CI provides deps)
  dpkg-buildpackage -us -uc -b -d
)

# Find the latest built .deb in dist
DEB_FILE=$(ls -1t "$ARTIFACTS_DIR"/voice-input_*_amd64*.deb 2>/dev/null | head -1 || true)

if [[ -n "$DEB_FILE" ]]; then
    echo "Debian package built successfully!"
    echo "Artifact(s) are in: $ARTIFACTS_DIR"
    echo "You can install it with: sudo dpkg -i \"$DEB_FILE\""
    echo "If apt suggests installing NVIDIA compute package, you can accept it; otherwise the app will run on CPU."
    echo "To fix any missing dependencies: sudo apt-get -f install"
else
    echo "Failed to build Debian package."
    echo "Check build logs above."
    exit 1
fi
