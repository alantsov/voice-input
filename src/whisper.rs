use std::fs::File;
use std::io::{Write, copy};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};
use reqwest::blocking::Client;
use indicatif::{ProgressBar, ProgressStyle};

pub struct WhisperTranscriber {
    context: WhisperContext,
}

impl WhisperTranscriber {
    /// Create a new WhisperTranscriber with the specified model path
    /// If the model doesn't exist, it will be downloaded automatically
    pub fn new(model_path: &str) -> Result<Self, String> {
        // Check if the model exists, download it if it doesn't
        if !Path::new(model_path).exists() {
            println!("Model file not found. Downloading...");
            Self::download_model(model_path)?;
        }

        let context = WhisperContext::new(model_path)
            .map_err(|e| format!("Failed to create whisper context: {}", e))?;

        Ok(WhisperTranscriber { context })
    }

    /// Download the Whisper model from the official repository
    fn download_model(model_name: &str) -> Result<(), String> {
        // Base URL for Whisper models
        let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";
        let url = format!("{}{}", base_url, model_name);

        println!("Downloading model from: {}", url);

        // Create a client
        let client = Client::new();

        // Make a request to get the file
        let response = client.get(&url)
            .send()
            .map_err(|e| format!("Failed to download model: {}", e))?;

        // Check if the request was successful
        if !response.status().is_success() {
            return Err(format!("Failed to download model: HTTP status {}", response.status()));
        }

        // Get the content length for progress reporting
        let total_size = response.content_length().unwrap_or(0);

        // Create a progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        // Create the file
        let mut file = File::create(model_name)
            .map_err(|e| format!("Failed to create model file: {}", e))?;

        // Create a reader that updates the progress bar
        let mut reader = response.bytes()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Write the file
        file.write_all(&reader)
            .map_err(|e| format!("Failed to write model file: {}", e))?;

        pb.finish_with_message("Download complete");

        println!("Model downloaded successfully to: {}", model_name);
        Ok(())
    }

    /// Transcribe audio from a WAV file and save the transcript to a text file
    pub fn transcribe_audio(&self, audio_path: &str) -> Result<String, String> {
        // Load audio samples from WAV file
        let audio_data = self.load_audio_from_wav(audio_path)?;

        // Create parameters for transcription
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set parameters as needed
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(true);

        // Create a state for the context
        let mut state = self.context.create_state()
            .map_err(|e| format!("Failed to create state: {}", e))?;

        // Process the audio
        state.full(params, &audio_data[..])
            .map_err(|e| format!("Failed to process audio: {}", e))?;

        // Extract the transcript
        let num_segments = state.full_n_segments()
            .map_err(|e| format!("Failed to get number of segments: {}", e))?;

        let mut transcript = String::new();

        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment {}: {}", i, e))?;

            transcript.push_str(&segment);
            transcript.push('\n');
        }

        // Generate timestamp for the transcript file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let transcript_filename = format!("transcript_{}.txt", timestamp);

        // Save transcript to file
        self.save_transcript(&transcript, &transcript_filename)?;

        Ok(transcript)
    }

    /// Load audio data from a WAV file
    fn load_audio_from_wav(&self, audio_path: &str) -> Result<Vec<f32>, String> {
        let reader = hound::WavReader::open(audio_path)
            .map_err(|e| format!("Failed to open WAV file: {}", e))?;

        let spec = reader.spec();
        let samples: Result<Vec<_>, _> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>().collect()
            },
            hound::SampleFormat::Int => {
                match spec.bits_per_sample {
                    16 => reader.into_samples::<i16>()
                        .map(|s| s.map(|s| s as f32 / 32768.0))
                        .collect(),
                    24 => reader.into_samples::<i32>()
                        .map(|s| s.map(|s| s as f32 / 8388608.0))
                        .collect(),
                    32 => reader.into_samples::<i32>()
                        .map(|s| s.map(|s| s as f32 / 2147483648.0))
                        .collect(),
                    _ => return Err(format!("Unsupported bits per sample: {}", spec.bits_per_sample)),
                }
            },
        };

        let samples = samples
            .map_err(|e| format!("Failed to read samples: {}", e))?;

        // Convert to mono if needed (whisper expects mono audio)
        let mono_samples = if spec.channels > 1 {
            self.convert_to_mono(&samples, spec.channels as usize)
        } else {
            samples
        };

        // Resample to 16kHz if needed (whisper expects 16kHz audio)
        let target_sample_rate = 16000;
        if spec.sample_rate != target_sample_rate {
            self.resample(&mono_samples, spec.sample_rate, target_sample_rate)
        } else {
            Ok(mono_samples)
        }
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

    /// Save transcript to a text file
    fn save_transcript(&self, transcript: &str, filename: &str) -> Result<(), String> {
        let mut file = File::create(filename)
            .map_err(|e| format!("Failed to create transcript file: {}", e))?;

        file.write_all(transcript.as_bytes())
            .map_err(|e| format!("Failed to write transcript: {}", e))?;

        println!("Transcript saved to {}", filename);
        Ok(())
    }
}
