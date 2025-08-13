use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use rdev::listen;
use std::sync::mpsc::channel;
use lazy_static::lazy_static;
use fs2::FileExt;
mod tray_icon;
mod audio_stream;
mod whisper;
mod keyboard_layout;
mod clipboard_inserter;
mod config;
mod single_instance;
mod hotkeys;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use keyboard_layout::KeyboardLayoutDetector;
use hotkeys::{KeyboardEvent, KEYBOARD_EVENT_SENDER, handle_keyboard_event};

lazy_static! {
    static ref SELECTED_MODEL: Mutex<String> = Mutex::new(config::get_selected_model());
    static ref MODEL_LOADING: Mutex<bool> = Mutex::new(false);
}

// Helpers for model selection and transcriber initialization
fn select_model_file(selected_model: &str, is_english: bool) -> String {
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

fn ensure_transcriber_for(
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
            println!("Initializing English transcriber with model: {}", resolved_model);
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
            println!("Initializing multilingual transcriber with model: {}", resolved_model);
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


// New helper to transcribe directly from in-memory samples
fn transcribe_samples_with(
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

/// Download base models during startup if they are missing
fn download_base_models() {
    let english_model = "ggml-base.en.bin";
    let multilingual_model = "ggml-base.bin";

    println!("Downloading base models...");

    // Download English model if it doesn't exist
    if config::get_model_path(english_model).is_none() {
        println!("Downloading English model...");
        if let Err(e) = WhisperTranscriber::download_model(english_model) {
            eprintln!("Failed to download English model: {}", e);
        }
    }

    // Download multilingual model if it doesn't exist
    if config::get_model_path(multilingual_model).is_none() {
        println!("Downloading multilingual model...");
        if let Err(e) = WhisperTranscriber::download_model(multilingual_model) {
            eprintln!("Failed to download multilingual model: {}", e);
        }
    }
}

fn main() {
    // keep the lock alive for the entire program
    let _instance_lock = single_instance::ensure_single_instance();

    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    let mut current_language = String::from("en");

    download_base_models();
    println!("Press Ctrl+CAPSLOCK to start recording, release to save and insert transcript at cursor position");


    // We'll initialize the transcribers on keydown instead of at startup
    let english_transcriber = Arc::new(Mutex::new(None));
    let multilingual_transcriber = Arc::new(Mutex::new(None));

    // Buffer to store recorded samples
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    // Create an audio stream for microphone recording
    let mut stream = AudioStream::new(recorded_samples.clone(), recording.clone())
        .expect("Failed to create audio stream");

    // Create a channel for keyboard events
    let (sender, receiver) = channel::<KeyboardEvent>();

    // Store the sender in the global static
    *KEYBOARD_EVENT_SENDER.lock().unwrap() = Some(sender);

    // Start listening for global keyboard events in a separate thread
    let _keyboard_thread = thread::spawn(move || {
        if let Err(e) = listen(handle_keyboard_event) {
            eprintln!("Failed to listen for keyboard events: {:?}", e);
        }
    });

    println!("Waiting for Ctrl+CAPSLOCK key combination (works even when app is not in focus)...");

    // Main loop to process events
    loop {
        // Check for keyboard events
        if let Ok(event) = receiver.try_recv() {
            match event {
                KeyboardEvent::CtrlCapsLockPressed => {
                    // Use the existing recording flag to guard "start" logic
                    let mut rec = recording.lock().unwrap();
                    if !*rec {
                        println!("Ctrl+CAPSLOCK pressed - Recording started");
                        *rec = true;

                        let keyboard_language =
                            KeyboardLayoutDetector::detect_language().unwrap_or_else(|_| String::from("en"));
                        let language_code = if keyboard_language.len() >= 2 {
                            keyboard_language[0..2].to_string()
                        } else {
                            String::from("en")
                        };
                        println!("Detected language code: {}", language_code);

                        // Store directly in local variable
                        current_language = language_code.clone();

                        // Clear previous recording and start new one
                        {
                            let mut samples = recorded_samples.lock().unwrap();
                            samples.clear();
                        }

                        // Resume the stream to start recording
                        stream.play().expect("Failed to start the stream");

                        // Update tray icon: recording (red)
                        #[cfg(feature = "tray-icon")]
                        {
                            crate::tray_icon::tray_icon_set_state("red");
                        }

                        // Initialize Whisper after starting recording
                        let is_english = language_code.starts_with("en");

                        // Get the selected model and resolve the model file
                        let selected_model = SELECTED_MODEL.lock().unwrap().clone();
                        let model_file = select_model_file(&selected_model, is_english);

                        // Ensure the appropriate transcriber is initialized (with fallback)
                        ensure_transcriber_for(
                            is_english,
                            &model_file,
                            &english_transcriber,
                            &multilingual_transcriber,
                        );
                    }
                },
                KeyboardEvent::CtrlCapsLockReleased => {
                    // Only process "stop/transcribe" if we were recording
                    let was_recording = {
                        let mut rec = recording.lock().unwrap();
                        let prev = *rec;
                        if prev {
                            *rec = false;
                        }
                        prev
                    };

                    if was_recording {
                        println!("Ctrl+CAPSLOCK released - Recording stopped, transcribing and inserting at cursor position");

                        // Pause the stream
                        stream.pause().expect("Failed to pause the stream");

                        // Update tray icon: processing/transcribing (blue)
                        #[cfg(feature = "tray-icon")]
                        {
                            crate::tray_icon::tray_icon_set_state("blue");
                        }

                        // Get the recorded samples
                        let samples = recorded_samples.lock().unwrap().clone();

                        if !samples.is_empty() {
                            println!("Processing recording for transcription");

                            // Use the local language variable
                            println!("Using language code for transcription: {}", current_language);
                            let is_english = current_language.starts_with("en");

                            let result = if is_english {
                                transcribe_samples_with(
                                    &english_transcriber,
                                    &samples,
                                    stream.get_sample_rate(),
                                    stream.get_channels(),
                                    &current_language,
                                )
                            } else {
                                transcribe_samples_with(
                                    &multilingual_transcriber,
                                    &samples,
                                    stream.get_sample_rate(),
                                    stream.get_channels(),
                                    &current_language,
                                )
                            };

                            match result {
                                Ok(transcript) => {
                                    println!("Transcription successful");
                                    println!(
                                        "Transcript preview: {}",
                                        transcript.lines().take(2).collect::<Vec<_>>().join(" ")
                                    );

                                    // Insert the transcript at the current cursor position in a separate thread to avoid blocking
                                    std::thread::spawn(move || {
                                        clipboard_inserter::insert_text(&transcript);
                                        println!("Transcript inserted");
                                    });
                                }
                                Err(e) => {
                                    eprintln!("{}", e);
                                }
                            }
                        }

                        // Back to ready: white
                        #[cfg(feature = "tray-icon")]
                        {
                            crate::tray_icon::tray_icon_set_state("white");
                        }
                    } else {
                        println!("No audio recorded");
                        // Back to ready: white
                        #[cfg(feature = "tray-icon")]
                        {
                            crate::tray_icon::tray_icon_set_state("white");
                        }
                    }
                }
            }
        }

        // Process GTK events if the tray-icon feature is enabled
        #[cfg(feature = "tray-icon")]
        {
            // Process any pending GTK events without blocking
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
        }

        // Sleep to reduce CPU usage
        thread::sleep(Duration::from_millis(10));
    }
}