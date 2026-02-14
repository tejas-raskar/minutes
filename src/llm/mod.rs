//! LLM module for minutes (post-MVP)
//!
//! Handles AI-powered summaries and Q&A using Gemini API.

mod client;
mod gemini;
mod prompts;

pub use client::{build_provider, LlmProvider, SummaryRequest};
pub use gemini::GeminiClient;
pub use prompts::build_summary_prompt;
