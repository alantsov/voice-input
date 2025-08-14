#!/bin/bash
set -e

echo "Building Debian package for voice-input..."
# Use -d flag to override build dependencies check
dpkg-buildpackage -us -uc -b -d

if ls ../voice-input_0.1.4-1_*.deb >/dev/null 2>&1; then
    echo "Debian package built successfully!"
    echo "You can install it with: sudo dpkg -i ../voice-input_0.1.4-1_*.deb"
    echo "If apt suggests installing NVIDIA compute package, you can accept it; otherwise the app will run on CPU."
    echo "To fix any missing dependencies: sudo apt-get -f install"
else
    echo "Failed to build Debian package."
    exit 1
fi
