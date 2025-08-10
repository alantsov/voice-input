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
    Error { error_type: ErrorType, recoverable: bool },
    Shutdown,
}

#[derive(Debug, Clone)]
enum ErrorType {
    ModelLoadingError { retryable: bool, retry_count: u32 },
    AudioSystemError { device_lost: bool },
    TranscriptionTimeout { partial_result: Option<String> },
    NetworkError { retry_after: Duration },
    ResourceExhausted,
}
```


### Event Definitions
```rust
#[derive(Debug, Clone)]
enum AppEvent {
    // Model events
    ModelLoaded,
    ModelLoadingFailed(ErrorType),
    
    // Recording events
    StartRecording,
    StopRecording,
    RecordingStoppedByOS,
    RecordingNotStarted,
    
    // Transcription events
    TranscriptionFinished(String),
    TranscriptionFailed(ErrorType),
    
    // Model management
    ChangeModel(String),
    LoadModel(String),
    
    // System events
    Shutdown,
    LanguageDetected(String),
}
```


### Command Definitions
```rust
#[derive(Debug, Clone)]
enum AudioCommand {
    StartRecording,
    StopRecording,
    Shutdown,
}

#[derive(Debug, Clone)]
enum TranscriptionCommand {
    ProcessAudio(Vec<f32>),
    ChangeModel(String),
    Shutdown,
}

#[derive(Debug, Clone)]
enum ModelCommand {
    LoadModel(String),
    DownloadModel(String),
    Shutdown,
}

#[derive(Debug, Clone)]
enum UIUpdate {
    StateChanged(AppState),
    TranscriptionResult(String),
    ErrorMessage(String),
    ProgressUpdate(u32),
}
```


### Event Processing Rules
- **Event Queue**: Use a bounded channel (capacity 100) to prevent memory issues
- **Priority Handling**: System events (Shutdown) take priority over user events
- **Concurrent Events**: Only one state transition can occur at a time
- **Event Dropping**: Drop events if channel is full, with logging
- **Timeout Handling**: All operations have configurable timeouts

### Channel Configuration
- **State Machine Event Channel**: Unbounded (system critical)
- **Audio Data Channel**: Bounded 10MB (prevents memory overflow)
- **UI Update Channel**: Bounded 100 messages
- **Worker Command Channels**: Bounded 50 commands each
- **Shutdown Channel**: Broadcast channel for clean termination

### Resource Constraints
- **Memory Limits**: Maximum audio buffer size (30 seconds)
- **Timeout Values**:
    - Model loading: 60 seconds
    - Transcription: 30 seconds
    - Recording: 5 minutes maximum
- **Retry Logic**: Exponential backoff for network operations
- **Cleanup**: Automatic cleanup of temporary audio files

## Implementation Plan

### Phase 1: Foundation (Steps 1-4)
#### Step 1: Create State Machine Module
- **File**: `src/state_machine.rs`
- Define all enums (`AppState`, `AppEvent`, `AudioCommand`, `TranscriptionCommand`, `ModelCommand`, `UIUpdate`, `ErrorType`)
- Create basic module structure with proper error handling
- Add configuration constants for timeouts and limits

#### Step 2: State Machine Structure
```rust
pub struct StateMachine {
    state: AppState,
    event_receiver: Receiver<AppEvent>,
    audio_sender: Sender<AudioCommand>,
    transcription_sender: Sender<TranscriptionCommand>,
    model_sender: Sender<ModelCommand>,
    ui_sender: Sender<UIUpdate>,
    shutdown_receiver: Receiver<()>,
}
```

- Implement constructor and basic event loop
- Add placeholder methods for command sending
- Add proper shutdown handling

#### Step 3: State Transition Logic
- Complete `handle_event()` method with full transition matrix
- Add state transition validation
- Implement comprehensive logging for all transitions
- Add timeout management for long-running operations

#### Step 4: Channel Communication
- Implement helper methods for sending commands to worker threads
- Add error handling for channel failures with retry logic
- Create UI update dispatch logic
- Implement priority-based event processing

### Phase 2: Worker Threads (Steps 5-7)
#### Step 5: Audio Worker Thread
- **File**: `src/audio_worker.rs`
- Integrate with existing `AudioStream`
- Handle `AudioCommand::StartRecording` and `AudioCommand::StopRecording`
- Send audio data and status events back to state machine
- Remove mutex dependencies from audio handling
- Implement proper resource cleanup

#### Step 6: Transcription Worker Thread
- **File**: `src/transcription_worker.rs`
- Integrate with existing `WhisperTranscriber`
- Process audio data from recording
- Send transcription results back to state machine
- Handle transcription failures gracefully with retry logic
- Implement timeout handling for long transcriptions

#### Step 7: Model Management Worker Thread
- **File**: `src/model_worker.rs`
- Handle model loading and downloading
- Integrate with existing model management code
- Send model status updates to state machine
- Support model switching without blocking
- Implement download progress reporting

### Phase 3: Main Thread Integration (Steps 8-10)
#### Step 8: Main Thread Refactoring
- **File**: `src/main.rs`
- Remove all `lazy_static!` mutex usage
- Create `spawn_state_machine()` function
- Replace direct function calls with event sending
- Implement proper error propagation

#### Step 9: Keyboard Event Integration
- Modify `handle_keyboard_event()` to send high-level events
- Map Ctrl+CapsLock to `StartRecording`/`StopRecording` events
- Remove direct audio stream manipulation from keyboard handler
- Add debouncing for rapid key presses

#### Step 10: UI Update Handler
- Create dedicated thread for handling `UIUpdate` messages
- Integrate with clipboard insertion
- Update tray icon based on state changes
- Handle error notifications with user-friendly messages

### Phase 4: Component Refactoring (Steps 11-13)
#### Step 11: Audio Stream Refactoring
- **File**: `src/audio_stream.rs`
- Remove `Arc<Mutex<Vec<f32>>>` shared state
- Use channels for audio data transfer
- Make audio stream thread-safe without mutexes
- Implement proper device reconnection logic

#### Step 12: Whisper Integration Update
- **File**: `src/whisper.rs`
- Ensure thread-safe operation without shared state
- Optimize for worker thread usage
- Maintain existing functionality
- Add model switching capabilities

#### Step 13: Tray Icon Integration
- **File**: `src/tray_icon.rs`
- Remove direct state access
- Use channels for state updates
- Handle menu actions via events
- Add visual feedback for different states

### Phase 5: Quality Assurance (Steps 14-20)
#### Step 14: Comprehensive Logging
- Add debug logging for all state transitions
- Log inter-thread communication with timing
- Add operation timing information
- Implement log rotation and cleanup

#### Step 15: Timeout Handling
- Add timeout events for long operations
- Implement recovery mechanisms
- Handle hanging operations gracefully
- Add user notifications for timeouts

#### Step 16: Graceful Shutdown
- Implement proper shutdown signaling across all threads
- Clean up resources properly (files, audio devices, network connections)
- Test shutdown scenarios (forced, graceful, error-induced)
- Add shutdown progress reporting

#### Step 17: Error Handling
- Add comprehensive error recovery
- Implement retry logic with exponential backoff
- Provide user-friendly error messages
- Add error reporting mechanisms

#### Step 18: Edge Case Testing
- Test rapid key presses with debouncing
- Test model switching during active operations
- Test system resource constraints
- Test network failures and recovery

#### Step 19: Integration Testing
- Test complete workflow end-to-end
- Verify all state transitions work correctly
- Test error recovery scenarios
- Test multi-session scenarios

#### Step 20: Performance Testing
- Measure latency and resource usage
- Optimize bottlenecks
- Ensure responsive user experience
- Benchmark against current implementation

## Complete State Transition Matrix

| Current State | Event | Next State | Action |
|---------------|-------|------------|---------|
| LoadingInitialModel | ModelLoaded | Ready | Send UIUpdate::StateChanged |
| LoadingInitialModel | ModelLoadingFailed | Error{recoverable: true} | Send UIUpdate::ErrorMessage |
| LoadingInitialModel | Shutdown | Shutdown | Send shutdown to all workers |
| Ready | StartRecording | Recording | Send AudioCommand::StartRecording |
| Ready | ChangeModel | LoadingInitialModel | Send ModelCommand::ChangeModel |
| Ready | StopRecording | Ready | Log invalid request, ignore |
| Ready | Shutdown | Shutdown | Send shutdown to all workers |
| Recording | StopRecording | Transcribing | Send AudioCommand::StopRecording |
| Recording | RecordingStoppedByOS | Ready | Send UIUpdate::StateChanged |
| Recording | RecordingNotStarted | Ready | Send UIUpdate::ErrorMessage |
| Recording | StartRecording | Recording | Log duplicate request, ignore |
| Recording | Shutdown | Shutdown | Stop recording, send shutdown |
| Transcribing | TranscriptionFinished | Ready | Send UIUpdate::TranscriptionResult |
| Transcribing | TranscriptionFailed | Error{recoverable: true} | Send UIUpdate::ErrorMessage |
| Transcribing | StartRecording | Transcribing | Queue event for after transcription |
| Transcribing | Shutdown | Shutdown | Cancel transcription, send shutdown |
| Error{recoverable: true} | LoadModel | LoadingInitialModel | Send ModelCommand::LoadModel |
| Error{recoverable: true} | StartRecording | Error | Log attempt, send error message |
| Error{recoverable: true} | Shutdown | Shutdown | Send shutdown to all workers |
| Error{recoverable: false} | Any (except Shutdown) | Error | Log attempt, ignore |
| Error{recoverable: false} | Shutdown | Shutdown | Send shutdown to all workers |
| Shutdown | Any | Shutdown | Ignore all events |

## Performance Targets
- **State Transition Latency**: < 1ms
- **Event Processing Rate**: > 1000 events/second
- **Memory Usage**: < 50MB baseline
- **Recording Start Latency**: < 100ms
- **UI Update Latency**: < 16ms (60 FPS)

## Testing Requirements

### Unit Tests
- All state transitions with valid/invalid events
- Channel communication failure scenarios
- Worker thread lifecycle management
- Error handling and recovery paths
- Timeout behavior

### Integration Tests
- End-to-end recording → transcription flow
- Model switching during active operations
- System resource exhaustion scenarios
- Network failure and recovery
- Concurrent operation handling

### Load Tests
- Rapid keyboard event simulation
- Long recording sessions (> 1 hour)
- Multiple model changes in sequence
- High-frequency state transitions
- Memory leak detection

### Performance Tests
- Latency measurements for all operations
- Resource usage under normal load
- Stress testing with maximum limits
- Comparison with current mutex-based implementation

## Implementation Safety

### Feature Flags
```rust
#[cfg(feature = "state_machine")]
// New implementation
#[cfg(not(feature = "state_machine"))]
// Fallback to current implementation
```


### Migration Path
- **Phase 1**: Implement alongside existing code
- **Phase 2**: Optional flag-based switching
- **Phase 3**: A/B testing with telemetry
- **Phase 4**: Full migration with fallback capability
- **Phase 5**: Remove old implementation

### Rollback Strategy
- **Automatic fallback**: On >10% performance degradation
- **Manual override**: Command-line flag for troubleshooting
- **Telemetry**: Monitor key metrics (latency, errors, crashes)
- **Gradual rollout**: Enable for percentage of users first

## Parallel Implementation Tracks

### Track A (State Core): Sequential
1. Steps 1-4: Core state machine
2. Step 8: Main thread integration
3. Step 16: Shutdown handling

### Track B (Audio): Can run parallel to Track A
1. Steps 5, 11: Audio worker and stream refactoring
2. Step 9: Keyboard integration
3. Step 18: Edge case testing

### Track C (Transcription): Depends on Track A completion
1. Steps 6, 12: Transcription worker and Whisper integration
2. Step 17: Error handling
3. Step 19: Integration testing

### Track D (UI/UX): Depends on Tracks A & B
1. Steps 10, 13: UI updates and tray icon
2. Step 14: Logging
3. Step 20: Performance testing

## Success Criteria

### Functional Requirements
- ✅ State machine runs in separate thread
- ✅ No mutex usage in state machine thread
- ✅ All communication via channels
- ✅ Heavy tasks delegated to worker threads
- ✅ All existing functionality preserved

### Technical Requirements
- ✅ Clean state transition logic with full coverage
- ✅ Comprehensive error handling with recovery
- ✅ Graceful shutdown capability
- ✅ Performance equivalent or better than current implementation
- ✅ Thread-safe operation without race conditions

### Quality Requirements
- ✅ Comprehensive logging with structured output
- ✅ Timeout handling for all long operations
- ✅ Edge case coverage with automated tests
- ✅ Integration test coverage >90%
- ✅ Performance benchmarks showing improvement

## Files to Modify/Create

### New Files
- `src/state_machine.rs` - Main state machine implementation
- `src/audio_worker.rs` - Audio recording worker thread
- `src/transcription_worker.rs` - Transcription processing worker
- `src/model_worker.rs` - Model management worker
- `src/channel_types.rs` - Channel type definitions and configuration
- `tests/integration_tests.rs` - Integration test suite
- `benches/performance.rs` - Performance benchmarks

### Modified Files
- `src/main.rs` - Remove mutexes, integrate state machine
- `src/audio_stream.rs` - Remove shared state, use channels
- `src/whisper.rs` - Ensure thread-safe operation
- `src/tray_icon.rs` - Use channels for state updates
- `Cargo.toml` - Add tokio for async channels if needed
- `src/config.rs` - Add state machine configuration options

## Implementation Priority

**Critical Path (Blocks everything else)**: Steps 1-4, 8
- Core state machine functionality
- Basic integration with main thread
- Channel infrastructure

**High Priority (Enables core features)**: Steps 5-6, 9-10
- Audio recording and transcription workers
- Keyboard and UI integration

**Medium Priority (Improves robustness)**: Steps 7, 11-13, 15-17
- Model management
- Component refactoring
- Error handling and timeouts

**Low Priority (Quality assurance)**: Steps 14, 18-20
- Logging, testing, and optimization

This architecture will provide a clean separation of concerns, eliminate race conditions, improve maintainability, and make the codebase more testable while maintaining or improving performance.