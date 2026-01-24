//! Storage module for minutes
//!
//! Handles database operations using SQLite with FTS5 for full-text search.

mod database;
mod models;
mod repository;

pub use database::Database;
pub use models::{Recording, RecordingState, TranscriptSegment};
pub use repository::Repository;
