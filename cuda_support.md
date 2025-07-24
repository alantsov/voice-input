# CUDA Support Implementation Plan for Voice Input Application

## Current Status
The Voice Input application currently has CUDA support partially implemented:
- The Cargo.toml file includes a "cuda" feature flag that enables the "cuda" feature of the whisper-rs crate.
- The whisper.rs file has conditional compilation blocks that detect when the "cuda" feature is enabled.
- The application logs whether CUDA is enabled but doesn't actually configure whisper-rs to use CUDA for inference.

Despite these preparations, the application doesn't effectively utilize GPU acceleration for transcription.

## Implementation Plan

### 1. Prerequisites and Dependencies

#### System Requirements
- NVIDIA GPU with CUDA support
- CUDA Toolkit installed (minimum version 11.2 recommended)
- Compatible NVIDIA drivers
- cuBLAS library

#### Verification Steps
Before implementing CUDA support, users should verify their system is ready:
```bash
# Check NVIDIA driver installation
nvidia-smi

# Check CUDA installation
nvcc --version

# Check for cuBLAS library
ldconfig -p | grep libcublas
```

### 2. Code Changes Required

#### 2.1. Update WhisperTranscriber::new Method
The current implementation creates a WhisperContext without any CUDA-specific configuration. We need to modify the WhisperTranscriber::new method in src/whisper.rs to enable CUDA when available:

```rust
pub fn new(model_path: &str) -> Result<Self, String> {
    // Log GPU information before loading the model
    Self::log_gpu_info();

    // Check if the model exists, download it if it doesn't
    if !Path::new(model_path).exists() {
        println!("Model file not found. Downloading...");
        Self::download_model(model_path)?;
    }

    println!("Loading whisper model: {}", model_path);
    let start_time = std::time::Instant::now();

    // Create context with CUDA support when available
    #[cfg(feature = "cuda")]
    {
        println!("Attempting to initialize model with CUDA support");
        // Try to initialize with CUDA first
        match Self::init_with_cuda(model_path) {
            Ok(context) => {
                let load_duration = start_time.elapsed();
                println!("Model loaded with CUDA in {:.2?}", load_duration);
                return Ok(WhisperTranscriber { context });
            },
            Err(e) => {
                println!("Failed to initialize with CUDA: {}", e);
                println!("Falling back to CPU implementation");
            }
        }
    }

    // CPU fallback or default path when CUDA is not enabled
    let context = WhisperContext::new(model_path)
        .map_err(|e| format!("Failed to create whisper context: {}", e))?;

    let load_duration = start_time.elapsed();
    println!("Model loaded (CPU) in {:.2?}", load_duration);

    // Print model information
    println!("Model information:");
    println!("  Model type: {}", context.model_type_readable().unwrap_or_else(|_| "Unknown".to_string()));
    println!("  Is multilingual: {}", context.is_multilingual());
    println!("  Vocabulary size: {}", context.n_vocab());
    println!("  Audio context size: {}", context.n_audio_ctx());
    println!("  Text context size: {}", context.n_text_ctx());

    Ok(WhisperTranscriber { context })
}
```

#### 2.2. Add CUDA Initialization Method
Add a new method to WhisperTranscriber to handle CUDA initialization:

```rust
#[cfg(feature = "cuda")]
fn init_with_cuda(model_path: &str) -> Result<WhisperContext, String> {
    // The whisper-rs crate should automatically use CUDA when the feature is enabled
    // and the system supports it, but we need to ensure the model is loaded correctly
    let context = WhisperContext::new(model_path)
        .map_err(|e| format!("Failed to create whisper context with CUDA: {}", e))?;

    // Check GPU memory usage after model loading to verify CUDA is being used
    match Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used,memory.total", "--format=csv"])
        .output() {
        Ok(output) => {
            if output.status.success() {
                println!("GPU memory usage after model loading with CUDA:");
                println!("  {}", String::from_utf8_lossy(&output.stdout));
            }
        },
        Err(_) => {}
    }

    Ok(context)
}
```

#### 2.3. Enhance CUDA Detection and Logging
Improve the existing log_gpu_info method to provide more detailed information about CUDA capabilities:

```rust
fn log_gpu_info() {
    println!("Checking for GPU availability...");

    // Check for NVIDIA GPU using nvidia-smi
    match Command::new("nvidia-smi").output() {
        Ok(output) => {
            if output.status.success() {
                let gpu_info = String::from_utf8_lossy(&output.stdout);
                println!("NVIDIA GPU detected. Summary:");

                // Extract and print relevant GPU information
                for line in gpu_info.lines() {
                    if line.contains("NVIDIA-SMI") || line.contains("GPU Name") || 
                       line.contains("Driver Version") || line.contains("CUDA Version") {
                        println!("  {}", line.trim());
                    }
                }

                // Check GPU memory usage before model loading
                println!("GPU memory usage before model loading:");
                if let Ok(mem_output) = Command::new("nvidia-smi")
                    .args(["--query-gpu=memory.used,memory.total", "--format=csv"])
                    .output() {
                    println!("  {}", String::from_utf8_lossy(&mem_output.stdout));
                }

                // Check CUDA compute capability
                if let Ok(compute_output) = Command::new("nvidia-smi")
                    .args(["--query-gpu=compute_cap", "--format=csv"])
                    .output() {
                    println!("CUDA Compute Capability:");
                    println!("  {}", String::from_utf8_lossy(&compute_output.stdout));
                }
            } else {
                println!("nvidia-smi command failed. GPU might not be available or drivers not installed.");
            }
        },
        Err(_) => {
            println!("nvidia-smi command not found. NVIDIA GPU drivers might not be installed.");
        }
    }

    // Check for CUDA libraries
    Self::check_cuda_libraries();
}
```

#### 2.4. Update the transcribe_audio Method
Enhance the transcribe_audio method to provide better feedback about CUDA usage during transcription:

```rust
pub fn transcribe_audio(audio_path: &str, language: Option<&str>) -> Result<(String, std::time::Duration, std::time::Duration), String> {
    println!("Starting transcription of audio file: {}", audio_path);

    // Check GPU memory usage before transcription
    match Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used,memory.total,utilization.gpu", "--format=csv"])
        .output() {
        Ok(output) => {
            if output.status.success() {
                println!("GPU status before transcription:");
                println!("  {}", String::from_utf8_lossy(&output.stdout));
            }
        },
        Err(_) => {}
    }

    // Start timing for audio conversion
    let conversion_start = std::time::Instant::now();

    // Load audio samples from WAV file
    let audio_data = self.load_audio_from_wav(audio_path)?;

    // Record time for audio conversion
    let conversion_duration = conversion_start.elapsed();
    println!("Audio conversion completed in {:.2?}", conversion_duration);
    println!("Loaded audio data: {} samples", audio_data.len());

    // Create parameters for transcription
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {beam_size: 5, patience: 1.2});

    // Set parameters as needed
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(true);
    params.set_temperature(0.0);

    // Set number of threads to use
    #[cfg(feature = "cuda")]
    {
        // When using CUDA, we can use fewer CPU threads as the GPU does most of the work
        params.set_n_threads(4);
        println!("Using 4 threads for transcription with CUDA");
    }

    #[cfg(not(feature = "cuda"))]
    {
        // When not using CUDA, use more CPU threads
        params.set_n_threads(8);
        println!("Using 8 threads for transcription (CPU only)");
    }

    // Set language if provided
    if let Some(lang) = language {
        // Extract the language code (first 2 characters of the locale)
        let lang_code = if lang.len() >= 2 {
            &lang[0..2]
        } else {
            lang
        };

        // Set the language for transcription
        params.set_language(Some(lang_code));
        println!("Using language '{}' for transcription", lang_code);
    }

    // Create a state for the context
    let mut state = self.context.create_state()
        .map_err(|e| format!("Failed to create state: {}", e))?;

    println!("Starting audio processing...");

    // Check if CUDA is available and being used
    #[cfg(feature = "cuda")]
    {
        println!("CUDA GPU acceleration is enabled and will be used");
    }

    #[cfg(not(feature = "cuda"))]
    {
        println!("CUDA GPU acceleration is not available, using CPU only");
    }

    // Start timing for actual transcription
    let transcription_start = std::time::Instant::now();

    // Process the audio
    state.full(params, &audio_data[..])
        .map_err(|e| format!("Failed to process audio: {}", e))?;

    // Record time for actual transcription
    let transcription_duration = transcription_start.elapsed();
    println!("Audio processed in {:.2?}", transcription_duration);

    // Check GPU memory usage after transcription
    match Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used,memory.total,utilization.gpu", "--format=csv"])
        .output() {
        Ok(output) => {
            if output.status.success() {
                println!("GPU status after transcription:");
                println!("  {}", String::from_utf8_lossy(&output.stdout));
            }
        },
        Err(_) => {}
    }

    // Extract the transcript
    let num_segments = state.full_n_segments()
        .map_err(|e| format!("Failed to get number of segments: {}", e))?;

    let mut transcript = String::new();

    for i in 0..num_segments {
        let segment = state.full_get_segment_text(i)
            .map_err(|e| format!("Failed to get segment {}: {}", i, e))?;
        let short_segment = &segment.strip_prefix(" ");
        transcript.push_str(short_segment.unwrap_or(&segment));
        transcript.push('\n');
    }

    // Generate timestamp for the transcript file
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let transcript_filename = format!("transcript_{}.txt", timestamp);

    // Save transcript to file
    self.save_transcript(&transcript, &transcript_filename)?;

    Ok((transcript, conversion_duration, transcription_duration))
}
```

### 3. Build and Run Instructions

#### 3.1. Building with CUDA Support
To build the application with CUDA support:

```bash
cargo build --release --features cuda
```

For both CUDA and tray icon support:

```bash
cargo build --release --features "cuda tray-icon"
```

#### 3.2. Running with CUDA Support
To run the application with CUDA support:

```bash
cargo run --release --features cuda
```

For both CUDA and tray icon support:

```bash
cargo run --release --features "cuda tray-icon"
```

### 4. Testing and Verification

#### 4.1. Verifying CUDA Usage
To verify that CUDA is being used:

1. Run the application with CUDA enabled
2. Press F12 to start recording
3. Release F12 to stop recording and transcribe
4. Check the console output for:
   - "CUDA GPU acceleration is enabled and will be used"
   - GPU memory usage changes before and after transcription
   - GPU utilization during transcription

#### 4.2. Performance Comparison
Compare transcription times with and without CUDA:

1. Run the application without CUDA: `cargo run --release`
2. Record a sample and note the transcription time
3. Run the application with CUDA: `cargo run --release --features cuda`
4. Record the same sample and compare the transcription time

### 5. Troubleshooting

#### 5.1. Common Issues
- **CUDA not detected**: Ensure NVIDIA drivers and CUDA toolkit are properly installed
- **Out of memory errors**: Try using a smaller model or reducing batch size
- **Slow performance**: Check GPU utilization, may need to adjust parameters

#### 5.2. Debugging
- Enable more verbose logging in the whisper.rs module
- Monitor GPU usage with `nvidia-smi -l 1` in a separate terminal
- Check for CUDA-related errors in the application output

### 6. Future Improvements

#### 6.1. Advanced CUDA Optimizations
- Implement batch processing for multiple audio files
- Add support for mixed precision (FP16) for faster inference
- Optimize memory usage for larger models

#### 6.2. User Interface Enhancements
- Add GPU selection for multi-GPU systems
- Provide GPU memory usage information in the UI
- Allow users to toggle between CPU and GPU processing

### 7. Conclusion
Implementing CUDA support in the Voice Input application will significantly improve transcription performance on systems with compatible NVIDIA GPUs. The changes outlined in this plan maintain backward compatibility while adding GPU acceleration capabilities.
