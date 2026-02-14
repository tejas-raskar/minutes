//! Audio capture module for minutes
//!
//! Provides unified audio capture with multiple backends:
//! - PipeWire (Linux, primary) - captures system audio + microphone
//! - cpal (fallback) - cross-platform, microphone only

mod cpal_capture;
mod encoder;
mod mixer;

#[cfg(feature = "pipewire")]
mod pipewire_capture;

pub use cpal_capture::CpalCapture;
pub use encoder::OggEncoder;
pub use mixer::AudioMixer;

#[cfg(feature = "pipewire")]
pub use pipewire_capture::PipeWireCapture;
#[cfg(feature = "pipewire")]
pub(crate) use pipewire_capture::{resolve_capture_targets, TargetResolutionMethod};

use anyhow::Result;
use std::path::Path;

use crate::config::Settings;

/// Audio backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioBackend {
    /// Auto-detect best available backend
    #[default]
    Auto,
    /// Force PipeWire backend (Linux only)
    PipeWire,
    /// Force cpal backend (cross-platform)
    Cpal,
}

/// Unified audio capture trait
///
/// Abstracts over different audio capture backends (PipeWire, cpal)
pub trait AudioCapture {
    /// Start capturing audio to the specified WAV path
    fn start(&mut self, output_path: &Path) -> Result<()>;

    /// Stop capturing and finalize the file
    fn stop(&mut self) -> Result<()>;

    /// Check if currently recording
    fn is_recording(&self) -> bool;

    /// Get capture backend name for logging
    fn backend_name(&self) -> &'static str;
}

/// Check if PipeWire is available on this system
#[cfg(feature = "pipewire")]
pub fn pipewire_available() -> bool {
    PipeWireCapture::is_available()
}

#[cfg(not(feature = "pipewire"))]
pub fn pipewire_available() -> bool {
    false
}

/// Create an audio capture instance based on settings and platform
///
/// Uses PipeWire on Linux if available (for system audio + mic capture),
/// falls back to cpal otherwise.
pub fn create_capture(settings: &Settings) -> Result<Box<dyn AudioCapture>> {
    match settings.audio.backend {
        AudioBackend::Auto => {
            #[cfg(all(target_os = "linux", feature = "pipewire"))]
            {
                if pipewire_available() {
                    tracing::info!("Using PipeWire audio backend (auto-detected)");
                    return Ok(Box::new(PipeWireCapture::new(settings)?));
                }
            }
            tracing::info!("Using cpal audio backend (fallback)");
            Ok(Box::new(CpalCapture::new(settings)?))
        }
        AudioBackend::PipeWire => {
            #[cfg(all(target_os = "linux", feature = "pipewire"))]
            {
                tracing::info!("Using PipeWire audio backend (forced)");
                return Ok(Box::new(PipeWireCapture::new(settings)?));
            }
            #[cfg(not(all(target_os = "linux", feature = "pipewire")))]
            {
                anyhow::bail!("PipeWire backend is only available on Linux with pipewire feature")
            }
        }
        AudioBackend::Cpal => {
            tracing::info!("Using cpal audio backend (forced)");
            Ok(Box::new(CpalCapture::new(settings)?))
        }
    }
}
