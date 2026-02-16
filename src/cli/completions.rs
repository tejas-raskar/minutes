//! Shell completion generation.

use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::cli::args::Cli;

/// Print completion script for the requested shell to stdout.
pub fn print(shell: Shell) {
    let mut cmd = Cli::command();
    let command_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, command_name, &mut io::stdout());
}
