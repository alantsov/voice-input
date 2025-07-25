#!/bin/bash
set -e

echo "Building Debian package for voice-input..."
dpkg-buildpackage -us -uc -b

if [ -f ../voice-input_0.1.0-1_*.deb ]; then
    echo "Debian package built successfully!"
    echo "You can install it with: sudo dpkg -i ../voice-input_0.1.0-1_*.deb"
    echo "And install any missing dependencies with: sudo apt-get install -f"
else
    echo "Failed to build Debian package."
    exit 1
fi