mod common;

use common::run_minutes;

#[test]
fn minutes_help_shows_usage() {
    let output = run_minutes(&["--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--help should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("Commands:"));
    assert!(
        !stderr.contains("No config file found"),
        "--help should not log config fallback noise\nstderr:\n{}",
        stderr
    );
}

#[test]
fn minutes_version_shows_version() {
    let output = run_minutes(&["--version"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--version should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("minutes "));
    assert!(
        !stderr.contains("No config file found"),
        "--version should not log config fallback noise\nstderr:\n{}",
        stderr
    );
}

#[test]
fn completions_bash_outputs_script() {
    let output = run_minutes(&["completions", "bash"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "completions bash should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("minutes"),
        "expected completion output to reference command name\nstdout:\n{}",
        stdout
    );
}

#[test]
fn config_show_works() {
    let output = run_minutes(&["config", "show"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "config show should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("[general]"));
    assert!(stdout.contains("data_dir"));
}

#[test]
fn config_path_returns_valid_path() {
    let output = run_minutes(&["config", "path"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "config path should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("config.toml"));
}

#[test]
fn list_works_with_empty_database() {
    let output = run_minutes(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "list should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("No recordings found"));
}

#[test]
fn daemon_status_reports_not_running() {
    let output = run_minutes(&["daemon", "status"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "daemon status should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("Daemon is not running"));
}
