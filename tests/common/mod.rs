use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::TempDir;

pub fn run_minutes(args: &[&str]) -> Output {
    TestEnv::new().run(args)
}

pub struct TestEnv {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    runtime: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        Self {
            home: tempfile::tempdir().expect("create temporary HOME dir"),
            config: tempfile::tempdir().expect("create temporary XDG config dir"),
            data: tempfile::tempdir().expect("create temporary XDG data dir"),
            runtime: tempfile::tempdir().expect("create temporary XDG runtime dir"),
        }
    }

    pub fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_minutes"))
            .args(args)
            .env("HOME", self.home.path())
            .env("XDG_CONFIG_HOME", self.config.path())
            .env("XDG_DATA_HOME", self.data.path())
            .env("XDG_RUNTIME_DIR", self.runtime.path())
            .env_remove("MINUTES_GEMINI_API_KEY")
            .output()
            .expect("failed to execute minutes binary")
    }

    #[allow(dead_code)]
    pub fn config_path(&self) -> PathBuf {
        let output = self.run(&["config", "path"]);
        assert!(
            output.status.success(),
            "config path should succeed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let path = String::from_utf8_lossy(&output.stdout);
        PathBuf::from(path.trim())
    }

    #[allow(dead_code)]
    pub fn write_config(&self, contents: &str) {
        let config_path = self.config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).expect("create config parent directory");
        }
        std::fs::write(&config_path, contents).expect("write config file");
    }
}
