use std::time::{Duration, Instant};
use std::thread;
use enigo::{Enigo, KeyboardControllable};
use crate::{KEY_RELEASE_TIME, TIMING_INFO};

// Function to simulate typing text at the current cursor position
pub fn simulate_typing(text: &str) {
    println!("Simulating typing: {}", text);

    // Start timing for typing simulation
    let typing_start = Instant::now();

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

    // Record time for typing simulation
    TIMING_INFO.lock().unwrap().insert("typing".to_string(), typing_start.elapsed());

    // Calculate and log the time between key release and end of simulated typing
    if let Some(start_time) = *KEY_RELEASE_TIME.lock().unwrap() {
        let elapsed = start_time.elapsed();
        println!("Time between key release and end of simulated typing: {:.2} seconds", elapsed.as_secs_f64());

        // Display breakdown of time spent in each stage
        println!("Time breakdown:");

        let timing_info = TIMING_INFO.lock().unwrap();

        // Print each stage's timing
        if let Some(time) = timing_info.get("stop_recording") {
            println!("  - Stopping recording: {:.2} seconds", time.as_secs_f64());
        }

        if let Some(time) = timing_info.get("save_wav") {
            println!("  - Saving WAV file: {:.2} seconds", time.as_secs_f64());
        }

        if let Some(time) = timing_info.get("audio_conversion") {
            println!("  - Converting audio (to mono and 16000Hz): {:.2} seconds", time.as_secs_f64());
        }

        if let Some(time) = timing_info.get("actual_transcription") {
            println!("  - Actual transcribing: {:.2} seconds", time.as_secs_f64());
        }

        if let Some(time) = timing_info.get("typing") {
            println!("  - Simulating typing: {:.2} seconds", time.as_secs_f64());
        }
    }
}