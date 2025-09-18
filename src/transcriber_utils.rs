use std::sync::{Arc, Mutex};

use crate::config;
use crate::whisper::WhisperTranscriber;

/// Select the model filename based on selected model and language mode.
pub fn select_model_file(selected_model: &str, is_english: bool) -> String {
    match (selected_model, is_english) {
        ("large", true) => "ggml-large-v2.bin".to_string(),
        ("large", false) => "ggml-large-v2.bin".to_string(),
        (m @ ("base" | "small" | "medium"), true) => format!("ggml-{m}.en.bin"),
        (m @ ("base" | "small" | "medium"), false) => format!("ggml-{m}.bin"),
        // Fallbacks to base variants
        (_, true) => "ggml-base.en.bin".to_string(),
        (_, false) => "ggml-base.bin".to_string(),
    }
}

/// Ensure the appropriate WhisperTranscriber is initialized (English or multilingual),
/// with fallback to base models if the selected file is missing.
pub fn ensure_transcriber_for(
    is_english: bool,
    model_file: &str,
    english_transcriber: &Arc<Mutex<Option<WhisperTranscriber>>>,
    multilingual_transcriber: &Arc<Mutex<Option<WhisperTranscriber>>>,
) {
    // If the selected model is missing, fall back to base automatically
    let resolved_model = if config::get_model_path(model_file).is_some() {
        model_file.to_string()
    } else {
        if is_english {
            "ggml-base.en.bin".to_string()
        } else {
            "ggml-base.bin".to_string()
        }
    };

    if is_english {
        let mut guard = english_transcriber.lock().unwrap();
        if guard.is_none() {
            println!(
                "Initializing English transcriber with model: {}",
                resolved_model
            );
            match WhisperTranscriber::new(&resolved_model) {
                Ok(t) => *guard = Some(t),
                Err(e) => {
                    eprintln!(
                        "Failed to initialize English WhisperTranscriber with model {}: {}",
                        resolved_model, e
                    );
                    eprintln!("English transcription will be disabled");
                }
            }
        }
    } else {
        let mut guard = multilingual_transcriber.lock().unwrap();
        if guard.is_none() {
            println!(
                "Initializing multilingual transcriber with model: {}",
                resolved_model
            );
            match WhisperTranscriber::new(&resolved_model) {
                Ok(t) => *guard = Some(t),
                Err(e) => {
                    eprintln!(
                        "Failed to initialize Multilingual WhisperTranscriber with model {}: {}",
                        resolved_model, e
                    );
                    eprintln!("Multilingual transcription will be disabled");
                }
            }
        }
    }
}

/// Transcribe in-memory audio samples using the provided transcriber reference.
pub fn transcribe_samples_with(
    transcriber: &Arc<Mutex<Option<WhisperTranscriber>>>,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    language: &str,
) -> Result<String, String> {
    let guard = transcriber
        .lock()
        .map_err(|_| "Failed to lock transcriber".to_string())?;
    if let Some(ref t) = *guard {
        t.transcribe_samples(samples, sample_rate, channels, Some(language))
            .map_err(|e| format!("Failed to transcribe audio: {}", e))
    } else {
        Err("Transcriber is not available".to_string())
    }
}

/// Translate in-memory audio samples to English using the provided transcriber reference.
pub fn translate_samples_with(
    transcriber: &Arc<Mutex<Option<WhisperTranscriber>>>,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    language: &str,
) -> Result<String, String> {
    let guard = transcriber
        .lock()
        .map_err(|_| "Failed to lock transcriber".to_string())?;
    if let Some(ref t) = *guard {
        // language is not strictly necessary for translation; we pass it for symmetry but the method ignores it
        t.translate_samples(samples, sample_rate, channels, Some(language))
            .map_err(|e| format!("Failed to translate audio: {}", e))
    } else {
        Err("Transcriber is not available".to_string())
    }
}

/// Explicitly drop the transcriber to free its underlying resources (including GPU VRAM if CUDA is used).
pub fn cleanup_transcriber(transcriber: &Arc<Mutex<Option<WhisperTranscriber>>>) {
    if let Ok(mut guard) = transcriber.lock() {
        // Dropping WhisperTranscriber drops WhisperContext and releases associated memory/VRAM.
        *guard = None;
    }
}

