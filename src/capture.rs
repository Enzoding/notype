use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;

use crate::audio;
use crate::tasks::{AudioWriter, CaptureResult, Task, TaskConfig};

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

#[allow(dead_code)]
pub fn default_device_name() -> String {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "系统默认麦克风".into())
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut out = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for dev in devices {
            if let Ok(name) = dev.name() {
                if !name.trim().is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
    out
}

pub struct ActiveRecording {
    stream: Stream,
    writer: Option<AudioWriter>,
    task: Option<Task>,
    error: Arc<Mutex<Option<String>>>,
    last_data: Arc<Mutex<Instant>>,
    started: Instant,
}

impl ActiveRecording {
    pub fn start_with_device(preferred: Option<&str>) -> Result<Self, String> {
        Self::start(preferred, None)
    }

    pub fn start_saved(preferred: Option<&str>, config: TaskConfig) -> Result<Self, String> {
        Self::start(preferred, Some(config))
    }

    fn start(preferred: Option<&str>, task_config: Option<TaskConfig>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = preferred
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .and_then(|name| {
                host.input_devices()
                    .ok()?
                    .find(|dev| dev.name().map(|n| n.trim() == name).unwrap_or(false))
            })
            .or_else(|| host.default_input_device())
            .ok_or_else(|| "没有找到麦克风".to_string())?;

        let config = device
            .default_input_config()
            .map_err(|err| format!("无法打开麦克风：{err}"))?;
        let channels = config.channels();
        let sample_rate = config.sample_rate().0;
        let task = task_config
            .map(|cfg| Task::new(channels, sample_rate, cfg))
            .transpose()?;
        let writer = task.as_ref().map(AudioWriter::start).transpose()?;
        let error = writer
            .as_ref()
            .map(|w| w.error.clone())
            .unwrap_or_else(|| Arc::new(Mutex::new(None)));
        let last_data = Arc::new(Mutex::new(Instant::now()));
        let sender = writer.as_ref().map(AudioWriter::sender);
        let sample_format = config.sample_format();
        let config = config.config();
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build::<f32>(
                &device,
                &config,
                sender,
                error.clone(),
                last_data.clone(),
                |v| v,
            )?,
            cpal::SampleFormat::I16 => build::<i16>(
                &device,
                &config,
                sender,
                error.clone(),
                last_data.clone(),
                |v| v as f32 / i16::MAX as f32,
            )?,
            cpal::SampleFormat::U16 => build::<u16>(
                &device,
                &config,
                sender,
                error.clone(),
                last_data.clone(),
                |v| (v as f32 / u16::MAX as f32) * 2.0 - 1.0,
            )?,
            other => return Err(format!("不支持的麦克风格式：{other:?}")),
        };
        stream
            .play()
            .map_err(|err| format!("麦克风没有开始录音：{err}"))?;
        Ok(Self {
            stream,
            writer,
            task,
            error,
            last_data,
            started: Instant::now(),
        })
    }

    pub fn finish(self) -> Option<CaptureResult> {
        drop(self.stream);
        LEVEL.store(0, Ordering::Relaxed);
        if let Ok(mut bars) = WAVE.lock() {
            *bars = [0.0; WAVE_BARS];
        }
        self.writer
            .zip(self.task)
            .map(|(writer, task)| writer.finish(task))
    }

    pub fn task_id(&self) -> Option<String> {
        self.task.as_ref().map(|t| t.id.clone())
    }
    pub fn error(&self) -> Option<String> {
        if let Ok(error) = self.error.lock() {
            if error.is_some() {
                return error.clone();
            }
        }
        if self
            .last_data
            .lock()
            .is_ok_and(|last| last.elapsed().as_secs() >= 5)
        {
            return Some("麦克风已停止提供音频，已保存的部分可以恢复".into());
        }
        None
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sender: Option<std::sync::mpsc::SyncSender<Vec<f32>>>,
    error: Arc<Mutex<Option<String>>>,
    last_data: Arc<Mutex<Instant>>,
    convert: fn(T) -> f32,
) -> Result<Stream, String>
where
    T: cpal::SizedSample + Send + 'static,
{
    let stream_error = error.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let converted: Vec<f32> = data.iter().copied().map(convert).collect();
                LEVEL.store(audio::rms(&converted).to_bits(), Ordering::Relaxed);
                push_wave(&converted);
                if let Ok(mut last) = last_data.lock() {
                    *last = Instant::now();
                }
                if let Some(sender) = &sender {
                    if sender.try_send(converted).is_err() {
                        if let Ok(mut e) = error.lock() {
                            *e = Some("录音缓存写入跟不上，已停止录音；可恢复已保存内容".into());
                        }
                    }
                }
            },
            move |_err| {
                if let Ok(mut e) = stream_error.lock() {
                    *e = Some("麦克风中断，已保存的部分可以恢复".into());
                }
            },
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
            // Fast attack, snappy release: tracks speech syllables and pauses
            // so waves bounce dynamically rather than lingering.
            let coefficient = if sample > *bar { 0.75 } else { 0.22 };
            *bar += (sample - *bar) * coefficient;
        }
    }
}
