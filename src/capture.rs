use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;

use crate::audio::{self, Recorded};

/// Ten samples, mirrored across the centre when drawn, so the HUD shows a
/// symmetric waveform rather than a bar chart.
pub const WAVE_BARS: usize = 10;

static LEVEL: AtomicU32 = AtomicU32::new(0);
static WAVE: Mutex<[f32; WAVE_BARS]> = Mutex::new([0.0; WAVE_BARS]);

pub fn level() -> f32 {
    f32::from_bits(LEVEL.load(Ordering::Relaxed)).clamp(0.0, 1.0)
}

pub fn wave() -> [f32; WAVE_BARS] {
    WAVE.lock().map(|bars| *bars).unwrap_or([0.0; WAVE_BARS])
}

pub struct ActiveRecording {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    sample_rate: u32,
    started: Instant,
}

impl ActiveRecording {
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "没有找到麦克风".to_string())?;
        let config = device
            .default_input_config()
            .map_err(|err| format!("无法打开麦克风：{err}"))?;
        let channels = config.channels();
        let sample_rate = config.sample_rate().0;
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let writer = Arc::clone(&samples);
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config.into(), writer, |value| value)?,
            cpal::SampleFormat::I16 => {
                build::<i16>(&device, &config.into(), writer, |value| value as f32 / i16::MAX as f32)?
            }
            cpal::SampleFormat::U16 => build::<u16>(&device, &config.into(), writer, |value| {
                (value as f32 / u16::MAX as f32) * 2.0 - 1.0
            })?,
            other => return Err(format!("不支持的麦克风格式：{other:?}")),
        };
        stream
            .play()
            .map_err(|err| format!("麦克风没有开始录音：{err}"))?;
        Ok(Self {
            stream,
            samples,
            channels,
            sample_rate,
            started: Instant::now(),
        })
    }

    pub fn finish(self) -> Recorded {
        drop(self.stream);
        LEVEL.store(0, Ordering::Relaxed);
        if let Ok(mut bars) = WAVE.lock() {
            *bars = [0.0; WAVE_BARS];
        }
        let samples = self.samples.lock().map(|guard| guard.clone()).unwrap_or_default();
        Recorded {
            samples,
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    convert: fn(T) -> f32,
) -> Result<Stream, String>
where
    T: cpal::SizedSample + Send + 'static,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let converted: Vec<f32> = data.iter().copied().map(convert).collect();
                LEVEL.store(audio::rms(&converted).to_bits(), Ordering::Relaxed);
                push_wave(&converted);
                if let Ok(mut buffer) = samples.lock() {
                    buffer.extend(converted);
                }
            },
            |err| eprintln!("麦克风中断：{err}"),
            None,
        )
        .map_err(|err| format!("无法建立录音：{err}"))
}

fn push_wave(samples: &[f32]) {
    let mut next = [0.0; WAVE_BARS];
    if !samples.is_empty() {
        for (index, bar) in next.iter_mut().enumerate() {
            let start = index * samples.len() / WAVE_BARS;
            let end = ((index + 1) * samples.len() / WAVE_BARS)
                .max(start + 1)
                .min(samples.len());
            *bar = samples[start..end]
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
                .clamp(0.0, 1.0);
        }
    }
    if let Ok(mut bars) = WAVE.lock() {
        for (bar, sample) in bars.iter_mut().zip(next) {
            // Fast attack, slow release: the envelope follows speech without
            // flickering on every audio buffer.
            let coefficient = if sample > *bar { 0.55 } else { 0.10 };
            *bar += (sample - *bar) * coefficient;
        }
    }
}
