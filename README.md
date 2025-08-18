# Voice Input Application

A simple application for recording voice input using the microphone.

## Features

- Press Ctrl+CAPSLOCK to start recording, release to save
- Recordings are saved as WAV files with timestamps
- Transcribes audio and inserts text at cursor position
- Uses GPU acceleration for faster transcription when available
- Optional system tray icon support
- Single instance enforcement (prevents multiple instances from running simultaneously)

## Building

### Basic Build (No System Tray Icon)

```bash
cargo build
```

This will build the application without the system tray icon, which is useful if you don't have the GTK/ATK dependencies installed.

### Build with System Tray Icon

```bash
cargo build --features tray-icon
```

This will build the application with the system tray icon support. Note that this requires the GTK/ATK dependencies to be installed on your system.

## System Dependencies

The application requires several system dependencies:

1. GTK/ATK dependencies (only needed if you want to build with the system tray icon feature)
2. ALSA libraries for audio recording
3. libxdo for keyboard simulation (used to insert transcribed text at cursor position)

Install the following packages for your distribution:

### Ubuntu/Debian/Pop!_OS

For Ubuntu/Debian/Pop!_OS 22.04 and newer:

```bash
sudo apt-get install libgtk-3-dev libatk1.0-dev libcairo2-dev libayatana-appindicator3-dev
sudo apt install -y libasound2-dev
sudo apt install libclang-14-dev
sudo apt install libxdo-dev
sudo apt-get install -y libxcb-shape0-dev libxcb-xfixes0-dev
```

`sudo apt install debhelper cargo rustc libclang-dev`

For older Ubuntu/Debian versions (before 22.04):

```bash
sudo apt-get install libgtk-3-dev libatk1.0-dev libcairo2-dev libappindicator3-dev
sudo apt install -y libasound2-dev
sudo apt install libxdo-dev
```

### Fedora

```bash
sudo dnf install gtk3-devel atk-devel cairo-devel libappindicator-gtk3-devel
sudo dnf install -y alsa-lib-devel
sudo dnf install libxdo-devel
```

### Arch Linux

```bash
sudo pacman -S gtk3 atk cairo libappindicator-gtk3
sudo pacman -S alsa-lib
sudo pacman -S xdotool
```

## Running

```bash
cargo run
```

Or with the system tray icon:

```bash
cargo run --features tray-icon --features cuda
```

### Build with System Tray Icon

```bash
cargo build --features tray-icon --features cuda
```

## Data Storage

The application follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html) for storing data:

- Configuration files are stored in `~/.config/voice_input/` (or the equivalent XDG config directory)
- Model weights are stored in `~/.local/share/voice_input/models/` (or the equivalent XDG data directory)

When you first run the application, it will download the necessary model files to the appropriate XDG directory. For backward compatibility, the application will also check the current directory for model files.

## Debian Package

### Building the Debian Package

To build a Debian package (.deb) for easy installation, you can use the provided script:

```bash
./build_deb.sh
```

Or manually run:

```bash
dpkg-buildpackage -us -uc -b
```

This will create a .deb file in the parent directory.

### Installing the Debian Package

To install the generated Debian package:

```bash
sudo dpkg -i ../voice-input_0.1.5-1_amd64.deb
sudo apt-get install -f  # Install any missing dependencies
```

After installation, you can launch the application from your application menu or by running `voice-input` in the terminal. The application will also start automatically when you log in to your system.


reproducing Github actions
```bash
docker run --rm -it ghcr.io/catthehacker/ubuntu:act-22.04 bash
apt-get update -qy
apt-get install -qy \
  build-essential curl git pkg-config cmake \
  libssl-dev \
  libasound2-dev \
  libx11-dev libxtst-dev libxi-dev libxkbcommon-dev \
  clang llvm-dev libclang-dev

# Optional: if you build with the tray feature, also:
# apt-get install -qy libgtk-3-dev libappindicator3-dev

# Point bindgen to libclang (best: use llvm-config)
export LIBCLANG_PATH="$(llvm-config --libdir)"

# Sanity check: should exist
ls -l "$LIBCLANG_PATH/libclang.so" || echo "libclang.so not found in $LIBCLANG_PATH"

curl https://sh.rustup.rs -sSf | sh -s -- -y
source $HOME/.cargo/env

git clone https://github.com/alantsov/voice-input.git
cd voice-input

# Build again
cargo build --release
# or, if your CI enables it:
# cargo build --release --features tray-icon

```

## GitHub Actions & Releases

This repository uses a single release workflow that runs only when a version tag is pushed. Regular branch pushes and pull requests do not trigger any workflows.

- Trigger: pushing a tag that matches vX.Y.Z (for example: v0.2.3)
- Workflow file: .github/workflows/release.yml
- Output: a Debian package (.deb) built with both features enabled: tray-icon and cuda
- Release: the .deb is attached to a GitHub Release created for the tag

What happens on tag push:
- Dependencies for packaging, GTK/tray, CUDA, and LLVM/Clang are installed.
- LIBCLANG_PATH is set for bindgen-dependent crates.
- The Debian package is built via dpkg-buildpackage, which calls debian/rules.
  - debian/rules forces cargo build --release with both --features tray-icon and --features cuda.
- A GitHub Release is created and the resulting .deb is uploaded as an asset.

How to cut a release:
```bash
# Ensure your changes are committed on the main branch (or your release branch)
# Bump version in Cargo.toml if needed.

git tag v0.2.3
git push origin v0.2.3
```

Notes:
- Only pushing a tag triggers the release. No other events run GitHub Actions.
- The release workflow builds on Ubuntu and produces an amd64 .deb. Installation example:
  sudo dpkg -i <downloaded_deb_file>
  sudo apt-get -f install  # fix any missing dependencies
