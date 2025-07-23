use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use device_query::{DeviceQuery, DeviceState, Keycode};
use chrono::Local;
use hound::{WavSpec, WavWriter};
use sys_locale::get_locale;
use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc::{channel, Sender, Receiver};
use lazy_static::lazy_static;
// Note: The enigo crate requires the libxdo-dev package on Linux
// Install it with: sudo apt-get install libxdo-dev
use enigo::{Enigo, KeyboardControllable};

#[cfg(feature = "tray-icon")]
use gtk::prelude::*;

mod tray_icon;
mod audio_stream;
mod whisper;
use audio_stream::AudioStream;
use whisper::WhisperTranscriber;

/// Detect the current keyboard layout and return its language code
// Define a type for keyboard events we're interested in
enum KeyboardEvent {
    F12Pressed,
    F12Released,
}

// Global channel for keyboard events
lazy_static! {
    static ref KEYBOARD_EVENT_SENDER: Mutex<Option<Sender<KeyboardEvent>>> = Mutex::new(None);
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
}

fn detect_keyboard_layout() -> Result<String, String> {
    // This is a simplified implementation that uses the system locale as a fallback
    // In a real implementation, you would use the input-linux crate to detect the keyboard layout

    // For now, we'll use the system locale as a fallback
    let locale = get_locale().unwrap_or_else(|| String::from("en-US"));

    // Try to detect the active keyboard layout using xkb-switch
    let output = match std::process::Command::new("xkb-switch")
        .output() {
        Ok(output) => {
            if output.status.success() {
                let layout_code = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!("xkb-switch output: {}", layout_code);

                // Map layout codes to language codes
                match layout_code.as_str() {
                    "us" | "gb" => Some("en".to_string()),
                    "de" => Some("de".to_string()),
                    "fr" => Some("fr".to_string()),
                    "es" => Some("es".to_string()),
                    "it" => Some("it".to_string()),
                    "ru" => Some("ru".to_string()),
                    _ => {
                        println!("Unknown keyboard layout: {}, falling back to /etc/default/keyboard", layout_code);
                        // If we can't determine the language from the active layout, fall back to /etc/default/keyboard
                        None
                    }
                }
            } else {
                println!("xkb-switch command failed, falling back to /etc/default/keyboard");
                None
            }
        },
        Err(e) => {
            println!("Failed to execute xkb-switch: {}, falling back to /etc/default/keyboard", e);
            None
        }
    };

    // If we got a language from xkb-switch, use it
    if let Some(lang) = output {
        println!("Detected keyboard layout language from xkb-switch: {}", lang);
        return Ok(lang);
    }

    // Fall back to /etc/default/keyboard if xkb-switch failed
    println!("Falling back to /etc/default/keyboard");
    let keyboard_layout = match std::fs::read_to_string("/etc/default/keyboard") {
        Ok(content) => {
            // Parse the keyboard layout from the file
            // Look for XKBLAYOUT=xx pattern
            if let Some(layout_line) = content.lines().find(|line| line.starts_with("XKBLAYOUT=")) {
                // Extract the layout code (everything after XKBLAYOUT=)
                let layout_code = layout_line.trim_start_matches("XKBLAYOUT=").trim_matches('"');
                println!("Found layout code in /etc/default/keyboard: {}", layout_code);

                // Map layout codes to language codes
                match layout_code {
                    "us" | "gb" => "en".to_string(),
                    "de" => "de".to_string(),
                    "fr" => "fr".to_string(),
                    "es" => "es".to_string(),
                    "it" => "it".to_string(),
                    "ru" => "ru".to_string(),
                    _ => {
                        println!("Unknown keyboard layout: {}, falling back to locale", layout_code);
                        // If we can't determine the language from the layout, fall back to the system locale
                        if locale.len() >= 2 {
                            locale[0..2].to_string()
                        } else {
                            "en".to_string()
                        }
                    }
                }
            } else {
                println!("Could not find XKBLAYOUT in keyboard configuration, falling back to locale");
                // If we can't find the layout in the file, fall back to the system locale
                if locale.len() >= 2 {
                    locale[0..2].to_string()
                } else {
                    "en".to_string()
                }
            }
        },
        Err(e) => {
            println!("Could not read keyboard configuration: {}, falling back to locale", e);
            // If we can't read the file, fall back to the system locale
            if locale.len() >= 2 {
                locale[0..2].to_string()
            } else {
                "en".to_string()
            }
        }
    };

    println!("Detected keyboard layout language: {}", keyboard_layout);
    Ok(keyboard_layout)
}

fn main() {
    println!("Voice Input Application");
    println!("Press F12 to start recording, release to save and insert transcript at cursor position");

    // Initialize the system tray icon if the feature is enabled
    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    // Detect keyboard layout language
    let keyboard_language = detect_keyboard_layout().unwrap_or_else(|_| String::from("en"));

    // Extract language code from keyboard layout (first 2 characters)
    let language_code = if keyboard_language.len() >= 2 {
        keyboard_language[0..2].to_string()
    } else {
        String::from("en")
    };
    println!("Using language code: {}", language_code);

    // Determine which model to use based on language
    let is_english = language_code.starts_with("en");
    let model_path = if is_english {
        "ggml-base.en.bin"
    } else {
        "ggml-base.bin"
    };

    println!("Using model: {}", model_path);

    // Download both models during startup
    let english_model = "ggml-base.en.bin";
    let multilingual_model = "ggml-base.bin";

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

    // Initialize the WhisperTranscriber with the appropriate model
    // Models are downloaded from: https://huggingface.co/ggerganov/whisper.cpp
    let transcriber = match WhisperTranscriber::new(model_path) {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("Failed to initialize WhisperTranscriber: {}", e);
            eprintln!("Audio transcription will be disabled");
            None
        }
    };

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

                            // Transcribe the audio file if transcriber is available
                            if let Some(ref t) = transcriber {
                                // Pass the language code to the transcribe_audio method
                                match t.transcribe_audio(&filename, Some(&language_code)) {
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
