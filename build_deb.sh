#!/bin/bash
set -e

echo "Building Debian package for voice-input..."
# Use -d flag to override build dependencies check (CI provides deps)
dpkg-buildpackage -us -uc -b -d

# Find the latest built .deb
DEB_FILE=$(ls -1t ../voice-input_*_amd64.deb 2>/dev/null | head -1 || true)

if [ -n "$DEB_FILE" ]; then
    echo "Debian package built successfully!"
    echo "You can install it with: sudo dpkg -i \"$DEB_FILE\""
    echo "If apt suggests installing NVIDIA compute package, you can accept it; otherwise the app will run on CPU."
    echo "To fix any missing dependencies: sudo apt-get -f install"
else
    echo "Failed to build Debian package."
    exit 1
fi
