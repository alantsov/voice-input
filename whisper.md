# Whisper Integration in Voice Input Application

## Overview

This document explains how the [Whisper](https://github.com/openai/whisper) speech recognition model is integrated into the Voice Input application. The application uses the Rust bindings for Whisper ([whisper-rs](https://github.com/tazz4843/whisper-rs)) to provide real-time speech-to-text functionality.

## Architecture

The Voice Input application consists of several key components:

1. **Main Application** (`main.rs`): Handles keyboard events, audio recording, and coordinates the transcription process.
2. **Whisper Integration** (`whisper.rs`): Provides a wrapper around the Whisper model for audio transcription.
3. **Audio Stream** (`audio_stream.rs`): Manages audio recording from the microphone.
4. **Keyboard Layout Detection** (`keyboard_layout.rs`): Detects the current keyboard layout to determine the language for transcription.
5. **Tray Icon** (`tray_icon.rs`): Provides a system tray icon for easy access to the application.

## How It Works

### Initialization Process

1. **Application Startup**:
   - The application checks if the required Whisper model files exist.
   - If not, it downloads them automatically from the Hugging Face repository.
   - The English model (`ggml-base.en.bin`) and multilingual model (`ggml-base.bin`) are downloaded if needed.
   - The transcribers are NOT initialized at startup to save memory and improve startup time.

2. **On-Demand Initialization (F12 Key Press)**:
   - When the user presses the F12 key, the application:
     - Detects the current keyboard layout language
     - Determines whether to use the English or multilingual model based on the detected language
     - Initializes the appropriate Whisper transcriber (loads model weights into VRAM or RAM)
     - Starts recording audio from the microphone

3. **Transcription Process (F12 Key Release)**:
   - When the user releases the F12 key, the application:
     - Stops recording audio
     - Saves the recorded audio to a WAV file
     - Transcribes the audio using the initialized Whisper model
     - Inserts the transcribed text at the current cursor position

### Language Detection and Model Selection

The application automatically detects the current keyboard layout language and selects the appropriate Whisper model:

- If the detected language is English (`en`), the English model (`ggml-base.en.bin`) is used.
- For all other languages, the multilingual model (`ggml-base.bin`) is used.

This approach optimizes performance by using the smaller, faster English model when possible, while still supporting multiple languages.

### Whisper Transcriber Implementation

The `WhisperTranscriber` class in `whisper.rs` provides the following functionality:

1. **Model Download**: Automatically downloads the required model files if they don't exist.
2. **Audio Processing**: Converts WAV files to the format required by Whisper (16kHz mono).
3. **Transcription**: Processes audio and generates text transcriptions.
4. **Language Support**: Allows specifying the language for more accurate transcription.

## Usage

1. Press the F12 key to start recording (this also initializes the Whisper model).
2. Speak into your microphone.
3. Release the F12 key to stop recording and transcribe.
4. The transcribed text will be automatically typed at your current cursor position.

## File Management

The application saves both the recorded audio and the transcription:

- Audio files are saved as `voice_YYYYMMDD_HHMMSS.wav`
- Transcription files are saved as `transcript_YYYYMMDD_HHMMSS.txt`

These files are stored in the application's working directory for reference.

## Performance Considerations

- **Memory Usage**: By initializing the Whisper model on-demand (when F12 is pressed) rather than at startup, the application reduces memory usage when not actively transcribing.
- **Model Selection**: Using the English-specific model for English transcription improves performance compared to the larger multilingual model.
- **GPU Acceleration**: The application uses CUDA GPU acceleration when an NVIDIA GPU is available, significantly improving transcription speed.
- **Transcription Time**: The time required for transcription depends on the length of the audio and the hardware capabilities. With GPU acceleration, transcription can be several times faster than CPU-only processing.

## Dependencies

- **whisper-rs**: Rust bindings for the Whisper speech recognition model
- **cpal**: Cross-platform audio library for recording
- **hound**: WAV file manipulation
- **sys-locale**: System locale detection for language identification
- **rdev**: Global keyboard event handling
- **enigo**: Keyboard input simulation for inserting transcribed text
