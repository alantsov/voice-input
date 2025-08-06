# Voice Input Application States

This document outlines all possible states of the Voice Input application and the events that are handled or change the application state.

## Application States

### 1. Idle State
The application is running but not actively recording or processing audio.

**Events handled:**
- **Ctrl+CapsLock Pressed**: Transitions to Recording State
- **Model Selection**: Changes the selected model and may trigger Model Loading State
- **Quit**: Exits the application

### 2. Recording State
The application is actively recording audio from the microphone.

**Events handled:**
- **Ctrl+CapsLock Released**: Stops recording and transitions to Processing State
- **Quit**: Exits the application

### 3. Processing State
The application is processing the recorded audio and transcribing it.

**Sub-states:**
- **WAV Processing**: Converting recorded samples to WAV format
- **Transcription**: Running the Whisper model to transcribe the audio
- **Text Insertion**: Inserting the transcribed text at the cursor position

**Events handled:**
- **Transcription Complete**: Transitions back to Idle State
- **Transcription Error**: Transitions back to Idle State with error message
- **Quit**: Exits the application

### 4. Model Loading State
The application is downloading a new model selected by the user.

**Events handled:**
- **Download Complete**: Transitions back to Idle State
- **Download Error**: Transitions back to Idle State with error message
- **Ctrl+CapsLock Pressed**: Ignored until model loading is complete
- **Quit**: Exits the application

## Event Handling by State

### Global Events (handled in all states)
- **Quit**: Exits the application
- **About Dialog**: Shows information about the application

### State-Specific Event Handling

#### Idle State
- **Ctrl+CapsLock Pressed**: 
  1. Detects keyboard layout language
  2. Initializes appropriate transcriber (English or multilingual)
  3. Clears previous recording
  4. Starts audio stream
  5. Transitions to Recording State

- **Model Selection**:
  1. Updates selected model in configuration
  2. If model files don't exist:
     - Sets MODEL_LOADING flag
     - Updates UI to show loading status
     - Starts download thread
     - Transitions to Model Loading State

#### Recording State
- **Ctrl+CapsLock Released**:
  1. Stops recording
  2. Pauses audio stream
  3. Transitions to Processing State

#### Processing State
- **Processing Complete**:
  1. Inserts transcribed text at cursor position
  2. Deletes temporary WAV file
  3. Transitions back to Idle State

#### Model Loading State
- **Download Complete**:
  1. Resets MODEL_LOADING flag
  2. Updates UI to show normal status
  3. Transitions back to Idle State