# Voice Input Application

A simple application for recording voice input using the microphone.

## Features

- Press F12 to start recording, release to save
- Recordings are saved as WAV files with timestamps
- Transcribes audio and inserts text at cursor position
- Uses GPU acceleration for faster transcription when available
- Optional system tray icon support

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
```

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
