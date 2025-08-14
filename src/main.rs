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
mod app;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use hotkeys::{KeyboardEvent, KEYBOARD_EVENT_SENDER, handle_keyboard_event};
use transcriber_utils::download_base_models;

lazy_static! {
    static ref SELECTED_MODEL: Mutex<String> = Mutex::new(config::get_selected_model());
    static ref MODEL_LOADING: Mutex<bool> = Mutex::new(false);
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
    let english_transcriber: Arc<Mutex<Option<WhisperTranscriber>>> = Arc::new(Mutex::new(None));
    let multilingual_transcriber: Arc<Mutex<Option<WhisperTranscriber>>> = Arc::new(Mutex::new(None));

    // Buffer to store recorded samples
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    // Create an audio stream for microphone recording
    let stream = AudioStream::new(recorded_samples.clone(), recording.clone())
        .expect("Failed to create audio stream");

    // Create the application instance
    let mut app = app::App::new(
        stream,
        recorded_samples,
        recording,
        english_transcriber,
        multilingual_transcriber,
    );

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

    // Hand over to the app's event loop
    app.run_loop(receiver);
}