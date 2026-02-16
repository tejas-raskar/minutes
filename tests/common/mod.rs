use std::process::{Command, Output};

use tempfile::TempDir;

pub fn run_minutes(args: &[&str]) -> Output {
    let env = isolated_env();

    Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(args)
        .env("HOME", env.home.path())
        .env("XDG_CONFIG_HOME", env.config.path())
        .env("XDG_DATA_HOME", env.data.path())
        .env("XDG_RUNTIME_DIR", env.runtime.path())
        .env_remove("MINUTES_GEMINI_API_KEY")
        .output()
        .expect("failed to execute minutes binary")
}

struct IsolatedEnv {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    runtime: TempDir,
}

fn isolated_env() -> IsolatedEnv {
    IsolatedEnv {
        home: tempfile::tempdir().expect("create temporary HOME dir"),
        config: tempfile::tempdir().expect("create temporary XDG config dir"),
        data: tempfile::tempdir().expect("create temporary XDG data dir"),
        runtime: tempfile::tempdir().expect("create temporary XDG runtime dir"),
    }
}
