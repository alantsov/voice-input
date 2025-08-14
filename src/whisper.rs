use std::fs::File;
use std::io::{Write, Read};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use reqwest::blocking::Client;
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command;
use crate::config;
use std::time::{Instant, Duration};
use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref DL_PROGRESS_CB: Mutex<Option<Box<dyn Fn(f64, u64) + Send + 'static>>> = Mutex::new(None);
}

pub struct WhisperTranscriber {
    context: WhisperContext,
}

impl WhisperTranscriber {
    /// Set or clear a global download progress callback.
    /// The callback receives (percent, eta_secs).
    pub fn set_download_progress_callback(cb: Option<Box<dyn Fn(f64, u64) + Send + 'static>>) {
        *DL_PROGRESS_CB.lock().unwrap() = cb;
    }
    /// Check if NVIDIA GPU is available and log GPU information
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

        // Check for other GPU information
        println!("Checking for other GPU information...");
        if let Ok(output) = Command::new("lspci").args(["-v"]).output() {
            let lspci_output = String::from_utf8_lossy(&output.stdout);
            for line in lspci_output.lines() {
                if line.contains("VGA") || line.contains("3D") || line.contains("Display") {
                    println!("  {}", line.trim());
                }
            }
        }
    }

    /// Check for CUDA libraries in the system
    fn check_cuda_libraries() {
        println!("Checking for CUDA libraries...");

        // Check for libcuda.so
        if let Ok(output) = Command::new("ldconfig").args(["-p"]).output() {
            let ldconfig_output = String::from_utf8_lossy(&output.stdout);
            let mut cuda_libs = Vec::new();

            for line in ldconfig_output.lines() {
                if line.contains("libcuda.so") || line.contains("libcudart.so") || 
                   line.contains("libnvrtc.so") || line.contains("libcublas.so") {
                    cuda_libs.push(line.trim());
                }
            }

            if !cuda_libs.is_empty() {
                println!("Found CUDA libraries:");
                for lib in cuda_libs {
                    println!("  {}", lib);
                }
            } else {
                println!("No CUDA libraries found in ldconfig cache.");
            }
        }

        // Check CUDA_VISIBLE_DEVICES environment variable
        match std::env::var("CUDA_VISIBLE_DEVICES") {
            Ok(value) => println!("CUDA_VISIBLE_DEVICES environment variable is set to: {}", value),
            Err(_) => println!("CUDA_VISIBLE_DEVICES environment variable is not set.")
        }

        // Try to get CUDA version using nvcc if available
        match Command::new("nvcc").args(["--version"]).output() {
            Ok(output) => {
                if output.status.success() {
                    println!("NVCC version information:");
                    println!("  {}", String::from_utf8_lossy(&output.stdout).trim());
                }
            },
            Err(_) => println!("NVCC (CUDA compiler) not found in PATH.")
        }
    }

    /// Initialize WhisperContext with CUDA support
    #[cfg(feature = "cuda")]
    fn init_with_cuda(model_path: &str) -> Result<WhisperContext, String> {
        // The whisper-rs crate should automatically use CUDA when the feature is enabled
        // and the system supports it, but we need to ensure the model is loaded correctly
        let temp_params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(model_path, temp_params)
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

    /// Create a new WhisperTranscriber with the specified model name
    /// If the model doesn't exist, it will be downloaded automatically
    pub fn new(model_name: &str) -> Result<Self, String> {
        // Log GPU information before loading the model
        Self::log_gpu_info();

        // Get the model path using the config module
        let model_path_opt = config::get_model_path(model_name);
        
        // If model doesn't exist in either location, download it
        if model_path_opt.is_none() {
            println!("Model file not found. Downloading...");
            Self::download_model(model_name)?;
        }
        
        // Get the path again after potential download
        let model_path = config::get_model_path(model_name).ok_or_else(|| 
            format!("Failed to locate model file after download: {}", model_name)
        )?;
        
        // Convert PathBuf to string for the whisper-rs functions
        let model_path_str = model_path.to_str().ok_or_else(|| 
            format!("Invalid UTF-8 in model path: {:?}", model_path)
        )?;

        println!("Loading whisper model: {}", model_path_str);
        let start_time = std::time::Instant::now();

        // Create context with CUDA support when available
        #[cfg(feature = "cuda")]
        {
            println!("Attempting to initialize model with CUDA support");
            // Try to initialize with CUDA first
            match Self::init_with_cuda(model_path_str) {
                Ok(context) => {
                    let load_duration = start_time.elapsed();
                    println!("Model loaded with CUDA in {:.2?}", load_duration);

                    // Print model information
                    println!("Model information:");
                    println!("  Model type: {}", context.model_type_readable().unwrap_or_else(|_| "Unknown".to_string()));
                    println!("  Is multilingual: {}", context.is_multilingual());
                    println!("  Vocabulary size: {}", context.n_vocab());
                    println!("  Audio context size: {}", context.n_audio_ctx());
                    println!("  Text context size: {}", context.n_text_ctx());

                    return Ok(WhisperTranscriber { context });
                },
                Err(e) => {
                    println!("Failed to initialize with CUDA: {}", e);
                    println!("Falling back to CPU implementation");
                }
            }
        }

        // CPU fallback or default path when CUDA is not enabled
        let temp_params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(model_path_str, temp_params)
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

        // Check GPU memory usage after model loading
        match Command::new("nvidia-smi")
            .args(["--query-gpu=memory.used,memory.total", "--format=csv"])
            .output() {
            Ok(output) => {
                if output.status.success() {
                    println!("GPU memory usage after model loading (CPU):");
                    println!("  {}", String::from_utf8_lossy(&output.stdout));
                }
            },
            Err(_) => {}
        }

        Ok(WhisperTranscriber { context })
    }

    /// Download the Whisper model from the official repository
    pub fn download_model(model_name: &str) -> Result<(), String> {
        // Base URL for Whisper models
        let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";
        let url = format!("{}{}", base_url, model_name);

        println!("Downloading model from: {}", url);

        // Create a client with increased timeout
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        // Maximum number of retries
        let max_retries = 3;
        let mut retry_count = 0;
        let mut last_error = String::new();

        // Retry loop
        while retry_count < max_retries {
            match Self::download_with_retry(&client, &url, model_name, retry_count) {
                Ok(_) => {
                    // Get the path where the model was saved for display purposes
                    if let Ok(path) = config::get_model_save_path(model_name) {
                        println!("Model downloaded successfully to: {}", path.display());
                    } else {
                        println!("Model downloaded successfully");
                    }
                    return Ok(());
                },
                Err(e) => {
                    last_error = e;
                    retry_count += 1;
                    if retry_count < max_retries {
                        let wait_time = std::time::Duration::from_secs(2u64.pow(retry_count as u32));
                        println!("Download attempt {} failed. Retrying in {} seconds...", 
                                 retry_count, wait_time.as_secs());
                        std::thread::sleep(wait_time);
                    }
                }
            }
        }

        Err(format!("Failed to download model {} after {} attempts: {}", 
                   model_name, max_retries, last_error))
    }

    /// Helper function to download with retry logic
    fn download_with_retry(client: &Client, url: &str, model_name: &str, attempt: usize) -> Result<(), String> {
        // Make a request to get the file
        let mut response = client.get(url)
            .send()
            .map_err(|e| format!("Failed to download model (attempt {}): {}", attempt + 1, e))?;

        // Check if the request was successful
        if !response.status().is_success() {
            return Err(format!("Failed to download model (attempt {}): HTTP status {}", 
                              attempt + 1, response.status()));
        }

        // Get the content length for progress reporting
        let total_size = response.content_length().unwrap_or(0);

        // Create a progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        // Get the path where the model should be saved (in XDG data directory)
        let model_path = config::get_model_save_path(model_name)
            .map_err(|e| format!("Failed to determine model save path: {}", e))?;
        
        println!("Saving model to: {}", model_path.display());
        
        // Create the file
        let mut file = File::create(&model_path)
            .map_err(|e| format!("Failed to create model file: {}", e))?;

        // Use a buffer to read the response in chunks
        let mut buffer = [0; 8192]; // 8KB buffer
        let mut downloaded: u64 = 0;

        // Timing for ETA and throttling
        let start_time = Instant::now();
        let mut last_emit = start_time;
        let emit_every = Duration::from_millis(200);

        // Read and write in chunks
        loop {
            let bytes_read = match response.read(&mut buffer) {
                Ok(0) => break, // End of file
                Ok(n) => n,
                Err(e) => return Err(format!("Failed to read from response: {}", e)),
            };

            file.write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Failed to write to file: {}", e))?;

            downloaded += bytes_read as u64;
            pb.set_position(downloaded);

            // Emit progress to callback if available and total_size is known
            if total_size > 0 {
                let now = Instant::now();
                if now.duration_since(last_emit) >= emit_every || downloaded == total_size {
                    let elapsed = now.duration_since(start_time).as_secs_f64();
                    let rate = if elapsed > 0.0 { downloaded as f64 / elapsed } else { 0.0 };
                    let remaining_bytes = (total_size.saturating_sub(downloaded)) as f64;
                    let eta_secs = if rate > 0.0 { (remaining_bytes / rate).round() as u64 } else { 0 };
                    let percent = (downloaded as f64 / total_size as f64) * 100.0;

                    if let Some(ref cb) = *DL_PROGRESS_CB.lock().unwrap() {
                        cb(percent, eta_secs);
                    }
                    last_emit = now;
                }
            }
        }

        pb.finish_with_message("Download complete");
        Ok(())
    }

    /// Transcribe audio directly from in-memory samples.
    /// Performs mono conversion and resampling to 16kHz if needed.
    pub fn transcribe_samples(
        &self,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
        language: Option<&str>,
    ) -> Result<String, String> {
        println!(
            "Starting transcription of in-memory audio: {} samples at {} Hz, {} channels",
            samples.len(),
            sample_rate,
            channels
        );

        // Check GPU memory usage before transcription
        match Command::new("nvidia-smi")
            .args(["--query-gpu=memory.used,memory.total,utilization.gpu", "--format=csv"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    println!("GPU status before transcription:");
                    println!("  {}", String::from_utf8_lossy(&output.stdout));
                }
            }
            Err(_) => {}
        }

        // Convert to mono if needed
        let mono_samples = if channels > 1 {
            self.convert_to_mono(samples, channels as usize)
        } else {
            samples.to_vec()
        };

        // Resample to 16kHz if needed
        let target_sample_rate = 16000;
        let audio_data = if sample_rate != target_sample_rate {
            self.resample(&mono_samples, sample_rate, target_sample_rate)?
        } else {
            mono_samples
        };

        println!("Prepared audio data: {} samples at 16kHz mono", audio_data.len());

        // Create parameters for transcription
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: 1.2,
        });

        // Set parameters as needed
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(true);
        params.set_temperature(0.0);

        // Set number of threads to use
        #[cfg(feature = "cuda")]
        {
            println!("Using default threads number for transcription with CUDA");
        }
        #[cfg(not(feature = "cuda"))]
        {
            params.set_n_threads(8);
            println!("Using 8 threads for transcription (CPU only)");
        }

        // Set language if provided (use 2-letter code if possible)
        if let Some(lang) = language {
            let lang_code = if lang.len() >= 2 { &lang[0..2] } else { lang };
            params.set_language(Some(lang_code));
            println!("Using language '{}' for transcription", lang_code);
        }

        // Create a state for the context
        let mut state = self
            .context
            .create_state()
            .map_err(|e| format!("Failed to create state: {}", e))?;

        println!("Starting audio processing...");

        // CUDA availability logging
        #[cfg(feature = "cuda")]
        {
            println!("CUDA GPU acceleration is enabled and will be used");
        }
        #[cfg(not(feature = "cuda"))]
        {
            println!("CUDA GPU acceleration is not available, using CPU only");
        }

        // Process the audio
        state
            .full(params, &audio_data[..])
            .map_err(|e| format!("Failed to process audio: {}", e))?;

        println!("Audio processed successfully");

        // Check GPU memory usage after transcription
        match Command::new("nvidia-smi")
            .args(["--query-gpu=memory.used,memory.total,utilization.gpu", "--format=csv"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    println!("GPU status after transcription:");
                    println!("  {}", String::from_utf8_lossy(&output.stdout));
                }
            }
            Err(_) => {}
        }

        // Extract the transcript
        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to get number of segments: {}", e))?;

        let mut transcript = String::new();
        for i in 0..num_segments {
            let segment = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment {}: {}", i, e))?;
            let short_segment = &segment.strip_prefix(" ");
            transcript.push_str(short_segment.unwrap_or(&segment));
            transcript.push('\n');
        }

        Ok(transcript)
    }

    /// Convert multi-channel audio to mono by averaging channels
    fn convert_to_mono(&self, samples: &[f32], channels: usize) -> Vec<f32> {
        let mono_len = samples.len() / channels;
        let mut mono_samples = Vec::with_capacity(mono_len);

        for i in 0..mono_len {
            let mut sum = 0.0;
            for c in 0..channels {
                sum += samples[i * channels + c];
            }
            mono_samples.push(sum / channels as f32);
        }

        mono_samples
    }

    /// Simple linear resampling
    fn resample(&self, samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, String> {
        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (samples.len() as f64 / ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let pos = i as f64 * ratio;
            let pos_floor = pos.floor() as usize;
            let pos_ceil = (pos_floor + 1).min(samples.len() - 1);
            let frac = pos - pos_floor as f64;

            let sample = samples[pos_floor] * (1.0 - frac as f32) + samples[pos_ceil] * frac as f32;
            resampled.push(sample);
        }

        Ok(resampled)
    }
}
