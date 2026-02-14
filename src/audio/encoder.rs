//! OGG Opus audio encoder
//!
//! Compresses WAV files to OGG Opus format for efficient storage.
//! Speech audio compresses from ~115MB/hour (WAV) to ~7MB/hour (OGG Opus).

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// OGG Opus encoder for compressing audio files
#[allow(dead_code)]
pub struct OggEncoder {
    /// Sample rate (must match source)
    sample_rate: u32,
    /// Number of channels
    channels: u8,
    /// Bitrate in bits per second
    bitrate: u32,
}

impl OggEncoder {
    /// Create a new OGG encoder
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz
    /// * `channels` - Number of audio channels (1 = mono, 2 = stereo)
    /// * `bitrate` - Target bitrate in bps (24000 is good for speech)
    pub fn new(sample_rate: u32, channels: u8, bitrate: u32) -> Self {
        Self {
            sample_rate,
            channels,
            bitrate,
        }
    }

    /// Create encoder with defaults for speech (16kHz mono, 24kbps)
    pub fn for_speech() -> Self {
        Self::new(16000, 1, 24000)
    }

    /// Get the bitrate
    pub fn bitrate(&self) -> u32 {
        self.bitrate
    }

    /// Encode a WAV file to OGG Opus
    ///
    /// # Arguments
    /// * `wav_path` - Path to input WAV file
    /// * `ogg_path` - Path for output OGG file
    ///
    /// # Returns
    /// Ok(()) on success, error on failure
    pub fn encode(&self, wav_path: &Path, ogg_path: &Path) -> Result<()> {
        use hound::WavReader;

        tracing::info!(
            "Encoding {} to OGG Opus ({}kbps)",
            wav_path.display(),
            self.bitrate / 1000
        );

        // Read WAV file
        let reader = WavReader::open(wav_path)
            .with_context(|| format!("Failed to open WAV file: {}", wav_path.display()))?;

        let spec = reader.spec();
        tracing::debug!(
            "WAV spec: {} Hz, {} channels, {} bits",
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample
        );

        // Collect samples
        let samples: Vec<i16> = match spec.sample_format {
            hound::SampleFormat::Int => {
                if spec.bits_per_sample == 16 {
                    reader.into_samples::<i16>().filter_map(Result::ok).collect()
                } else if spec.bits_per_sample == 32 {
                    reader
                        .into_samples::<i32>()
                        .filter_map(Result::ok)
                        .map(|s| (s >> 16) as i16)
                        .collect()
                } else {
                    anyhow::bail!("Unsupported bit depth: {}", spec.bits_per_sample);
                }
            }
            hound::SampleFormat::Float => reader
                .into_samples::<f32>()
                .filter_map(Result::ok)
                .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect(),
        };

        if samples.is_empty() {
            anyhow::bail!("WAV file contains no samples");
        }

        // Create Opus encoder
        let mut encoder = opus::Encoder::new(
            spec.sample_rate,
            match spec.channels {
                1 => opus::Channels::Mono,
                2 => opus::Channels::Stereo,
                n => anyhow::bail!("Unsupported channel count: {}", n),
            },
            opus::Application::Voip, // Optimized for speech
        )
        .context("Failed to create Opus encoder")?;

        encoder
            .set_bitrate(opus::Bitrate::Bits(self.bitrate as i32))
            .context("Failed to set bitrate")?;

        // Create OGG stream
        let mut ogg_file = BufWriter::new(
            File::create(ogg_path)
                .with_context(|| format!("Failed to create OGG file: {}", ogg_path.display()))?,
        );

        // Write Opus header and encode
        let serial = rand_serial();
        let mut packet_no = 0u64;
        let mut granule_pos = 0u64;

        // Write Opus ID header
        let id_header = create_opus_id_header(spec.channels as u8, spec.sample_rate);
        write_ogg_page(&mut ogg_file, serial, 0, 2, packet_no, &id_header)?;
        packet_no += 1;

        // Write Opus comment header
        let comment_header = create_opus_comment_header();
        write_ogg_page(&mut ogg_file, serial, 0, 0, packet_no, &comment_header)?;
        packet_no += 1;

        // Encode audio in frames
        // Opus typically uses 20ms frames = sample_rate * 0.02
        let frame_size = (spec.sample_rate as usize) / 50; // 20ms
        let channels = spec.channels as usize;
        let samples_per_frame = frame_size * channels;

        let mut encoded_buf = vec![0u8; 4000]; // Max Opus packet size

        for chunk in samples.chunks(samples_per_frame) {
            // Pad last chunk if needed
            let mut frame = chunk.to_vec();
            if frame.len() < samples_per_frame {
                frame.resize(samples_per_frame, 0);
            }

            // Encode frame
            let encoded_len = encoder
                .encode(&frame, &mut encoded_buf)
                .context("Opus encoding failed")?;

            if encoded_len > 0 {
                granule_pos += frame_size as u64;

                // Determine if this is the last page
                let header_type = if chunk.len() < samples_per_frame { 4 } else { 0 };

                write_ogg_page(
                    &mut ogg_file,
                    serial,
                    granule_pos,
                    header_type,
                    packet_no,
                    &encoded_buf[..encoded_len],
                )?;
                packet_no += 1;
            }
        }

        ogg_file.flush()?;

        let wav_size = std::fs::metadata(wav_path)?.len();
        let ogg_size = std::fs::metadata(ogg_path)?.len();
        let ratio = wav_size as f64 / ogg_size as f64;

        tracing::info!(
            "Encoded to OGG: {} -> {} ({:.1}x compression)",
            format_size(wav_size),
            format_size(ogg_size),
            ratio
        );

        Ok(())
    }

    /// Encode WAV to OGG and delete the original WAV file
    ///
    /// # Arguments
    /// * `wav_path` - Path to input WAV file
    ///
    /// # Returns
    /// Path to the created OGG file
    pub fn encode_and_cleanup(&self, wav_path: &Path) -> Result<PathBuf> {
        let ogg_path = wav_path.with_extension("ogg");

        // Encode
        self.encode(wav_path, &ogg_path)?;

        // Verify OGG file exists and has content
        let ogg_meta = std::fs::metadata(&ogg_path)
            .with_context(|| format!("OGG file not found after encoding: {}", ogg_path.display()))?;

        if ogg_meta.len() == 0 {
            anyhow::bail!("OGG file is empty after encoding");
        }

        // Delete original WAV
        std::fs::remove_file(wav_path)
            .with_context(|| format!("Failed to delete WAV file: {}", wav_path.display()))?;

        tracing::info!("Deleted original WAV file: {}", wav_path.display());

        Ok(ogg_path)
    }
}

impl Default for OggEncoder {
    fn default() -> Self {
        Self::for_speech()
    }
}

/// Create Opus ID header packet
fn create_opus_id_header(channels: u8, sample_rate: u32) -> Vec<u8> {
    let mut header = Vec::with_capacity(19);

    // Magic signature
    header.extend_from_slice(b"OpusHead");

    // Version (1)
    header.push(1);

    // Channel count
    header.push(channels);

    // Pre-skip (samples to skip at start, typically 312 for encoder delay)
    header.extend_from_slice(&312u16.to_le_bytes());

    // Input sample rate (informational)
    header.extend_from_slice(&sample_rate.to_le_bytes());

    // Output gain (0 dB)
    header.extend_from_slice(&0u16.to_le_bytes());

    // Channel mapping family (0 = mono/stereo)
    header.push(0);

    header
}

/// Create Opus comment header packet
fn create_opus_comment_header() -> Vec<u8> {
    let mut header = Vec::with_capacity(60);

    // Magic signature
    header.extend_from_slice(b"OpusTags");

    // Vendor string
    let vendor = b"minutes";
    header.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    header.extend_from_slice(vendor);

    // User comment list (empty)
    header.extend_from_slice(&0u32.to_le_bytes());

    header
}

/// Write an OGG page
fn write_ogg_page<W: Write>(
    writer: &mut W,
    serial: u32,
    granule_pos: u64,
    header_type: u8,
    page_no: u64,
    data: &[u8],
) -> Result<()> {
    // OGG page header
    let mut header = Vec::with_capacity(27 + 255);

    // Capture pattern
    header.extend_from_slice(b"OggS");

    // Version (0)
    header.push(0);

    // Header type
    header.push(header_type);

    // Granule position
    header.extend_from_slice(&granule_pos.to_le_bytes());

    // Serial number
    header.extend_from_slice(&serial.to_le_bytes());

    // Page sequence number
    header.extend_from_slice(&(page_no as u32).to_le_bytes());

    // CRC placeholder (will be filled after)
    let crc_pos = header.len();
    header.extend_from_slice(&0u32.to_le_bytes());

    // Segment count and table
    let segment_count = (data.len() + 254) / 255;
    header.push(segment_count as u8);

    let mut remaining = data.len();
    for _ in 0..segment_count {
        let seg_size = remaining.min(255);
        header.push(seg_size as u8);
        remaining -= seg_size;
    }

    // Calculate CRC over header + data
    let crc = ogg_crc(&header, data);
    header[crc_pos..crc_pos + 4].copy_from_slice(&crc.to_le_bytes());

    // Write header and data
    writer.write_all(&header)?;
    writer.write_all(data)?;

    Ok(())
}

/// Calculate OGG CRC-32
fn ogg_crc(header: &[u8], data: &[u8]) -> u32 {
    const CRC_TABLE: [u32; 256] = generate_crc_table();

    let mut crc = 0u32;

    // Zero out CRC field in header for calculation
    let mut header_copy = header.to_vec();
    if header_copy.len() >= 26 {
        header_copy[22..26].copy_from_slice(&[0, 0, 0, 0]);
    }

    for &byte in &header_copy {
        crc = (crc << 8) ^ CRC_TABLE[((crc >> 24) as u8 ^ byte) as usize];
    }

    for &byte in data {
        crc = (crc << 8) ^ CRC_TABLE[((crc >> 24) as u8 ^ byte) as usize];
    }

    crc
}

/// Generate OGG CRC lookup table at compile time
const fn generate_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;

    while i < 256 {
        let mut r = (i as u32) << 24;
        let mut j = 0;

        while j < 8 {
            if r & 0x80000000 != 0 {
                r = (r << 1) ^ 0x04c11db7;
            } else {
                r <<= 1;
            }
            j += 1;
        }

        table[i] = r;
        i += 1;
    }

    table
}

/// Generate a random serial number for OGG stream
fn rand_serial() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    (duration.as_nanos() & 0xFFFFFFFF) as u32
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_id_header() {
        let header = create_opus_id_header(1, 16000);
        assert_eq!(&header[..8], b"OpusHead");
        assert_eq!(header[8], 1); // version
        assert_eq!(header[9], 1); // mono
    }

    #[test]
    fn test_opus_comment_header() {
        let header = create_opus_comment_header();
        assert_eq!(&header[..8], b"OpusTags");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
    }
}
