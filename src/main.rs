use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use chrono::Local;
use hound::{WavSpec, WavWriter};
use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc::{channel, Sender};
use lazy_static::lazy_static;
// Note: The enigo crate requires the libxdo-dev package on Linux
// Install it with: sudo apt-get install libxdo-dev
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::process;
use fs2::FileExt;
use directories::ProjectDirs;

// Thread-local storage for the current language code
thread_local! {
    static CURRENT_LANGUAGE: RefCell<String> = RefCell::new(String::from("en"));
}


mod tray_icon;
mod audio_stream;
mod whisper;
mod keyboard_layout;
mod keyboard_simulator;
mod clipboard_inserter;
mod config;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use keyboard_layout::KeyboardLayoutDetector;


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
    static ref KEY_PRESS_TIME: Mutex<Option<Instant>> = Mutex::new(None);
    static ref KEY_RELEASE_TIME: Mutex<Option<Instant>> = Mutex::new(None);
    static ref TIMING_INFO: Mutex<HashMap<String, Duration>> = Mutex::new(HashMap::new());
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


fn main() {
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
    // or when the lock_file variable goes out of scope
    if let Err(_) = lock_file.try_lock_exclusive() {
        eprintln!("Another instance of Voice Input is already running.");
        process::exit(0);
    }

    // Keep the lock_file variable in scope for the entire program
    // This ensures the lock is held until the program exits

    println!("Voice Input Application");
    println!("Press Ctrl+CAPSLOCK to start recording, release to save and insert transcript at cursor position");

    // Initialize the system tray icon if the feature is enabled
    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    // Only download base models during startup
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

    let mut f12_pressed = false;

    // Main loop to process events
    loop {
        // Check for keyboard events
        if let Ok(event) = receiver.try_recv() {
            match event {
                KeyboardEvent::CtrlCapsLockPressed => {
                    if !f12_pressed {
                        println!("Ctrl+CAPSLOCK pressed - Recording started");
                        f12_pressed = true;

                        // Record the time when Ctrl+CAPSLOCK is pressed
                        let start_time = Instant::now();
                        *KEY_PRESS_TIME.lock().unwrap() = Some(start_time);

                        // Clear previous timing information
                        TIMING_INFO.lock().unwrap().clear();

                        // Detect keyboard layout language on keydown
                        let lang_detection_start = Instant::now();
                        let keyboard_language = KeyboardLayoutDetector::detect_language().unwrap_or_else(|_| String::from("en"));
                        TIMING_INFO.lock().unwrap().insert("language_detection".to_string(), lang_detection_start.elapsed());

                        // Extract language code from keyboard layout (first 2 characters)
                        let language_code = if keyboard_language.len() >= 2 {
                            keyboard_language[0..2].to_string()
                        } else {
                            String::from("en")
                        };
                        println!("Detected language code: {}", language_code);

                        // Store the language code for later use
                        CURRENT_LANGUAGE.with(|lang| {
                            *lang.borrow_mut() = language_code.clone();
                        });

                        // Clear previous recording and start new one
                        let clear_recording_start = Instant::now();
                        {
                            let mut samples = recorded_samples.lock().unwrap();
                            samples.clear();
                            *recording.lock().unwrap() = true;
                        }
                        TIMING_INFO.lock().unwrap().insert("clear_recording".to_string(), clear_recording_start.elapsed());

                        // Resume the stream to start recording
                        stream.play().expect("Failed to start the stream");

                        // Initialize Whisper after starting recording
                        let whisper_init_start = Instant::now();
                        let is_english = language_code.starts_with("en");

                        // Get the selected model
                        let selected_model = SELECTED_MODEL.lock().unwrap().clone();

                        // Determine the model file based on the selected model and language
                        let model_file = if is_english {
                            match selected_model.as_str() {
                                "base" | "small" | "medium" => format!("ggml-{}.en.bin", selected_model),
                                "large" => format!("ggml-{}-v2.bin", selected_model),
                                _ => english_model.to_string()
                            }
                        } else {
                            match selected_model.as_str() {
                                "base" | "small" | "medium" => format!("ggml-{}.bin", selected_model),
                                "large" => format!("ggml-{}-v2.bin", selected_model),
                                _ => multilingual_model.to_string()
                            }
                        };

                        // Check if the model file exists in XDG data directory or current directory
                        if config::get_model_path(&model_file).is_none() {
                            println!("Model file {} does not exist. Using base model instead.", model_file);

                            // Use base model as fallback
                            if is_english {
                                // Initialize English transcriber if not already initialized
                                let mut english_guard = english_transcriber.lock().unwrap();
                                if english_guard.is_none() {
                                    println!("Initializing English transcriber after starting recording");
                                    match WhisperTranscriber::new(english_model) {
                                        Ok(t) => *english_guard = Some(t),
                                        Err(e) => {
                                            eprintln!("Failed to initialize English WhisperTranscriber: {}", e);
                                            eprintln!("English transcription will be disabled");
                                        }
                                    }
                                }
                            } else {
                                // Initialize multilingual transcriber if not already initialized
                                let mut multilingual_guard = multilingual_transcriber.lock().unwrap();
                                if multilingual_guard.is_none() {
                                    println!("Initializing multilingual transcriber after starting recording");
                                    match WhisperTranscriber::new(multilingual_model) {
                                        Ok(t) => *multilingual_guard = Some(t),
                                        Err(e) => {
                                            eprintln!("Failed to initialize Multilingual WhisperTranscriber: {}", e);
                                            eprintln!("Multilingual transcription will be disabled");
                                        }
                                    }
                                }
                            }
                        } else {
                            // Use the selected model
                            if is_english {
                                // Initialize English transcriber with the selected model
                                let mut english_guard = english_transcriber.lock().unwrap();
                                println!("Initializing English transcriber with model: {}", model_file);
                                match WhisperTranscriber::new(&model_file) {
                                    Ok(t) => *english_guard = Some(t),
                                    Err(e) => {
                                        eprintln!("Failed to initialize English WhisperTranscriber with model {}: {}", model_file, e);
                                        eprintln!("English transcription will be disabled");
                                    }
                                }
                            } else {
                                // Initialize multilingual transcriber with the selected model
                                let mut multilingual_guard = multilingual_transcriber.lock().unwrap();
                                println!("Initializing multilingual transcriber with model: {}", model_file);
                                match WhisperTranscriber::new(&model_file) {
                                    Ok(t) => *multilingual_guard = Some(t),
                                    Err(e) => {
                                        eprintln!("Failed to initialize Multilingual WhisperTranscriber with model {}: {}", model_file, e);
                                        eprintln!("Multilingual transcription will be disabled");
                                    }
                                }
                            }
                        }

                        // Record time for Whisper initialization
                        TIMING_INFO.lock().unwrap().insert("whisper_initialization".to_string(), whisper_init_start.elapsed());

                        // Calculate and display total time from key press to stream start
                        if let Some(press_time) = *KEY_PRESS_TIME.lock().unwrap() {
                            let total_time = press_time.elapsed();
                            println!("Time from key press to stream start: {} ms", total_time.as_millis());

                            // Display breakdown of timing
                            let timing_info = TIMING_INFO.lock().unwrap();
                            println!("Timing breakdown:");
                            if let Some(time) = timing_info.get("language_detection") {
                                println!("  Language detection: {} ms", time.as_millis());
                            }
                            if let Some(time) = timing_info.get("whisper_initialization") {
                                println!("  Whisper initialization: {} ms", time.as_millis());
                            }
                            if let Some(time) = timing_info.get("clear_recording") {
                                println!("  Clear recording: {} ms", time.as_millis());
                            }

                            // Calculate other time (time not accounted for in the breakdown)
                            let accounted_time = timing_info.values().fold(Duration::from_millis(0), |acc, &val| acc + val);
                            let other_time = total_time.checked_sub(accounted_time).unwrap_or(Duration::from_millis(0));
                            println!("  Other operations: {} ms", other_time.as_millis());
                        }

                        // Generate some dummy data for demonstration
                        let mut samples = recorded_samples.lock().unwrap();
                        for i in 0..1000 {
                            samples.push(0.1 * (i as f32 % 10.0));
                        }
                    }
                },
                KeyboardEvent::CtrlCapsLockReleased => {
                    if f12_pressed {
                        println!("Ctrl+CAPSLOCK released - Recording stopped, transcribing and inserting at cursor position");
                        f12_pressed = false;

                        // Record the time when Ctrl+CAPSLOCK is released
                        let start_time = Instant::now();
                        *KEY_RELEASE_TIME.lock().unwrap() = Some(start_time);

                        // Clear previous timing information
                        TIMING_INFO.lock().unwrap().clear();

                        // Stop recording
                        {
                            *recording.lock().unwrap() = false;
                        }

                        // Pause the stream
                        stream.pause().expect("Failed to pause the stream");

                        // Record time for stopping recording
                        TIMING_INFO.lock().unwrap().insert("stop_recording".to_string(), start_time.elapsed());

                        // Create a temporary WAV file in memory for transcription
                        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                        let filename = format!("temp_voice_{}.wav", timestamp);

                        // Get the recorded samples
                        let samples = recorded_samples.lock().unwrap().clone();

                        if !samples.is_empty() {
                            println!("Processing recording for transcription");

                            // Start timing for WAV file processing
                            let save_start = Instant::now();

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

                            // Record time for WAV file processing
                            TIMING_INFO.lock().unwrap().insert("process_wav".to_string(), save_start.elapsed());

                            println!("Recording processed successfully");

                            // Get the current language code
                            let current_language = CURRENT_LANGUAGE.with(|lang| lang.borrow().clone());
                            println!("Using language code for transcription: {}", current_language);

                            // Determine which transcriber to use based on language
                            let is_english = current_language.starts_with("en");

                            if is_english {
                                // Use English transcriber
                                let english_guard = english_transcriber.lock().unwrap();
                                if let Some(ref t) = *english_guard {
                                    println!("Using English transcriber");

                                    // Start timing for transcription (handled by transcribe_audio)
                                    let _transcribe_start = Instant::now();

                                    match t.transcribe_audio(&filename, Some(&current_language)) {
                                        Ok((transcript, conversion_time, transcription_time)) => {
                                            // Record times for audio conversion and actual transcription
                                            TIMING_INFO.lock().unwrap().insert("audio_conversion".to_string(), conversion_time);
                                            TIMING_INFO.lock().unwrap().insert("actual_transcription".to_string(), transcription_time);

                                            println!("Transcription successful");
                                            println!("Transcript preview: {}", 
                                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));

                                            // Insert the transcript at the current cursor position
                                            //keyboard_simulator::simulate_typing(&transcript);
                                            clipboard_inserter::insert_text(&transcript);
                                            println!("Transcript inserted at cursor position");
                                        },
                                        Err(e) => {
                                            eprintln!("Failed to transcribe audio: {}", e);
                                        }
                                    }
                                } else {
                                    eprintln!("English transcriber is not available");
                                }
                            } else {
                                // Use multilingual transcriber
                                let multilingual_guard = multilingual_transcriber.lock().unwrap();
                                if let Some(ref t) = *multilingual_guard {
                                    println!("Using multilingual transcriber");

                                    // Start timing for transcription (handled by transcribe_audio)
                                    let _transcribe_start = Instant::now();

                                    match t.transcribe_audio(&filename, Some(&current_language)) {
                                        Ok((transcript, conversion_time, transcription_time)) => {
                                            // Record times for audio conversion and actual transcription
                                            TIMING_INFO.lock().unwrap().insert("audio_conversion".to_string(), conversion_time);
                                            TIMING_INFO.lock().unwrap().insert("actual_transcription".to_string(), transcription_time);

                                            println!("Transcription successful");
                                            println!("Transcript preview: {}", 
                                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));

                                            // Insert the transcript at the current cursor position
                                            //keyboard_simulator::simulate_typing(&transcript);
                                            clipboard_inserter::insert_text(&transcript);
                                            println!("Transcript inserted at cursor position");
                                        },
                                        Err(e) => {
                                            eprintln!("Failed to transcribe audio: {}", e);
                                        }
                                    }
                                } else {
                                    eprintln!("Multilingual transcriber is not available");
                                }
                            }

                            // Delete the temporary WAV file
                            if let Err(e) = std::fs::remove_file(&filename) {
                                eprintln!("Warning: Failed to delete temporary file {}: {}", filename, e);
                            } else {
                                println!("Temporary file {} deleted", filename);
                            }
                        } else {
                            println!("No audio recorded");
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
