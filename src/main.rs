use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use device_query::{DeviceQuery, DeviceState, Keycode};
use chrono::Local;
use hound::{WavSpec, WavWriter};

#[cfg(feature = "tray-icon")]
use gtk::prelude::*;

mod tray_icon;
mod audio_stream;
mod whisper;
use audio_stream::AudioStream;
use whisper::WhisperTranscriber;

fn main() {
    println!("Voice Input Application");
    println!("Press F12 to start recording, release to save");

    // Initialize the system tray icon if the feature is enabled
    if let Err(e) = tray_icon::init_tray_icon() {
        eprintln!("Failed to initialize tray icon: {}", e);
    }

    // Initialize the WhisperTranscriber
    // The model will be downloaded automatically if it doesn't exist
    // Models are downloaded from: https://huggingface.co/ggerganov/whisper.cpp
    let model_path = "ggml-base.en.bin"; // Change this to use a different model
    let transcriber = match WhisperTranscriber::new(model_path) {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("Failed to initialize WhisperTranscriber: {}", e);
            eprintln!("Audio transcription will be disabled");
            None
        }
    };

    // Initialize device state for keyboard monitoring
    let device_state = DeviceState::new();
    let mut f12_pressed = false;

    // Buffer to store recorded samples
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    // Create an audio stream for microphone recording
    let mut stream = AudioStream::new(recorded_samples.clone(), recording.clone())
        .expect("Failed to create audio stream");

    println!("Waiting for F12 key...");

    // Main loop to monitor keyboard events
    loop {
        let keys = device_state.get_keys();
        let is_f12_pressed = keys.contains(&Keycode::F12);

        // F12 key was just pressed
        if is_f12_pressed && !f12_pressed {
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

        // F12 key was just released
        if !is_f12_pressed && f12_pressed {
            println!("F12 released - Recording stopped");
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
                    match t.transcribe_audio(&filename) {
                        Ok(transcript) => {
                            println!("Transcription successful");
                            println!("Transcript preview: {}", 
                                     transcript.lines().take(2).collect::<Vec<_>>().join(" "));
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
