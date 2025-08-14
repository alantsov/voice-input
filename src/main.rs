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
mod transcriber_utils;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use keyboard_layout::KeyboardLayoutDetector;
use hotkeys::{KeyboardEvent, KEYBOARD_EVENT_SENDER, handle_keyboard_event};
use transcriber_utils::{select_model_file, ensure_transcriber_for, transcribe_samples_with, download_base_models};

lazy_static! {
    static ref SELECTED_MODEL: Mutex<String> = Mutex::new(config::get_selected_model());
    static ref MODEL_LOADING: Mutex<bool> = Mutex::new(false);
}

// Semantic tray states instead of magic strings
enum TrayState {
    Ready,
    Recording,
    Processing,
}

impl TrayState {
    fn icon_key(&self) -> &'static str {
        match self {
            TrayState::Ready => "white",
            TrayState::Recording => "red",
            TrayState::Processing => "blue",
        }
    }
}

// Thin wrapper to update tray icon by semantic state
fn set_tray(state: TrayState) {
    #[cfg(feature = "tray-icon")]
    {
        crate::tray_icon::tray_icon_set_state(state.icon_key());
    }
}

// Centralized app state for the event loop
struct AppState {
    current_language: String,
    english_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
    multilingual_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
    recorded_samples: Arc<Mutex<Vec<f32>>>,
    recording: Arc<Mutex<bool>>,
    stream: AudioStream,
}

fn detect_language_code() -> String {
    let keyboard_language = KeyboardLayoutDetector::detect_language().unwrap_or_else(|_| String::from("en"));
    if keyboard_language.len() >= 2 {
        keyboard_language[0..2].to_string()
    } else {
        String::from("en")
    }
}

fn start_recording(state: &mut AppState) {
    // Guard "start" logic with the recording flag
    let mut rec = state.recording.lock().unwrap();
    if *rec {
        return;
    }
    println!("Ctrl+CAPSLOCK pressed - Recording started");
    *rec = true;

    // Detect and store language code
    let language_code = detect_language_code();
    println!("Detected language code: {}", language_code);
    state.current_language = language_code.clone();

    // Clear previous recording
    {
        let mut samples = state.recorded_samples.lock().unwrap();
        samples.clear();
    }

    // Start audio stream
    state.stream.play().expect("Failed to start the stream");
    set_tray(TrayState::Recording);

    // Initialize Whisper after starting recording
    let is_english = language_code.starts_with("en");

    // Resolve the model file based on selected model and language
    let selected_model = SELECTED_MODEL.lock().unwrap().clone();
    let model_file = select_model_file(&selected_model, is_english);

    // Ensure the appropriate transcriber is initialized (with fallback)
    ensure_transcriber_for(
        is_english,
        &model_file,
        &state.english_transcriber,
        &state.multilingual_transcriber,
    );
}

fn stop_and_transcribe(state: &mut AppState) {
    // Only process "stop/transcribe" if we were recording
    let was_recording = {
        let mut rec = state.recording.lock().unwrap();
        let prev = *rec;
        if prev {
            *rec = false;
        }
        prev
    };

    if !was_recording {
        println!("No audio recorded");
        set_tray(TrayState::Ready);
        return;
    }

    println!("Ctrl+CAPSLOCK released - Recording stopped, transcribing and inserting at cursor position");

    // Pause the stream
    state.stream.pause().expect("Failed to pause the stream");

    // Update tray icon: processing/transcribing
    set_tray(TrayState::Processing);

    // Get the recorded samples
    let samples = state.recorded_samples.lock().unwrap().clone();

    if !samples.is_empty() {
        println!("Processing recording for transcription");

        // Use stored language
        println!("Using language code for transcription: {}", state.current_language);
        let is_english = state.current_language.starts_with("en");

        let transcriber = if is_english {
            &state.english_transcriber
        } else {
            &state.multilingual_transcriber
        };

        let result = transcribe_samples_with(
            transcriber,
            &samples,
            state.stream.get_sample_rate(),
            state.stream.get_channels(),
            &state.current_language,
        );

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
    set_tray(TrayState::Ready);
}

fn main() {
    // keep the lock alive for the entire program
    let _instance_lock = single_instance::ensure_single_instance();

    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    download_base_models();
    println!("Press Ctrl+CAPSLOCK to start recording, release to save and insert transcript at cursor position");

    // Initialize shared components
    let english_transcriber = Arc::new(Mutex::new(None));
    let multilingual_transcriber = Arc::new(Mutex::new(None));

    // Buffer to store recorded samples
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    // Create an audio stream for microphone recording
    let stream = AudioStream::new(recorded_samples.clone(), recording.clone())
        .expect("Failed to create audio stream");

    // App state
    let mut app_state = AppState {
        current_language: String::from("en"),
        english_transcriber,
        multilingual_transcriber,
        recorded_samples,
        recording,
        stream,
    };

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
                    start_recording(&mut app_state);
                }
                KeyboardEvent::CtrlCapsLockReleased => {
                    stop_and_transcribe(&mut app_state);
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