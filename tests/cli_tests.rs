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
    assert!(stdout.contains("No recordings found."));
    assert!(stdout.contains("minutes start"));
    assert!(
        !stderr.contains("No config file found"),
        "runtime commands should not print config fallback logs by default\nstderr:\n{}",
        stderr
    );
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
    assert!(stdout.contains("minutes daemon start"));
    assert!(
        !stderr.contains("No config file found"),
        "runtime commands should not print config fallback logs by default\nstderr:\n{}",
        stderr
    );
}

#[test]
fn status_reports_not_running_with_hint() {
    let output = run_minutes(&["status"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "status should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("Daemon is not running"));
    assert!(stdout.contains("minutes daemon start"));
    assert!(
        !stderr.contains("No config file found"),
        "runtime commands should not print config fallback logs by default\nstderr:\n{}",
        stderr
    );
}

#[test]
fn list_search_empty_mentions_query_and_next_step() {
    let output = run_minutes(&["list", "--search", "launch"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "list --search should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("No recordings found for query"));
    assert!(stdout.contains("launch"));
    assert!(stdout.contains("minutes list"));
}

#[test]
fn search_empty_mentions_query_and_next_step() {
    let output = run_minutes(&["search", "deadline"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search should succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(stdout.contains("No transcript matches found for"));
    assert!(stdout.contains("deadline"));
    assert!(stdout.contains("minutes list"));
}

#[test]
fn verbose_flag_enables_info_logs() {
    let output = run_minutes(&["--verbose", "list"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "--verbose list should succeed\nstderr:\n{}",
        stderr
    );
    assert!(
        stderr.contains("No config file found"),
        "verbose mode should include info diagnostics in stderr"
    );
}
