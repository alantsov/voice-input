# Voice Input State Machine Implementation Task

## Overview
Implement a state machine-based architecture for the voice input application to replace the current mutex-based approach with a clean, thread-safe, channel-based communication system.

## Requirements

### Core Principles
- **Separate thread**: State machine must run in its own dedicated thread, not the main thread
- **High-level events only**: State machine receives semantic events like "start recording", not low-level events like "ctrl key down"
- **No mutex usage**: All variables should be thread-local within each thread
- **No heavy tasks**: State machine thread only manages state transitions and delegates work to other threads
- **Channel-based communication**: All inter-thread communication via mpsc channels

### State Definitions
```rust
#[derive(Debug, Clone, PartialEq)]
enum AppState {
    LoadingInitialModel,
    Ready,
    Recording,
    Transcribing,
    Error { error_message: String, recoverable: bool },
    Shutdown,
}
```


### Event Definitions
```rust
#[derive(Debug, Clone)]
enum AppEvent {
    // Model events
    ModelLoaded,
    ModelLoadingFailed(String),
    
    // Recording events
    StartRecording,
    StopRecording,
    RecordingStoppedByOS,
    RecordingNotStarted,
    
    // Transcription events
    TranscriptionFinished(String),
    TranscriptionFailed(String),
    
    // Model management
    ChangeModel(String),
    LoadModel(String),
    
    // System events
    Shutdown,
    LanguageDetected(String),
}
```


## Implementation Plan

### Phase 1: Foundation (Steps 1-4)
#### Step 1: Create State Machine Module
- **File**: `src/task_state_machine.rs`
- Define all enums (`AppState`, `AppEvent`, `AudioCommand`, `TranscriptionCommand`, `ModelCommand`, `UIUpdate`)
- Create basic module structure

#### Step 2: State Machine Structure
- Define `StateMachine` struct with channels and current state
- Implement constructor and basic event loop
- Add placeholder methods for command sending

#### Step 3: State Transition Logic
- Complete `handle_event()` method with full transition matrix
- Add state transition validation
- Implement logging for all transitions

#### Step 4: Channel Communication
- Implement helper methods for sending commands to worker threads
- Add error handling for channel failures
- Create UI update dispatch logic

### Phase 2: Worker Threads (Steps 5-7)
#### Step 5: Audio Worker Thread
- **File**: `src/audio_worker.rs`
- Integrate with existing `AudioStream`
- Handle `AudioCommand::StartRecording` and `AudioCommand::StopRecording`
- Send audio data and status events back to state machine
- Remove mutex dependencies from audio handling

#### Step 6: Transcription Worker Thread
- **File**: `src/transcription_worker.rs`
- Integrate with existing `WhisperTranscriber`
- Process audio data from recording
- Send transcription results back to state machine
- Handle transcription failures gracefully

#### Step 7: Model Management Worker Thread
- **File**: `src/model_worker.rs`
- Handle model loading and downloading
- Integrate with existing model management code
- Send model status updates to state machine
- Support model switching without blocking

### Phase 3: Main Thread Integration (Steps 8-10)
#### Step 8: Main Thread Refactoring
- **File**: `src/main.rs`
- Remove all `lazy_static!` mutex usage
- Create `spawn_state_machine()` function
- Replace direct function calls with event sending

#### Step 9: Keyboard Event Integration
- Modify `handle_keyboard_event()` to send high-level events
- Map Ctrl+CapsLock to `StartRecording`/`StopRecording` events
- Remove direct audio stream manipulation from keyboard handler

#### Step 10: UI Update Handler
- Create dedicated thread for handling `UIUpdate` messages
- Integrate with clipboard insertion
- Update tray icon based on state changes
- Handle error notifications

### Phase 4: Component Refactoring (Steps 11-13)
#### Step 11: Audio Stream Refactoring
- **File**: `src/audio_stream.rs`
- Remove `Arc<Mutex<Vec<f32>>>` shared state
- Use channels for audio data transfer
- Make audio stream thread-safe without mutexes

#### Step 12: Whisper Integration Update
- **File**: `src/whisper.rs`
- Ensure thread-safe operation without shared state
- Optimize for worker thread usage
- Maintain existing functionality

#### Step 13: Tray Icon Integration
- **File**: `src/tray_icon.rs`
- Remove direct state access
- Use channels for state updates
- Handle menu actions via events

### Phase 5: Quality Assurance (Steps 14-20)
#### Step 14: Comprehensive Logging
- Add debug logging for all state transitions
- Log inter-thread communication
- Add operation timing information

#### Step 15: Timeout Handling
- Add timeout events for long operations
- Implement recovery mechanisms
- Handle hanging operations gracefully

#### Step 16: Graceful Shutdown
- Implement proper shutdown signaling across all threads
- Clean up resources properly
- Test shutdown scenarios

#### Step 17: Error Handling
- Add comprehensive error recovery
- Implement retry logic where appropriate
- Provide user-friendly error messages

#### Step 18: Edge Case Testing
- Test rapid key presses
- Test model switching during operations
- Test system resource constraints
- Test network failures

#### Step 19: Integration Testing
- Test complete workflow end-to-end
- Verify all state transitions work correctly
- Test error recovery scenarios

#### Step 20: Performance Testing
- Measure latency and resource usage
- Optimize bottlenecks
- Ensure responsive user experience

## State Transition Matrix

| Current State | Event | Next State | Action |
|---------------|-------|------------|---------|
| LoadingInitialModel | ModelLoaded | Ready | - |
| LoadingInitialModel | ModelLoadingFailed | Error{recoverable: true} | - |
| Ready | StartRecording | Recording | Send AudioCommand::StartRecording |
| Ready | ChangeModel | LoadingInitialModel | Send ModelCommand::ChangeModel |
| Recording | StopRecording | Transcribing | Send AudioCommand::StopRecording |
| Recording | RecordingStoppedByOS | Ready | - |
| Recording | RecordingNotStarted | Ready | - |
| Transcribing | TranscriptionFinished | Ready | Send UIUpdate::TranscriptionResult |
| Transcribing | TranscriptionFailed | Error{recoverable: true} | - |
| Error{recoverable: true} | LoadModel | LoadingInitialModel | Send ModelCommand::LoadModel |
| Any | Shutdown | Shutdown | Send shutdown to all workers |

## Success Criteria

### Functional Requirements
- ✅ State machine runs in separate thread
- ✅ No mutex usage in state machine thread
- ✅ All communication via channels
- ✅ Heavy tasks delegated to worker threads
- ✅ All existing functionality preserved

### Technical Requirements
- ✅ Clean state transition logic
- ✅ Comprehensive error handling
- ✅ Graceful shutdown capability
- ✅ Performance equivalent to current implementation
- ✅ Thread-safe operation

### Quality Requirements
- ✅ Comprehensive logging
- ✅ Timeout handling for long operations
- ✅ Edge case coverage
- ✅ Integration test coverage
- ✅ Performance benchmarks

## Files to Modify/Create

### New Files
- `src/task_state_machine.rs` - Main state machine implementation
- `src/audio_worker.rs` - Audio recording worker thread
- `src/transcription_worker.rs` - Transcription processing worker
- `src/model_worker.rs` - Model management worker

### Modified Files
- `src/main.rs` - Remove mutexes, integrate state machine
- `src/audio_stream.rs` - Remove shared state, use channels
- `src/whisper.rs` - Ensure thread-safe operation
- `src/tray_icon.rs` - Use channels for state updates
- `Cargo.toml` - Update if needed for additional dependencies

## Implementation Priority

**Phase 1 (Critical)**: Steps 1-4, 8-9
- Core state machine functionality
- Basic integration with main thread

**Phase 2 (High)**: Steps 5-6, 10
- Audio recording and transcription workers
- UI update handling

**Phase 3 (Medium)**: Steps 7, 11-13
- Model management
- Component refactoring

**Phase 4 (Low)**: Steps 14-20
- Quality assurance and optimization

This architecture will provide a clean separation of concerns, eliminate race conditions, and make the codebase more maintainable and testable.