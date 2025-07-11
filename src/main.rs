use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use device_query::{DeviceQuery, DeviceState, Keycode};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};

// Audio stream implementation for microphone recording
struct AudioStream {
    stream: Option<cpal::Stream>,
    samples: Arc<Mutex<Vec<f32>>>,
    recording: Arc<Mutex<bool>>,
    sample_rate: u32,
    channels: u16,
}

impl AudioStream {
    fn new(samples: Arc<Mutex<Vec<f32>>>, recording: Arc<Mutex<bool>>) -> Result<Self, String> {
        Ok(AudioStream {
            stream: None,
            samples,
            recording,
            sample_rate: 44100, // Default value, will be updated when stream is created
            channels: 1,        // Default value, will be updated when stream is created
        })
    }

    fn play(&mut self) -> Result<(), String> {
        let host = cpal::default_host();

        // Get the default input device
        let device = host.default_input_device()
            .ok_or_else(|| "No input device available".to_string())?;

        println!("Using input device: {}", device.name().map_err(|e| e.to_string())?);

        // Get the default config for the device
        let config = device.default_input_config()
            .map_err(|e| e.to_string())?;

        println!("Default input config: {:?}", config);

        // Store the sample rate and channels for WAV file creation
        self.sample_rate = config.sample_rate().0;
        self.channels = config.channels();

        let samples = self.samples.clone();
        let recording = self.recording.clone();

        // Create a stream for recording
        let err_fn = move |err| {
            eprintln!("an error occurred on the input audio stream: {}", err);
        };

        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if *recording.lock().unwrap() {
                        let mut samples_lock = samples.lock().unwrap();
                        samples_lock.extend_from_slice(data);
                    }
                },
                err_fn,
                None
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if *recording.lock().unwrap() {
                        let mut samples_lock = samples.lock().unwrap();
                        samples_lock.extend(data.iter().map(|&s| s as f32 / 32768.0));
                    }
                },
                err_fn,
                None
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    if *recording.lock().unwrap() {
                        let mut samples_lock = samples.lock().unwrap();
                        samples_lock.extend(data.iter().map(|&s| (s as f32 / 65535.0) * 2.0 - 1.0));
                    }
                },
                err_fn,
                None
            ),
            _ => return Err("Unsupported sample format".to_string()),
        }.map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);

        Ok(())
    }

    fn pause(&mut self) -> Result<(), String> {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        Ok(())
    }

    fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn get_channels(&self) -> u16 {
        self.channels
    }
}

fn main() {
    println!("Voice Input Application");
    println!("Press F1 to start recording, release to save");

    // Initialize device state for keyboard monitoring
    let device_state = DeviceState::new();
    let mut f1_pressed = false;

    // Buffer to store recorded samples
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(Mutex::new(false));

    // Create an audio stream for microphone recording
    let mut stream = AudioStream::new(recorded_samples.clone(), recording.clone())
        .expect("Failed to create audio stream");

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
            } else {
                println!("No audio recorded");
            }
        }

        // Sleep to reduce CPU usage
        thread::sleep(Duration::from_millis(10));
    }
}
