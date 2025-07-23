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
use enigo::{Enigo, KeyboardControllable};
use std::cell::RefCell;

// Thread-local storage for the current language code
thread_local! {
    static CURRENT_LANGUAGE: RefCell<String> = RefCell::new(String::from("en"));
}


mod tray_icon;
mod audio_stream;
mod whisper;
mod keyboard_layout;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use keyboard_layout::KeyboardLayoutDetector;


/// Detect the current keyboard layout and return its language code
// Define a type for keyboard events we're interested in
enum KeyboardEvent {
    F12Pressed,
    F12Released,
}

// Global channel for keyboard events and model selection
lazy_static! {
    static ref KEYBOARD_EVENT_SENDER: Mutex<Option<Sender<KeyboardEvent>>> = Mutex::new(None);
    static ref SELECTED_MODEL: Mutex<String> = Mutex::new(String::from("base"));
    static ref MODEL_LOADING: Mutex<bool> = Mutex::new(false);
    static ref KEY_RELEASE_TIME: Mutex<Option<Instant>> = Mutex::new(None);
}

// Function to handle keyboard events globally
fn handle_keyboard_event(event: Event) {
    // We're only interested in F12 key events
    match event.event_type {
        EventType::KeyPress(Key::F12) => {
            if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                let _ = sender.send(KeyboardEvent::F12Pressed);
            }
        },
        EventType::KeyRelease(Key::F12) => {
            if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                let _ = sender.send(KeyboardEvent::F12Released);
            }
        },
        _ => {}
    }
}

// Function to simulate typing text at the current cursor position
fn simulate_typing(text: &str) {
    println!("Simulating typing: {}", text);

    // Add a small delay to ensure the application is ready
    thread::sleep(Duration::from_millis(500));

    // Create a new Enigo instance
    let mut enigo = Enigo::new();

    // Type the text character by character
    for c in text.chars() {
        // Type the character
        enigo.key_sequence(&c.to_string());

        // Add a small delay between keystrokes
        thread::sleep(Duration::from_millis(5));
    }

    // Calculate and log the time between key release and end of simulated typing
    if let Some(start_time) = *KEY_RELEASE_TIME.lock().unwrap() {
        let elapsed = start_time.elapsed();
        println!("Time between key release and end of simulated typing: {:.2} seconds", elapsed.as_secs_f64());
    }
}


fn main() {
    println!("Voice Input Application");
    println!("Press F12 to start recording, release to save and insert transcript at cursor position");

    // Initialize the system tray icon if the feature is enabled
    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    // Only download base models during startup
    let english_model = "ggml-base.en.bin";
    let multilingual_model = "ggml-base.bin";

    println!("Downloading base models...");

    // Download English model if it doesn't exist
    if !std::path::Path::new(english_model).exists() {
        println!("Downloading English model...");
        if let Err(e) = WhisperTranscriber::download_model(english_model) {
            eprintln!("Failed to download English model: {}", e);
        }
    }

    // Download multilingual model if it doesn't exist
    if !std::path::Path::new(multilingual_model).exists() {
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

    println!("Waiting for F12 key (works even when app is not in focus)...");

    let mut f12_pressed = false;

    // Main loop to process events
    loop {
        // Check for keyboard events
        if let Ok(event) = receiver.try_recv() {
            match event {
                KeyboardEvent::F12Pressed => {
                    if !f12_pressed {
                        println!("F12 pressed - Recording started");
                        f12_pressed = true;

                        // Detect keyboard layout language on keydown
                        let keyboard_language = KeyboardLayoutDetector::detect_language().unwrap_or_else(|_| String::from("en"));

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

                        // Initialize Whisper on keydown
                        let is_english = language_code.starts_with("en");

                        // Get the selected model
                        let selected_model = SELECTED_MODEL.lock().unwrap().clone();

                        // Determine the model file based on the selected model and language
                        let model_file = if is_english {
                            match selected_model.as_str() {
                                "base" => english_model.to_string(),
                                "small" | "medium" => format!("ggml-{}.en.bin", selected_model),
                                "large" => format!("ggml-{}-v2.bin", selected_model),
                                _ => english_model.to_string()
                            }
                        } else {
                            match selected_model.as_str() {
                                "base" => multilingual_model.to_string(),
                                "small" | "medium" | "large" => format!("ggml-{}-v2.bin", selected_model),
                                _ => multilingual_model.to_string()
                            }
                        };

                        // Check if the model file exists
                        if !std::path::Path::new(&model_file).exists() {
                            println!("Model file {} does not exist. Using base model instead.", model_file);

                            // Use base model as fallback
                            if is_english {
                                // Initialize English transcriber if not already initialized
                                let mut english_guard = english_transcriber.lock().unwrap();
                                if english_guard.is_none() {
                                    println!("Initializing English transcriber on keydown");
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
                                    println!("Initializing multilingual transcriber on keydown");
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

                        // Clear previous recording and start new one
                        {
                            let mut samples = recorded_samples.lock().unwrap();
                            samples.clear();
                            *recording.lock().unwrap() = true;
                        }

                        // Resume the stream to start recording
                        stream.play().expect("Failed to start the stream");

                        // Generate some dummy data for demonstration
                        let mut samples = recorded_samples.lock().unwrap();
                        for i in 0..1000 {
                            samples.push(0.1 * (i as f32 % 10.0));
                        }
                    }
                },
                KeyboardEvent::F12Released => {
                    if f12_pressed {
                        println!("F12 released - Recording stopped, transcribing and inserting at cursor position");
                        f12_pressed = false;

                        // Record the time when F12 is released
                        *KEY_RELEASE_TIME.lock().unwrap() = Some(Instant::now());

                        // Stop recording
                        {
                            *recording.lock().unwrap() = false;
                        }

                        // Pause the stream
                        stream.pause().expect("Failed to pause the stream");

                        // Save the recorded audio
                        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                        let filename = format!("voice_{}.wav", timestamp);

                        // Get the recorded samples
                        let samples = recorded_samples.lock().unwrap().clone();

                        if !samples.is_empty() {
                            println!("Saving recording to {}", filename);

                            // Create a WAV file
                            let spec = WavSpec {
                                channels: stream.get_channels(),
                                sample_rate: stream.get_sample_rate(),
                                bits_per_sample: 32,
                                sample_format: hound::SampleFormat::Float,
                            };

                            let mut writer = WavWriter::create(&filename, spec)
                                .expect("Failed to create WAV file");

                            // Write the samples to the WAV file
                            for &sample in &samples {
                                writer.write_sample(sample).expect("Failed to write sample");
                            }

                            writer.finalize().expect("Failed to finalize WAV file");
                            println!("Recording saved successfully to {}", filename);

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
                                    match t.transcribe_audio(&filename, Some(&current_language)) {
                                        Ok(transcript) => {
                                            println!("Transcription successful");
                                            println!("Transcript preview: {}", 
                                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));

                                            // Insert the transcript at the current cursor position
                                            simulate_typing(&transcript);
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
                                    match t.transcribe_audio(&filename, Some(&current_language)) {
                                        Ok(transcript) => {
                                            println!("Transcription successful");
                                            println!("Transcript preview: {}", 
                                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));

                                            // Insert the transcript at the current cursor position
                                            simulate_typing(&transcript);
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
