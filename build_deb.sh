#!/bin/bash
set -e

echo "Building Debian package for voice-input..."
# Use -d flag to override build dependencies check
dpkg-buildpackage -us -uc -b -d

if [ -f ../voice-input_0.1.1-1_*.deb ]; then
    echo "Debian package built successfully!"
    echo "You can install it with: sudo dpkg -i ../voice-input_0.1.2-1_*.deb"
    echo "And install any missing dependencies with: sudo apt-get install -f"
else
    echo "Failed to build Debian package."
    exit 1
fi
