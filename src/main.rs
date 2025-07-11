use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use device_query::{DeviceQuery, DeviceState, Keycode};
use chrono::Local;
use std::fs::File;
use std::io::{BufWriter, Write};

// Dummy audio stream implementation
struct DummyStream;

impl DummyStream {
    fn new() -> Self {
        DummyStream
    }

    fn play(&self) -> Result<(), String> {
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        Ok(())
    }
}

fn main() {
    println!("Voice Input Application");
    println!("Press F1 to start recording, release to save");

    // Initialize device state for keyboard monitoring
    let device_state = DeviceState::new();
    let mut f1_pressed = false;

    // Buffer to store recorded samples (dummy data for demonstration)
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    // Create a dummy audio stream
    let stream = DummyStream::new();

    println!("Waiting for F1 key...");

    // Main loop to monitor keyboard events
    loop {
        let keys = device_state.get_keys();
        let is_f1_pressed = keys.contains(&Keycode::F12);

        // F1 key was just pressed
        if is_f1_pressed && !f1_pressed {
            println!("F1 pressed - Recording started");
            f1_pressed = true;

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

        // F1 key was just released
        if !is_f1_pressed && f1_pressed {
            println!("F1 released - Recording stopped");
            f1_pressed = false;

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

                // Create a dummy file instead of a WAV file
                let file = File::create(&filename).expect("Failed to create file");
                let mut writer = BufWriter::new(file);

                // Write a simple text representation of the samples
                writeln!(writer, "Dummy audio recording with {} samples", samples.len())
                    .expect("Failed to write to file");

                for (i, sample) in samples.iter().enumerate().take(10) {
                    writeln!(writer, "Sample {}: {}", i, sample)
                        .expect("Failed to write sample");
                }

                writeln!(writer, "... (more samples)")
                    .expect("Failed to write to file");

                writer.flush().expect("Failed to flush writer");
                println!("Recording saved successfully (dummy file)");
            } else {
                println!("No audio recorded");
            }
        }

        // Sleep to reduce CPU usage
        thread::sleep(Duration::from_millis(10));
    }
}
