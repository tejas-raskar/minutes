//! Audio mixing utilities
//!
//! Provides functions for combining multiple audio streams into one,
//! used by the PipeWire backend to mix system audio and microphone.

/// Audio mixer for combining multiple streams
pub struct AudioMixer {
    /// Target sample rate
    sample_rate: u32,
    /// Microphone boost factor (1.0 = no boost)
    mic_boost: f32,
}

impl AudioMixer {
    /// Create a new audio mixer
    ///
    /// # Arguments
    /// * `sample_rate` - Target sample rate for output
    /// * `mic_boost` - Microphone volume multiplier (e.g., 1.2 for 20% boost)
    pub fn new(sample_rate: u32, mic_boost: f32) -> Self {
        Self {
            sample_rate,
            mic_boost,
        }
    }

    /// Get the target sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Mix two audio buffers together
    ///
    /// Combines system audio and microphone input into a single buffer.
    /// Applies mic boost and prevents clipping.
    ///
    /// # Arguments
    /// * `system` - System audio samples (what you hear)
    /// * `mic` - Microphone samples (what you say)
    ///
    /// # Returns
    /// Mixed audio samples
    pub fn mix(&self, system: &[f32], mic: &[f32]) -> Vec<f32> {
        let len = system.len().max(mic.len());
        let mut output = Vec::with_capacity(len);

        for i in 0..len {
            let sys_sample = system.get(i).copied().unwrap_or(0.0);
            let mic_sample = mic.get(i).copied().unwrap_or(0.0) * self.mic_boost;

            // Simple additive mixing with soft clipping
            let mixed = sys_sample + mic_sample;
            output.push(soft_clip(mixed));
        }

        output
    }

    /// Mix and convert to i16 samples for WAV output
    pub fn mix_to_i16(&self, system: &[f32], mic: &[f32]) -> Vec<i16> {
        self.mix(system, mic)
            .into_iter()
            .map(f32_to_i16)
            .collect()
    }

    /// Convert stereo to mono by averaging channels
    pub fn stereo_to_mono(stereo: &[f32]) -> Vec<f32> {
        stereo
            .chunks(2)
            .map(|chunk| {
                if chunk.len() == 2 {
                    (chunk[0] + chunk[1]) / 2.0
                } else {
                    chunk[0]
                }
            })
            .collect()
    }

    /// Resample audio to target sample rate using linear interpolation
    ///
    /// Note: For production use, consider a proper resampling library.
    /// Linear interpolation is simple but introduces aliasing.
    pub fn resample(&self, samples: &[f32], source_rate: u32) -> Vec<f32> {
        if source_rate == self.sample_rate {
            return samples.to_vec();
        }

        let ratio = source_rate as f64 / self.sample_rate as f64;
        let output_len = ((samples.len() as f64) / ratio).ceil() as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_pos = i as f64 * ratio;
            let src_idx = src_pos.floor() as usize;
            let frac = src_pos.fract() as f32;

            let sample = if src_idx + 1 < samples.len() {
                // Linear interpolation between adjacent samples
                samples[src_idx] * (1.0 - frac) + samples[src_idx + 1] * frac
            } else if src_idx < samples.len() {
                samples[src_idx]
            } else {
                0.0
            };

            output.push(sample);
        }

        output
    }
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new(16000, 1.2) // 16kHz for Whisper, 20% mic boost
    }
}

/// Soft clipping function to prevent harsh distortion
///
/// Uses tanh-like curve that gently compresses values approaching +-1.0
fn soft_clip(sample: f32) -> f32 {
    if sample.abs() <= 0.5 {
        sample
    } else {
        sample.signum() * (0.5 + 0.5 * (2.0 * (sample.abs() - 0.5)).tanh())
    }
}

/// Convert f32 sample (-1.0 to 1.0) to i16
fn f32_to_i16(sample: f32) -> i16 {
    let clamped = sample.clamp(-1.0, 1.0);
    (clamped * 32767.0) as i16
}

/// Convert i16 sample to f32 (-1.0 to 1.0)
#[allow(dead_code)]
pub fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / 32768.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_equal_length() {
        let mixer = AudioMixer::new(16000, 1.0);
        let sys = vec![0.5, 0.3, -0.2];
        let mic = vec![0.2, -0.1, 0.4];
        let result = mixer.mix(&sys, &mic);
        
        assert_eq!(result.len(), 3);
        assert!((result[0] - soft_clip(0.7)).abs() < 0.01);
        assert!((result[1] - 0.2).abs() < 0.01);
        assert!((result[2] - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_mix_different_length() {
        let mixer = AudioMixer::new(16000, 1.0);
        let sys = vec![0.5, 0.3];
        let mic = vec![0.2, -0.1, 0.4, 0.1];
        let result = mixer.mix(&sys, &mic);
        
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_soft_clip() {
        assert!((soft_clip(0.3) - 0.3).abs() < 0.001);
        assert!(soft_clip(1.5) < 1.0);
        assert!(soft_clip(-1.5) > -1.0);
    }

    #[test]
    fn test_stereo_to_mono() {
        let stereo = vec![0.4, 0.6, 0.2, 0.8];
        let mono = AudioMixer::stereo_to_mono(&stereo);
        
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < 0.01);
        assert!((mono[1] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_resample_same_rate() {
        let mixer = AudioMixer::new(16000, 1.0);
        let samples = vec![0.1, 0.2, 0.3];
        let result = mixer.resample(&samples, 16000);
        
        assert_eq!(result, samples);
    }
}
