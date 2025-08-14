use std::sync::{Arc, Mutex};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use crate::audio_stream::AudioStream;
use crate::clipboard_inserter;
use crate::hotkeys::KeyboardEvent;
use crate::keyboard_layout::KeyboardLayoutDetector;
use crate::transcriber_utils::{ensure_transcriber_for, select_model_file, transcribe_samples_with};
use crate::whisper::WhisperTranscriber;
use crate::SELECTED_MODEL;

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

pub struct App {
    state: AppState,
}

impl App {
    pub fn new(
        stream: AudioStream,
        recorded_samples: Arc<Mutex<Vec<f32>>>,
        recording: Arc<Mutex<bool>>,
        english_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
        multilingual_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
    ) -> Self {
        Self {
            state: AppState {
                current_language: String::from("en"),
                english_transcriber,
                multilingual_transcriber,
                recorded_samples,
                recording,
                stream,
            },
        }
    }

    fn start_recording(&mut self) {
        // Guard "start" logic with the recording flag
        let mut rec = self.state.recording.lock().unwrap();
        if *rec {
            return;
        }
        println!("Ctrl+CAPSLOCK pressed - Recording started");
        *rec = true;

        // Detect and store language code
        let language_code = detect_language_code();
        println!("Detected language code: {}", language_code);
        self.state.current_language = language_code.clone();

        // Clear previous recording
        {
            let mut samples = self.state.recorded_samples.lock().unwrap();
            samples.clear();
        }

        // Start audio stream
        self.state.stream.play().expect("Failed to start the stream");
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
            &self.state.english_transcriber,
            &self.state.multilingual_transcriber,
        );
    }

    fn stop_and_transcribe(&mut self) {
        // Only process "stop/transcribe" if we were recording
        let was_recording = {
            let mut rec = self.state.recording.lock().unwrap();
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
        self.state.stream.pause().expect("Failed to pause the stream");

        // Update tray icon: processing/transcribing
        set_tray(TrayState::Processing);

        // Get the recorded samples
        let samples = self.state.recorded_samples.lock().unwrap().clone();

        if !samples.is_empty() {
            println!("Processing recording for transcription");

            // Use stored language
            println!("Using language code for transcription: {}", self.state.current_language);
            let is_english = self.state.current_language.starts_with("en");

            let transcriber = if is_english {
                &self.state.english_transcriber
            } else {
                &self.state.multilingual_transcriber
            };

            let result = transcribe_samples_with(
                transcriber,
                &samples,
                self.state.stream.get_sample_rate(),
                self.state.stream.get_channels(),
                &self.state.current_language,
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

    pub fn run_loop(&mut self, receiver: Receiver<KeyboardEvent>) -> ! {
        loop {
            // Check for keyboard events
            if let Ok(event) = receiver.try_recv() {
                match event {
                    KeyboardEvent::CtrlCapsLockPressed => self.start_recording(),
                    KeyboardEvent::CtrlCapsLockReleased => self.stop_and_transcribe(),
                }
            }

            // Process GTK events if the tray-icon feature is enabled
            #[cfg(feature = "tray-icon")]
            {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
            }

            // Sleep to reduce CPU usage
            thread::sleep(Duration::from_millis(10));
        }
    }
}
