//! CLI module for minutes
//!
//! Contains argument parsing and command implementations.

pub mod args;
pub mod commands;
pub mod completions;

pub use args::{Cli, Commands, ConfigCommand, DaemonCommand};
