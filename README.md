# Voice Input
Dictate anywhere on your computer. Hold Ctrl+CapsLock to record, release to insert the transcription at your cursor.
## What it does
- Press and hold Ctrl+CapsLock to start recording
- Release the keys to stop and insert text where you’re typing
- Saves each recording as a timestamped WAV file
- Uses your GPU automatically if available for faster transcription
- Optional system tray icon

## Install
- Debian/Ubuntu: Download the latest .deb from Releases and install:
    - Double-click the .deb, or run: sudo dpkg -i path/to/voice-input_*.deb && sudo apt-get -f install

- Other Linux: See Advanced for building from source

Note: On first run, the app downloads speech model files to your user data directory.
## Run
- After installing the .deb, launch “Voice Input” from your applications menu
- Or run from a terminal: voice-input

Tip: The app can start automatically on login when installed via the .deb.
## Use
- Start recording: Hold Ctrl+CapsLock
- Stop and insert text: Release the keys
- System tray: click the tray icon for quick actions and settings

## Requirements
- A working microphone
- Linux desktop environment
- For best results with the tray icon: a system tray supported by your desktop

GPU is optional. If available, it will be used automatically for faster transcription.
## Data and privacy
- Config: ~/.config/voice_input/
- Models: ~/.local/share/voice_input/models/
- Your transcriptions stay local unless you choose to share them

## Troubleshooting
- Shortcut doesn’t work: Check for conflicts with other apps using CapsLock shortcuts, set another shortcut for voice-input in tray menu
- No microphone input: Verify input device in system sound settings and mic permissions
- Text not inserted: Ensure an editable text field is focused; if using Wayland, try an XWayland app or enable assistive/automation permissions
- Slow transcription: use `small` model

## Uninstall
- If installed via .deb: sudo apt remove voice-input
- Optional: Manually delete config and models from the paths above

## Advanced
- Building from source, optional tray icon, CUDA, and packaging instructions: See the Advanced section in the project documentation.
