use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

// Audio stream implementation for microphone recording
pub struct AudioStream {
    stream: Option<cpal::Stream>,
    samples: Arc<Mutex<Vec<f32>>>,
    recording: Arc<Mutex<bool>>,
    sample_rate: u32,
    channels: u16,
}

impl AudioStream {
    pub fn new(samples: Arc<Mutex<Vec<f32>>>, recording: Arc<Mutex<bool>>) -> Result<Self, String> {
        Ok(AudioStream {
            stream: None,
            samples,
            recording,
            sample_rate: 44100, // Default value, will be updated when stream is created
            channels: 1,        // Default value, will be updated when stream is created
        })
    }

    pub fn play(&mut self) -> Result<(), String> {
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

    pub fn pause(&mut self) -> Result<(), String> {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        Ok(())
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn get_channels(&self) -> u16 {
        self.channels
    }
}