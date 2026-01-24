//! LLM module for minutes (post-MVP)
//!
//! Handles AI-powered summaries and Q&A using Gemini API.

#[cfg(feature = "llm")]
mod client;
#[cfg(feature = "llm")]
mod gemini;
#[cfg(feature = "llm")]
mod prompts;

#[cfg(feature = "llm")]
pub use client::LlmClient;
#[cfg(feature = "llm")]
pub use gemini::GeminiClient;
