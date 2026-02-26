// src/audio/cpal_backend.rs  —  cpal sine-wave tone generator
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use crate::morse::ToneSeq;
use super::AudioOutput;
use std::sync::{Arc, Mutex};

struct SharedState {
    key_down:  bool,
    frequency: f32,
    volume:    f32,
    phase:     f32,
    sample_rate: f32,
}

pub struct CpalAudio {
    state:  Arc<Mutex<SharedState>>,
    _stream: Stream,
}

// Stream is !Send on some platforms; wrap it
unsafe impl Send for CpalAudio {}

impl CpalAudio {
    pub fn new(hz: f32, volume: f32) -> Result<Self> {
        let host   = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow!("No output device"))?;
        let config = device.default_output_config()?;
        let sr = config.sample_rate().0 as f32;

        let state = Arc::new(Mutex::new(SharedState {
            key_down: false,
            frequency: hz,
            volume,
            phase: 0.0,
            sample_rate: sr,
        }));

        let st = Arc::clone(&state);
        let stream = match config.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), st)?,
            SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), st)?,
            SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), st)?,
            _                 => return Err(anyhow!("Unsupported sample format")),
        };
        stream.play()?;
        Ok(Self { state, _stream: stream })
    }
}

fn build_stream<S>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<SharedState>>,
) -> Result<Stream>
where S: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>
{
    let ch = config.channels as usize;
    let stream = device.build_output_stream(
        config,
        move |data: &mut [S], _: &cpal::OutputCallbackInfo| {
            let mut s = state.lock().unwrap();
            let step = s.frequency / s.sample_rate;
            for frame in data.chunks_mut(ch) {
                let sample = if s.key_down {
                    // Sine with soft envelope (immediate for CW feel)
                    let v = (s.phase * 2.0 * std::f32::consts::PI).sin() * s.volume;
                    s.phase = (s.phase + step) % 1.0;
                    v
                } else {
                    s.phase = 0.0;
                    0.0
                };
                let out = S::from_sample(sample);
                for smp in frame.iter_mut() { *smp = out; }
            }
        },
        |e| log::error!("Audio error: {e}"),
        None,
    )?;
    Ok(stream)
}

impl AudioOutput for CpalAudio {
    fn play_sequence(&mut self, seq: &ToneSeq) -> Result<()> {
        for &(on, dur) in seq {
            {
                let mut s = self.state.lock().unwrap();
                s.key_down = on;
            }
            std::thread::sleep(dur);
        }
        // Ensure key is off at end
        self.state.lock().unwrap().key_down = false;
        Ok(())
    }

    fn tone_on(&mut self) -> Result<()> {
        self.state.lock().unwrap().key_down = true;
        Ok(())
    }
    fn tone_off(&mut self) -> Result<()> {
        self.state.lock().unwrap().key_down = false;
        Ok(())
    }
    fn set_frequency(&mut self, hz: f32) {
        self.state.lock().unwrap().frequency = hz;
    }
    fn set_volume(&mut self, vol: f32) {
        self.state.lock().unwrap().volume = vol;
    }
}
