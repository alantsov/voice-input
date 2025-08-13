use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use chrono::Local;
use hound::{WavSpec, WavWriter};
use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc::{channel, Sender};
use lazy_static::lazy_static;
// Note: The enigo crate requires the libxdo-dev package on Linux
// Install it with: sudo apt-get install libxdo-dev
use std::cell::RefCell;
use std::fs::File;
use std::process;
use fs2::FileExt;
use directories::ProjectDirs;

mod tray_icon;
mod audio_stream;
mod whisper;
mod keyboard_layout;
mod clipboard_inserter;
mod config;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use keyboard_layout::KeyboardLayoutDetector;

/// Ensure only a single instance of the app is running by creating and locking a file.
/// Returns the opened lock file, which must be kept in scope for the duration of the program.
fn ensure_single_instance() -> File {
    // Implement single instance check
    let lock_file = if let Some(proj_dirs) = ProjectDirs::from("com", "voice-input", "voice-input") {
        let cache_dir = proj_dirs.cache_dir();
        std::fs::create_dir_all(cache_dir).unwrap_or_else(|e| {
            eprintln!("Failed to create cache directory: {}", e);
            process::exit(1);
        });
        let lock_path = cache_dir.join("voice-input.lock");
        match File::create(&lock_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to create lock file: {}", e);
                process::exit(1);
            }
        }
    } else {
        // Fallback to temp directory if ProjectDirs fails
        let lock_path = std::env::temp_dir().join("voice-input.lock");
        match File::create(&lock_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to create lock file: {}", e);
                process::exit(1);
            }
        }
    };

    // Try to acquire an exclusive lock
    // The lock will be automatically released when the program exits
    // or when the returned file goes out of scope
    if let Err(_) = lock_file.try_lock_exclusive() {
        eprintln!("Another instance of Voice Input is already running.");
        process::exit(0);
    }

    lock_file
}

/// Detect the current keyboard layout and return its language code
// Define a type for keyboard events we're interested in
enum KeyboardEvent {
    CtrlCapsLockPressed,
    CtrlCapsLockReleased,
}

// Global channel for keyboard events and model selection
lazy_static! {
    static ref KEYBOARD_EVENT_SENDER: Mutex<Option<Sender<KeyboardEvent>>> = Mutex::new(None);
    static ref SELECTED_MODEL: Mutex<String> = Mutex::new(config::get_selected_model());
    static ref MODEL_LOADING: Mutex<bool> = Mutex::new(false);
    static ref CTRL_PRESSED: Mutex<bool> = Mutex::new(false);
}

// Function to handle keyboard events globally
fn handle_keyboard_event(event: Event) {
    // We're interested in Ctrl+CAPSLOCK key combination
    match event.event_type {
        EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) => {
            *CTRL_PRESSED.lock().unwrap() = true;
        },
        EventType::KeyRelease(Key::ControlLeft) | EventType::KeyRelease(Key::ControlRight) => {
            *CTRL_PRESSED.lock().unwrap() = false;
            // Send CtrlCapsLockReleased event when Ctrl is released
            if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                let _ = sender.send(KeyboardEvent::CtrlCapsLockReleased);
            }
        },
        EventType::KeyPress(Key::CapsLock) => {
            if *CTRL_PRESSED.lock().unwrap() {
                if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                    let _ = sender.send(KeyboardEvent::CtrlCapsLockPressed);
                }
            }
        },
        EventType::KeyRelease(Key::CapsLock) => {
            // Send CtrlCapsLockReleased event when CAPSLOCK is released, regardless of Ctrl state
            if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                let _ = sender.send(KeyboardEvent::CtrlCapsLockReleased);
            }
        },
        _ => {}
    }
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

// New helper to deduplicate transcription call
fn transcribe_with(
    transcriber: &Arc<Mutex<Option<WhisperTranscriber>>>,
    filename: &str,
    language: &str,
) -> Result<String, String> {
    let guard = transcriber
        .lock()
        .map_err(|_| "Failed to lock transcriber".to_string())?;
    if let Some(ref t) = *guard {
        t.transcribe_audio(filename, Some(language))
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
    let _instance_lock = ensure_single_instance();

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

                        // Create a temporary WAV file in memory for transcription
                        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                        let filename = format!("temp_voice_{}.wav", timestamp);

                        // Get the recorded samples
                        let samples = recorded_samples.lock().unwrap().clone();

                        if !samples.is_empty() {
                            println!("Processing recording for transcription");

                            // Create a WAV file in memory for transcription
                            let spec = WavSpec {
                                channels: stream.get_channels(),
                                sample_rate: stream.get_sample_rate(),
                                bits_per_sample: 32,
                                sample_format: hound::SampleFormat::Float,
                            };

                            let mut writer = WavWriter::create(&filename, spec)
                                .expect("Failed to create temporary WAV file");

                            // Write the samples to the WAV file
                            for &sample in &samples {
                                writer.write_sample(sample).expect("Failed to write sample");
                            }

                            writer.finalize().expect("Failed to finalize temporary WAV file");

                            println!("Recording processed successfully");

                            // Use the local language variable
                            println!("Using language code for transcription: {}", current_language);
                            let is_english = current_language.starts_with("en");

                            let result = if is_english {
                                transcribe_with(&english_transcriber, &filename, &current_language)
                            } else {
                                transcribe_with(&multilingual_transcriber, &filename, &current_language)
                            };

                            match result {
                                Ok(transcript) => {
                                    println!("Transcription successful");
                                    println!("Transcript preview: {}", 
                                             transcript.lines().take(2).collect::<Vec<_>>().join(" "));

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

                        // Delete the temporary WAV file
                        if let Err(e) = std::fs::remove_file(&filename) {
                            eprintln!("Warning: Failed to delete temporary file {}: {}", filename, e);
                        } else {
                            println!("Temporary file {} deleted", filename);
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