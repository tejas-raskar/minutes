use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::Settings;
use crate::llm::client::{LlmProvider, SummaryRequest};
use crate::llm::prompts::build_summary_prompt;

const DEFAULT_GEMINI_ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";

pub struct GeminiClient {
    http: Client,
    api_key: String,
    model: String,
    endpoint: String,
}

impl GeminiClient {
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        let api_key = settings.llm.api_key.trim().to_string();
        if api_key.is_empty() {
            anyhow::bail!(
                "Gemini API key is missing. Set llm.api_key in config or MINUTES_GEMINI_API_KEY."
            );
        }

        let model = if settings.llm.model.trim().is_empty() {
            DEFAULT_GEMINI_MODEL.to_string()
        } else {
            settings.llm.model.trim().to_string()
        };

        let endpoint = if settings.llm.endpoint.trim().is_empty() {
            DEFAULT_GEMINI_ENDPOINT.to_string()
        } else {
            settings
                .llm
                .endpoint
                .trim()
                .trim_end_matches('/')
                .to_string()
        };

        Ok(Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .context("Failed to build Gemini HTTP client")?,
            api_key,
            model,
            endpoint,
        })
    }

    fn request_url(&self) -> String {
        format!(
            "{}/models/{}:generateContent?key={}",
            self.endpoint, self.model, self.api_key
        )
    }
}

#[async_trait]
impl LlmProvider for GeminiClient {
    async fn summarize(&self, request: SummaryRequest<'_>) -> Result<String> {
        let prompt = build_summary_prompt(request.title, request.transcript);

        let body = GeminiGenerateContentRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: prompt }],
            }],
        };

        let response = self
            .http
            .post(self.request_url())
            .json(&body)
            .send()
            .await
            .context("Gemini request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", format_gemini_http_error(status, &body));
        }

        let payload: GeminiGenerateContentResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        let summary = payload
            .candidates
            .iter()
            .flat_map(|c| c.content.parts.iter())
            .filter_map(|p| p.text.as_deref())
            .map(str::trim)
            .find(|t| !t.is_empty())
            .map(str::to_string)
            .context("Gemini response did not contain summary text")?;

        Ok(summary)
    }
}

#[derive(Debug, Serialize)]
struct GeminiGenerateContentRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResponse,
}

#[derive(Debug, Deserialize)]
struct GeminiContentResponse {
    #[serde(default)]
    parts: Vec<GeminiPartResponse>,
}

#[derive(Debug, Deserialize)]
struct GeminiPartResponse {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorResponse {
    error: GeminiErrorPayload,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorPayload {
    code: Option<u16>,
    message: Option<String>,
    status: Option<String>,
}

fn format_gemini_http_error(status: reqwest::StatusCode, body: &str) -> String {
    let status_text = status.canonical_reason().unwrap_or("Unknown Status");
    let mut message = format!(
        "Gemini API request failed ({} {})",
        status.as_u16(),
        status_text
    );

    if let Some(detail) =
        gemini_error_detail(body).or_else(|| compact_error_body(body).map(|s| s.to_string()))
    {
        message.push_str(": ");
        message.push_str(&detail);
    }

    if let Some(hint) = gemini_status_hint(status) {
        message.push_str(". ");
        message.push_str(hint);
    }

    message
}

fn gemini_error_detail(body: &str) -> Option<String> {
    let payload: GeminiErrorResponse = serde_json::from_str(body).ok()?;
    let message = payload.error.message?.trim().to_string();
    if message.is_empty() {
        return None;
    }

    let status = payload.error.status.unwrap_or_default();
    let code = payload.error.code;

    let detail = match (status.is_empty(), code) {
        (false, Some(code)) => format!("{} (status: {}, code: {})", message, status, code),
        (false, None) => format!("{} (status: {})", message, status),
        (true, Some(code)) => format!("{} (code: {})", message, code),
        (true, None) => message,
    };

    Some(detail)
}

fn compact_error_body(body: &str) -> Option<String> {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }

    if collapsed.chars().count() <= 240 {
        return Some(collapsed);
    }

    let truncated: String = collapsed.chars().take(240).collect();
    Some(format!("{}...", truncated))
}

fn gemini_status_hint(status: reqwest::StatusCode) -> Option<&'static str> {
    match status.as_u16() {
        401 | 403 => Some("Check MINUTES_GEMINI_API_KEY and Gemini API access permissions"),
        404 => Some("Check llm.model and llm.endpoint in your config"),
        429 => Some("Gemini quota or rate limit exceeded; retry later"),
        500..=599 => Some("Gemini service appears unavailable; retry shortly"),
        _ => None,
    }
}
