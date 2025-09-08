use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use std::collections::HashMap;

use crate::audio_stream::AudioStream;
use crate::clipboard_inserter;
use crate::hotkeys::KeyboardEvent;
use crate::keyboard_layout::KeyboardLayoutDetector;
use crate::transcriber_utils::{ensure_transcriber_for, select_model_file, transcribe_samples_with, translate_samples_with};
use crate::whisper::WhisperTranscriber;
use crate::config;

use crate::tray_ui::{ModelProgress, UiIntent};
#[cfg(feature = "tray-icon")]
use crate::tray_ui::{tray_post_view, AppView, TrayStatus};

static PRIMING: AtomicBool = AtomicBool::new(false);

// Public, app-wide status for logic/UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppStatus {
    Priming,
    Ready,
    Recording,
    Processing,
}

impl AppStatus {
    #[cfg(feature = "tray-icon")]
    fn to_tray(self) -> TrayStatus {
        match self {
            AppStatus::Priming => TrayStatus::Priming,
            AppStatus::Ready => TrayStatus::Ready,
            AppStatus::Recording => TrayStatus::Recording,
            AppStatus::Processing => TrayStatus::Processing,
        }
    }
}

// Centralized app state for the event loop
struct AppState {
    status: AppStatus,
    current_language: String,
    active_model: String,
    loading: HashMap<String, ModelProgress>,
    english_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
    multilingual_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
    recorded_samples: Arc<Mutex<Vec<f32>>>,
    stream: AudioStream,
    translate_enabled: bool,
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
        english_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
        multilingual_transcriber: Arc<Mutex<Option<WhisperTranscriber>>>,
        initial_model: String,
    ) -> Self {
        Self {
            state: AppState {
                status: AppStatus::Ready, // will be adjusted below
                current_language: String::from("en"),
                active_model: initial_model.clone(),
                loading: HashMap::new(),
                english_transcriber,
                multilingual_transcriber,
                recorded_samples,
                stream,
                translate_enabled: config::get_translate_enabled(),
            },
        }
        .with_startup_status()
    }

    // Adjust initial status to Priming if the selected model (both en/multi) is missing
    fn with_startup_status(mut self) -> Self {
        let (en_file, multi_file) = get_both_model_filenames(&self.state.active_model);
        let need_en = if self.state.active_model != "large" {
            config::get_model_path(&en_file).is_none()
        } else { false };
        let need_multi = config::get_model_path(&multi_file).is_none();
        let is_priming = need_en || need_multi;
        if is_priming {
            self.state.status = AppStatus::Priming;
            PRIMING.store(true, Ordering::SeqCst);
        } else {
            self.state.status = AppStatus::Ready;
            PRIMING.store(false, Ordering::SeqCst);
        }
        self
    }

    #[cfg(feature = "tray-icon")]
    fn post_view(&self) {
        let view = AppView {
            active_model: self.state.active_model.clone(),
            status: self.state.status.to_tray(),
            loading: self.state.loading.clone(),
            translate_enabled: self.state.translate_enabled,
        };
        tray_post_view(view);
    }

    fn start_recording(&mut self) {
        // Guard with status (single-source-of-truth for app logic/UI)
        if self.state.status != AppStatus::Ready {
            return;
        }

        println!("Ctrl+CAPSLOCK pressed - Recording started");
        self.state.status = AppStatus::Recording;
        #[cfg(feature = "tray-icon")]
        self.post_view();

        // Detect and store language code
        let language_code = detect_language_code();
        println!("Detected language code: {}", language_code);
        self.state.current_language = language_code.clone();

        // Clear previous recording
        {
            let mut samples = self.state.recorded_samples.lock().unwrap();
            samples.clear();
        }

        // Start audio stream + enable capture
        self.state.stream.play().expect("Failed to start the stream");
        self.state.stream.start_capture();

        // Initialize Whisper after starting recording
        let is_english = language_code.starts_with("en");

        // Resolve the model file based on selected model and language
        let model_file = select_model_file(&self.state.active_model, is_english);

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
        if self.state.status != AppStatus::Recording {
            println!("No audio recorded");
            self.state.status = AppStatus::Ready;
            #[cfg(feature = "tray-icon")]
            self.post_view();
            return;
        }

        println!("Ctrl+CAPSLOCK released - Recording stopped, transcribing and inserting at cursor position");

        // Stop capture immediately, then pause stream
        self.state.stream.stop_capture();
        self.state.stream.pause().expect("Failed to pause the stream");

        // Update status: processing/transcribing (tray will be blue)
        self.state.status = AppStatus::Processing;
        #[cfg(feature = "tray-icon")]
        self.post_view();

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

            let result = if self.state.translate_enabled {
                translate_samples_with(
                    transcriber,
                    &samples,
                    self.state.stream.get_sample_rate(),
                    self.state.stream.get_channels(),
                    &self.state.current_language,
                )
            } else {
                transcribe_samples_with(
                    transcriber,
                    &samples,
                    self.state.stream.get_sample_rate(),
                    self.state.stream.get_channels(),
                    &self.state.current_language,
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

        // Back to ready
        self.state.status = AppStatus::Ready;
        #[cfg(feature = "tray-icon")]
        self.post_view();
    }

    pub fn run_loop(&mut self, kb_receiver: Receiver<KeyboardEvent>, ui_receiver: Receiver<UiIntent>) -> ! {
        // Kick off initial ensure if we are priming
        if PRIMING.load(Ordering::SeqCst) {
            let model = self.state.active_model.clone();
            self.ensure_model_async(model);
        }
        #[cfg(feature = "tray-icon")]
        self.post_view();

        loop {
            // Handle UI intents (model selection, quit)
            if let Ok(intent) = ui_receiver.try_recv() {
                match intent {
                    UiIntent::SelectModel(model) => {
                        if self.state.active_model != model {
                            // Persist selection
                            if let Err(e) = config::save_selected_model(&model) {
                                eprintln!("Failed to save selected model to config file: {}", e);
                            } else {
                                println!("Saved selected model '{}' to config file", model);
                            }

                            self.state.active_model = model.clone();
                            #[cfg(feature = "tray-icon")]
                            self.post_view();

                            // Ensure model is available (downloads if needed) and update progress map
                            self.ensure_model_async(model);
                        }
                    }
                    UiIntent::ToggleTranslate(enabled) => {
                        if self.state.translate_enabled != enabled {
                            self.state.translate_enabled = enabled;
                            if let Err(e) = config::save_translate_enabled(enabled) {
                                eprintln!("Failed to save translate setting: {}", e);
                            } else {
                                println!("Translate setting set to {} and saved", enabled);
                            }
                            #[cfg(feature = "tray-icon")]
                            self.post_view();
                        }
                    }
                    UiIntent::QuitRequested => {
                        // Exit process (clean up if needed)
                        std::process::exit(0);
                    }
                }
            }

            // Check for keyboard events
            if let Ok(event) = kb_receiver.try_recv() {
                match event {
                    KeyboardEvent::CtrlCapsLockPressed => self.start_recording(),
                    KeyboardEvent::CtrlCapsLockReleased => self.stop_and_transcribe(),
                    KeyboardEvent::AltCapsToggleTranslate => {
                        let new_val = !self.state.translate_enabled;
                        self.state.translate_enabled = new_val;
                        if let Err(e) = config::save_translate_enabled(new_val) {
                            eprintln!("Failed to save translate setting: {}", e);
                        } else {
                            println!("Translate setting toggled to {} via Alt+Caps", new_val);
                        }
                        #[cfg(feature = "tray-icon")]
                        self.post_view();
                    }
                }
            }

            // Sleep to reduce CPU usage
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn ensure_model_async(&mut self, model: String) {
        // quick existence check
        let (en_model_file, multi_model_file) = get_both_model_filenames(&model);
        let en_exists = if model != "large" {
            config::get_model_path(&en_model_file).is_some()
        } else { true };
        let multi_exists = config::get_model_path(&multi_model_file).is_some();

        if en_exists && multi_exists {
            return;
        }

        // Mark as loading at 0%
        #[cfg(feature = "tray-icon")]
        {
            self.state.loading.insert(model.clone(), ModelProgress { percent: 0, eta_secs: 0 });
            self.post_view();
        }

        // Spawn download worker
        let ui_model = model.clone();
        thread::spawn(move || {
            // forward progress to app thread via global callback by updating config or external event mechanism
            // Here, we reuse whisper's callback; the app thread will not receive direct calls,
            // so we emit updates via a simple side-channel approach: we cannot mutate self here,
            // thus we only drive downloads; the app thread will refresh progress via callback closures set below.
            let _ = ui_model; // kept to satisfy move
        });

        // Register progress callback that updates our local state and posts snapshots
        let last_p = Arc::new(AtomicU8::new(255)); // force first emit
        let model_for_cb = model.clone();
        let last_p_cb = last_p.clone();
        WhisperTranscriber::set_download_progress_callback(Some(Box::new({
            let model_for_cb_clone = model_for_cb.clone();
            move |percent, eta_secs| {
                let p = percent.clamp(0.0, 100.0) as u8;
                let prev = last_p_cb.swap(p, Ordering::SeqCst);
                if p == prev {
                    return;
                }
                #[cfg(feature = "tray-icon")]
                {
                    // This closure runs on a background thread; only send snapshots to the UI.
                    let mut loading = HashMap::new();
                    loading.insert(model_for_cb_clone.clone(), ModelProgress { percent: p, eta_secs });
                    let view = AppView {
                        active_model: model_for_cb_clone.clone(),
                        status: if PRIMING.load(Ordering::SeqCst) { TrayStatus::Priming } else { TrayStatus::Ready },
                        loading,
                        translate_enabled: config::get_translate_enabled(),
                    };
                    tray_post_view(view);
                }
            }
        })));

        // Perform the downloads synchronously on a worker thread so the app loop remains responsive
        let need_en = !en_exists && model != "large";
        let need_multi = !multi_exists;
        let model_for_done = model.clone();
        thread::spawn(move || {
            if need_en {
                let _ = WhisperTranscriber::download_model(&en_model_file);
            }
            if need_multi {
                let _ = WhisperTranscriber::download_model(&multi_model_file);
            }
            WhisperTranscriber::set_download_progress_callback(None);
            PRIMING.store(false, Ordering::SeqCst);
            #[cfg(feature = "tray-icon")]
            {
                // Clear progress for the model
                let view = AppView {
                    active_model: model_for_done.clone(),
                    status: TrayStatus::Ready,
                    loading: HashMap::new(),
                    translate_enabled: config::get_translate_enabled(),
                };
                tray_post_view(view);
            }
        });
    }
}

fn get_both_model_filenames(model: &str) -> (String, String) {
    match model {
        "base" | "tiny" | "small" | "medium" => (
            format!("ggml-{}.en.bin", model),
            format!("ggml-{}.bin", model),
        ),
        "large" => (
            format!("ggml-{}-v2.bin", model),
            format!("ggml-{}-v2.bin", model),
        ),
        _ => ("ggml-base.en.bin".into(), "ggml-base.bin".into()),
    }
}
