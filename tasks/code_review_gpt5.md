Here’s a focused audit highlighting unused pieces, duplication, and places that feel overcomplicated, plus small, safe refactors you can apply right away.

Key findings

- Unused globals
    - main.rs: MODEL_LOADING is declared but never used. Remove it.

- Misleading/leftover code
    - main.rs: f12_pressed actually means “recording is active.” Rename to recording_active for clarity.
    - main.rs: A block that pushes “dummy samples” into recorded_samples is a leftover and corrupts real audio. Remove it.

- Duplicate logic
    - main.rs: Transcriber initialization logic is duplicated between English and Multilingual branches. The only difference is the selected model path. A helper that “get_or_init_transcriber(model_file, target)” would reduce branching and future bugs.
    - main.rs: On release (transcription), code for English vs Multilingual is also nearly identical except the transcriber source. Extract a run_transcription(transcriber, language) helper.

- Overcomplicated state
    - main.rs: CURRENT_LANGUAGE is stored in a thread_local just to pass from press to release within the same main thread loop. A simple local variable (e.g., current_language: Option<String>) or an Arc<Mutex<String>> is simpler and clearer.
    - main.rs: KEYBOARD_EVENT_SENDER is a global Mutex<Option<Sender<...>>> while you already have a Sender in scope. You can clone the sender into the rdev callback and avoid the global.

- Keyboard event handling is noisy and can double-fire release
    - main.rs: You send CtrlCapsLockReleased both on Ctrl release and on CapsLock release. Depending on user timing, you can trigger the release path twice. It’s simpler and more predictable to only send release on CapsLock release when Ctrl was pressed (or track an atomic state machine and emit a single release).

- Disk I/O and comments mismatch
    - main.rs: The comment says “Create a temporary WAV file in memory” but it writes to disk. Either use an in-memory approach or fix the comment. Writing to a timestamped file in CWD is brittle; prefer a real temporary file location.

- Eager downloads on startup
    - main.rs: Always downloading both base models increases startup cost. Consider lazy download: when selecting model_file, ensure it exists (download on demand if not).

- Config path fallback adds complexity
    - config.rs: get_model_path() falls back to the current directory. This adds ambiguity and surprises. Standardize on the XDG data directory and drop the current-dir fallback unless you truly need legacy migration.

- Assets generator overkill and inconsistency
    - voice_input_asset_generator.sh: Generates multiple color variants and sizes, detects two different renderers, and echoes a message that mentions only blue/red/yellow though it also generates white. If you only need one tray color set/states, simplify to the minimum set you actually use and fix the final echo.

Targeted patches

1) Remove unused MODEL_LOADING, rename f12_pressed, remove dummy samples, and avoid duplicate release events

```rust
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use chrono::Local;
use hound::{WavSpec, WavWriter};
use rdev::{listen, Event, EventType, Key};
use std::sync::mpsc::{channel, Sender};
use lazy_static::lazy_static;
use std::cell::RefCell;
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
mod clipboard_inserter;
mod config;

use audio_stream::AudioStream;
use whisper::WhisperTranscriber;
use keyboard_layout::KeyboardLayoutDetector;

// Define a type for keyboard events we're interested in
enum KeyboardEvent {
    CtrlCapsLockPressed,
    CtrlCapsLockReleased,
}

// Global channel for keyboard events and model selection
lazy_static! {
    static ref KEYBOARD_EVENT_SENDER: Mutex<Option<Sender<KeyboardEvent>>> = Mutex::new(None);
    static ref SELECTED_MODEL: Mutex<String> = Mutex::new(config::get_selected_model());
    // Removed: MODEL_LOADING (unused)
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
            // Removed duplicate release signal on Ctrl release to avoid double-firing
        },
        EventType::KeyPress(Key::CapsLock) => {
            if *CTRL_PRESSED.lock().unwrap() {
                if let Some(sender) = &*KEYBOARD_EVENT_SENDER.lock().unwrap() {
                    let _ = sender.send(KeyboardEvent::CtrlCapsLockPressed);
                }
            }
        },
        EventType::KeyRelease(Key::CapsLock) => {
            // Send release only when CapsLock is released
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

    if let Err(_) = lock_file.try_lock_exclusive() {
        eprintln!("Another instance of Voice Input is already running.");
        process::exit(0);
    }

    println!("Voice Input Application");
    println!("Press Ctrl+CAPSLOCK to start recording, release to save and insert transcript at cursor position");

    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    // Only download base models during startup
    let english_model = "ggml-base.en.bin";
    let multilingual_model = "ggml-base.bin";

    println!("Downloading base models...");

    if config::get_model_path(english_model).is_none() {
        println!("Downloading English model...");
        if let Err(e) = WhisperTranscriber::download_model(english_model) {
            eprintln!("Failed to download English model: {}", e);
        }
    }

    if config::get_model_path(multilingual_model).is_none() {
        println!("Downloading multilingual model...");
        if let Err(e) = WhisperTranscriber::download_model(multilingual_model) {
            eprintln!("Failed to download multilingual model: {}", e);
        }
    }

    let english_transcriber = Arc::new(Mutex::new(None));
    let multilingual_transcriber = Arc::new(Mutex::new(None));

    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    let mut stream = AudioStream::new(recorded_samples.clone(), recording.clone())
        .expect("Failed to create audio stream");

    let (sender, receiver) = channel::<KeyboardEvent>();
    *KEYBOARD_EVENT_SENDER.lock().unwrap() = Some(sender);

    let _keyboard_thread = thread::spawn(move || {
        if let Err(e) = listen(handle_keyboard_event) {
            eprintln!("Failed to listen for keyboard events: {:?}", e);
        }
    });

    println!("Waiting for Ctrl+CAPSLOCK key combination (works even when app is not in focus)...");

    // Renamed for clarity
    let mut recording_active = false;

    loop {
        if let Ok(event) = receiver.try_recv() {
            match event {
                KeyboardEvent::CtrlCapsLockPressed => {
                    if !recording_active {
                        println!("Ctrl+CAPSLOCK pressed - Recording started");
                        recording_active = true;

                        let keyboard_language = KeyboardLayoutDetector::detect_language()
                            .unwrap_or_else(|_| String::from("en"));

                        let language_code = if keyboard_language.len() >= 2 {
                            keyboard_language[0..2].to_string()
                        } else {
                            String::from("en")
                        };
                        println!("Detected language code: {}", language_code);

                        CURRENT_LANGUAGE.with(|lang| {
                            *lang.borrow_mut() = language_code.clone();
                        });

                        {
                            let mut samples = recorded_samples.lock().unwrap();
                            samples.clear();
                            *recording.lock().unwrap() = true;
                        }

                        stream.play().expect("Failed to start the stream");

                        #[cfg(feature = "tray-icon")]
                        {
                            crate::tray_icon::tray_icon_set_state("red");
                        }

                        let is_english = language_code.starts_with("en");
                        let selected_model = SELECTED_MODEL.lock().unwrap().clone();

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

                        if config::get_model_path(&model_file).is_none() {
                            println!("Model file {} does not exist. Using base model instead.", model_file);
                            if is_english {
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
                            if is_english {
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

                        // Removed: dummy data generation that polluted real audio
                        // for i in 0..1000 {
                        //     samples.push(0.1 * (i as f32 % 10.0));
                        // }
                    }
                },
                KeyboardEvent::CtrlCapsLockReleased => {
                    if recording_active {
                        println!("Ctrl+CAPSLOCK released - Recording stopped, transcribing and inserting at cursor position");
                        recording_active = false;

                        {
                            *recording.lock().unwrap() = false;
                        }

                        stream.pause().expect("Failed to pause the stream");

                        #[cfg(feature = "tray-icon")]
                        {
                            crate::tray_icon::tray_icon_set_state("blue");
                        }

                        // Create a temporary WAV file on disk for transcription
                        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                        let filename = format!("temp_voice_{}.wav", timestamp);

                        let samples = recorded_samples.lock().unwrap().clone();

                        if !samples.is_empty() {
                            println!("Processing recording for transcription");

                            let spec = WavSpec {
                                channels: stream.get_channels(),
                                sample_rate: stream.get_sample_rate(),
                                bits_per_sample: 32,
                                sample_format: hound::SampleFormat::Float,
                            };

                            let mut writer = WavWriter::create(&filename, spec)
                                .expect("Failed to create temporary WAV file");

                            for &sample in &samples {
                                writer.write_sample(sample).expect("Failed to write sample");
                            }

                            writer.finalize().expect("Failed to finalize temporary WAV file");

                            println!("Recording processed successfully");

                            let current_language = CURRENT_LANGUAGE.with(|lang| lang.borrow().clone());
                            println!("Using language code for transcription: {}", current_language);

                            let is_english = current_language.starts_with("en");

                            if is_english {
                                let english_guard = english_transcriber.lock().unwrap();
                                if let Some(ref t) = *english_guard {
                                    println!("Using English transcriber");
                                    match t.transcribe_audio(&filename, Some(&current_language)) {
                                        Ok(transcript) => {
                                            println!("Transcription successful");
                                            println!("Transcript preview: {}", 
                                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));
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
                                let multilingual_guard = multilingual_transcriber.lock().unwrap();
                                if let Some(ref t) = *multilingual_guard {
                                    println!("Using multilingual transcriber");
                                    match t.transcribe_audio(&filename, Some(&current_language)) {
                                        Ok(transcript) => {
                                            println!("Transcription successful");
                                            println!("Transcript preview: {}", 
                                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));
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

                            if let Err(e) = std::fs::remove_file(&filename) {
                                eprintln!("Warning: Failed to delete temporary file {}: {}", filename, e);
                            } else {
                                println!("Temporary file {} deleted", filename);
                            }

                            #[cfg(feature = "tray-icon")]
                            {
                                crate::tray_icon::tray_icon_set_state("white");
                            }
                        } else {
                            println!("No audio recorded");
                            #[cfg(feature = "tray-icon")]
                            {
                                crate::tray_icon::tray_icon_set_state("white");
                            }
                        }
                    }
                }
            }
        }

        #[cfg(feature = "tray-icon")]
        {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
        }

        thread::sleep(Duration::from_millis(10));
    }
}
```


2) Optional: simplify get_model_path behavior (drop current-dir fallback)

If you don’t need legacy fallback, this reduces ambiguity.

```rust
/// Get the full path for a model file
/// Returns Some(path) if the model file exists in the XDG data directory, otherwise None.
pub fn get_model_path(model_name: &str) -> Option<PathBuf> {
    if let Some(models_dir) = get_models_dir() {
        let xdg_path = models_dir.join(model_name);
        if xdg_path.exists() {
            return Some(xdg_path);
        }
    }
    None
}
```


Suggested next simplifications (no immediate patch)

- Unify transcriber usage paths
    - Introduce a helper that ensures the desired model exists (download-on-demand), initializes the correct WhisperTranscriber for the chosen model, and returns a &WhisperTranscriber regardless of language. This removes most branching.

- Replace thread_local CURRENT_LANGUAGE
    - Store language in a local variable captured between press/release or in Arc<Mutex<String>> if you need to share across threads. This removes an uncommon pattern from an otherwise simple single-threaded loop.

- Remove global KEYBOARD_EVENT_SENDER
    - You can capture a cloned Sender in the rdev listener closure instead of storing it globally. If you stick with a global, at least use an AtomicBool for CTRL_PRESSED for lower overhead.

- Lazy downloads
    - Don’t download any models on startup. When the user presses the hotkey, determine the model_file and ensure it’s present; if not, download it with a small progress indicator. This speeds up app start and avoids unused assets.

- Temporary WAV handling
    - Use a proper temp location (e.g., std::env::temp_dir().join(...)) or a tempfile-based approach. Also consider bypassing WAV altogether if whisper-rs accepts raw f32 PCM; then you can avoid disk I/O completely.

- Assets script trimming
    - Generate only the icon sizes and colors you actually use. Also fix the final echo to reflect all generated variants or remove unused colors (e.g., “white”) to avoid confusion.

Why these changes help

- Fewer globals and special cases means fewer race conditions and easier reasoning.
- Removing dummy writes and duplicated logic reduces bugs and maintenance cost.
- Clear names and single-source-of-truth helpers improve readability and simplify future enhancements (e.g., model selection UI).

If you’d like, I can follow up with a refactor that extracts a small TranscriptionEngine with:
- ensure_model(model_name)
- get_or_init_transcriber(model_name)
- transcribe(samples, language)
  This would collapse most of the branching in main.rs into three straightforward calls.