use anyhow::Result;
use async_trait::async_trait;

use crate::config::Settings;
use crate::llm::gemini::GeminiClient;

/// Summary generation request payload.
pub struct SummaryRequest<'a> {
    pub title: &'a str,
    pub transcript: &'a str,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn summarize(&self, request: SummaryRequest<'_>) -> Result<String>;
}

/// Build an LLM provider from runtime settings.
pub fn build_provider(settings: &Settings) -> Result<Box<dyn LlmProvider>> {
    match settings.llm.provider.to_lowercase().as_str() {
        "gemini" => Ok(Box::new(GeminiClient::from_settings(settings)?)),
        other => anyhow::bail!(
            "Unsupported llm.provider '{}'. Supported providers: gemini",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn unsupported_provider_returns_error() {
        let mut settings = Settings::default();
        settings.llm.provider = "unknown".to_string();

        let err = match build_provider(&settings) {
            Ok(_) => panic!("expected provider creation to fail"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("Unsupported llm.provider"));
    }

    #[test]
    fn gemini_provider_requires_api_key() {
        let settings = Settings::default();

        let err = match build_provider(&settings) {
            Ok(_) => panic!("expected provider creation to fail"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("Gemini API key is missing"));
    }
}
