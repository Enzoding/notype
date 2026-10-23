const TARGET_RATE: u32 = 16_000;

pub struct Recorded {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl Recorded {
    pub fn duration_secs(&self) -> f32 {
        let frames = self.samples.len() as f32 / self.channels.max(1) as f32;
        if self.sample_rate == 0 {
            0.0
        } else {
            frames / self.sample_rate as f32
        }
    }

    pub fn to_wav(&self) -> Vec<u8> {
        let pcm = resample_mono_i16(&self.samples, self.channels, self.sample_rate, TARGET_RATE);
        encode_wav(&pcm, TARGET_RATE)
    }
}

pub fn resample_mono_i16(samples: &[f32], channels: u16, from_rate: u32, to_rate: u32) -> Vec<u8> {
    let channels = channels.max(1) as usize;
    let frames = samples.len() / channels;
    if frames == 0 || from_rate == 0 || to_rate == 0 {
        return Vec::new();
    }
    let mono: Vec<f32> = (0..frames)
        .map(|frame| {
            let start = frame * channels;
            let sum: f32 = samples[start..start + channels].iter().sum();
            sum / channels as f32
        })
        .collect();
    let out_len = ((mono.len() as f64) * to_rate as f64 / from_rate as f64).round() as usize;
    let mut pcm = Vec::with_capacity(out_len * 2);
    if out_len == 0 {
        return pcm;
    }
    let ratio = from_rate as f64 / to_rate as f64;
    for index in 0..out_len {
        let src = index as f64 * ratio;
        let left = src.floor() as usize;
        let frac = (src - left as f64) as f32;
        let a = mono.get(left).copied().unwrap_or(0.0);
        let b = mono.get(left + 1).copied().unwrap_or(a);
        let sample = (a + (b - a) * frac).clamp(-1.0, 1.0);
        let value = (sample * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }
    pcm
}

pub fn encode_wav(pcm_le: &[u8], sample_rate: u32) -> Vec<u8> {
    let data_len = pcm_le.len() as u32;
    let mut out = Vec::with_capacity(44 + pcm_le.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm_le);
    out
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    energy.sqrt().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_mono_16k() {
        let wav = encode_wav(&[0, 0, 1, 0], 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert_eq!(wav.len(), 48);
    }

    #[test]
    fn resample_changes_length_with_rate() {
        let samples = vec![0.0; 48_000];
        let pcm = resample_mono_i16(&samples, 1, 48_000, 16_000);
        assert_eq!(pcm.len(), 16_000 * 2);
    }
}
