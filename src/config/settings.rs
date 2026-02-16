//! Application settings management

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::audio::AudioBackend;

/// Main application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// General settings
    #[serde(default)]
    pub general: GeneralSettings,

    /// Audio capture settings
    #[serde(default)]
    pub audio: AudioSettings,

    /// Whisper transcription settings
    #[serde(default)]
    pub whisper: WhisperSettings,

    /// LLM settings (post-MVP)
    #[serde(default)]
    pub llm: LlmSettings,

    /// TUI settings
    #[serde(default)]
    pub tui: TuiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Data directory for recordings and database
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Audio backend to use (auto, pipewire, cpal)
    #[serde(default)]
    pub backend: AudioBackend,

    /// Sample rate for recording (default: 16000 for Whisper compatibility)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,

    /// Number of audio channels (1 = mono, 2 = stereo)
    #[serde(default = "default_channels")]
    pub channels: u16,

    /// Whether to capture system audio (what others say)
    #[serde(default = "default_true")]
    pub capture_system: bool,

    /// Whether to capture microphone (what you say)
    #[serde(default = "default_true")]
    pub capture_microphone: bool,

    /// Preferred audio device (empty = default)
    #[serde(default)]
    pub device: String,

    /// Whether to compress recordings to OGG Opus
    #[serde(default = "default_true")]
    pub compress_to_ogg: bool,

    /// OGG Opus bitrate in bits per second (default: 24000 for speech)
    #[serde(default = "default_ogg_bitrate")]
    pub ogg_bitrate: u32,

    /// Microphone boost factor (1.0 = no boost, 1.2 = 20% boost)
    #[serde(default = "default_mic_boost")]
    pub mic_boost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperSettings {
    /// Whisper model to use (tiny, base, small, medium, large)
    #[serde(default = "default_model")]
    pub model: String,

    /// Path to model files directory
    #[serde(default = "default_models_dir")]
    pub models_dir: PathBuf,

    /// Language for transcription (empty = auto-detect)
    #[serde(default)]
    pub language: String,

    /// Enable translation to English
    #[serde(default)]
    pub translate: bool,

    /// Number of threads for inference (0 = auto)
    #[serde(default)]
    pub threads: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    /// LLM provider (gemini, ollama)
    #[serde(default = "default_llm_provider")]
    pub provider: String,

    /// API key (for cloud providers)
    #[serde(default)]
    pub api_key: String,

    /// Model name
    #[serde(default = "default_llm_model")]
    pub model: String,

    /// API endpoint (for local/custom providers)
    #[serde(default)]
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiSettings {
    /// Show timestamps in transcript view
    #[serde(default = "default_true")]
    pub show_timestamps: bool,

    /// Number of recent recordings to show on dashboard
    #[serde(default = "default_recent_count")]
    pub recent_count: usize,

    /// Color theme (dark, light)
    #[serde(default = "default_theme")]
    pub theme: String,
}

// Default value functions

fn default_data_dir() -> PathBuf {
    ProjectDirs::from("com", "minutes", "minutes")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("~/.local/share/minutes"))
}

fn default_models_dir() -> PathBuf {
    let mut dir = default_data_dir();
    dir.push("models");
    dir
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_sample_rate() -> u32 {
    16000
}

fn default_channels() -> u16 {
    1
}

fn default_true() -> bool {
    true
}

fn default_ogg_bitrate() -> u32 {
    24000
}

fn default_mic_boost() -> f32 {
    1.2
}

fn default_model() -> String {
    "base".to_string()
}

fn default_llm_provider() -> String {
    "gemini".to_string()
}

fn default_llm_model() -> String {
    "gemini-2.5-flash".to_string()
}

fn default_recent_count() -> usize {
    5
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            log_level: default_log_level(),
        }
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            backend: AudioBackend::default(),
            sample_rate: default_sample_rate(),
            channels: default_channels(),
            capture_system: true,
            capture_microphone: true,
            device: String::new(),
            compress_to_ogg: true,
            ogg_bitrate: default_ogg_bitrate(),
            mic_boost: default_mic_boost(),
        }
    }
}

impl Default for WhisperSettings {
    fn default() -> Self {
        Self {
            model: default_model(),
            models_dir: default_models_dir(),
            language: String::new(),
            translate: false,
            threads: 0,
        }
    }
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: default_llm_provider(),
            api_key: String::new(),
            model: default_llm_model(),
            endpoint: String::new(),
        }
    }
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            show_timestamps: true,
            recent_count: default_recent_count(),
            theme: default_theme(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            audio: AudioSettings::default(),
            whisper: WhisperSettings::default(),
            llm: LlmSettings::default(),
            tui: TuiSettings::default(),
        }
    }
}

impl Settings {
    /// Load settings from the configuration file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            tracing::info!("No config file found, using defaults");
            let mut settings = Self::default();
            settings.apply_env_overrides();
            return Ok(settings);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let mut settings: Settings = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        settings.apply_env_overrides();

        Ok(settings)
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        if self.llm.api_key.trim().is_empty() {
            if let Ok(key) = std::env::var("MINUTES_GEMINI_API_KEY") {
                if !key.trim().is_empty() {
                    self.llm.api_key = key;
                }
            }
        }
    }

    /// Get the path to the configuration file
    pub fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("com", "minutes", "minutes")
            .context("Could not determine config directory")?;

        let config_dir = dirs.config_dir();
        Ok(config_dir.join("config.toml"))
    }

    /// Write default configuration to a file
    pub fn write_default(path: &PathBuf) -> Result<()> {
        let settings = Self::default();
        let content = toml::to_string_pretty(&settings)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the database path
    pub fn database_path(&self) -> PathBuf {
        self.general.data_dir.join("minutes.db")
    }

    /// Get the audio recordings directory
    pub fn audio_dir(&self) -> PathBuf {
        self.general.data_dir.join("audio")
    }

    /// Get the Unix socket path for IPC
    pub fn socket_path(&self) -> PathBuf {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        runtime_dir.join("minutes.sock")
    }

    /// Get the PID file path
    pub fn pid_path(&self) -> PathBuf {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        runtime_dir.join("minutes.pid")
    }

    /// Ensure all required directories exist
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.general.data_dir)?;
        std::fs::create_dir_all(self.audio_dir())?;
        std::fs::create_dir_all(&self.whisper.models_dir)?;
        Ok(())
    }

    /// Get the path to a whisper model file
    pub fn model_path(&self) -> PathBuf {
        self.whisper.models_dir.join(format!("ggml-{}.bin", self.whisper.model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_gemini_25_flash() {
        let settings = Settings::default();
        assert_eq!(settings.llm.model, "gemini-2.5-flash");
    }
}
